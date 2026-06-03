use std::sync::RwLock;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use webrtc::{
    ice_transport::ice_connection_state::RTCIceConnectionState,
    ice_transport::ice_server::RTCIceServer,
    peer_connection::{configuration::RTCConfiguration, RTCPeerConnection},
};

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct IceRuntimeConfig {
    pub stun_urls: Vec<String>,
    pub turn_urls: Vec<String>,
    pub turn_username: String,
    pub turn_credential: String,
    /// When true, only relay (TURN) candidates are used — required for some strict NAT setups.
    pub force_relay: bool,
}

impl Default for IceRuntimeConfig {
    fn default() -> Self {
        Self {
            stun_urls: vec![
                "stun:stun.l.google.com:19302".to_owned(),
                "stun:stun1.l.google.com:19302".to_owned(),
            ],
            turn_urls: Vec::new(),
            turn_username: String::new(),
            turn_credential: String::new(),
            force_relay: false,
        }
    }
}

static ICE_CONFIG: Lazy<RwLock<IceRuntimeConfig>> =
    Lazy::new(|| RwLock::new(IceRuntimeConfig::default()));

#[derive(Serialize, Deserialize, Clone)]
pub struct TurnConfigView {
    pub stun_urls: Vec<String>,
    pub turn_urls: Vec<String>,
    pub username: String,
    pub has_credential: bool,
    pub force_relay: bool,
}

pub fn load_from_env() {
    let mut cfg = ICE_CONFIG.write().unwrap_or_else(|e| e.into_inner());

    if let Ok(urls) = std::env::var("STUN_URLS") {
        let parsed: Vec<String> = urls
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if !parsed.is_empty() {
            cfg.stun_urls = parsed;
        }
    }

    if let Ok(urls) = std::env::var("TURN_URLS") {
        cfg.turn_urls = parse_url_list(&urls);
        cfg.turn_username = std::env::var("TURN_USERNAME").unwrap_or_default();
        cfg.turn_credential = std::env::var("TURN_CREDENTIAL").unwrap_or_default();
        if std::env::var("TURN_FORCE_RELAY")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            cfg.force_relay = true;
        }
        if !cfg.turn_urls.is_empty() {
            println!(
                "  WebRTC: loaded {} TURN URL(s) from environment",
                cfg.turn_urls.len()
            );
        }
    }
}

fn parse_url_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Duplicate each `turn:` URL with a TCP transport variant (helps restrictive networks).
fn expand_turn_urls(urls: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for url in urls {
        out.push(url.clone());
        if url.starts_with("turn:") && !url.contains("transport=") {
            out.push(format!("{}?transport=tcp", url));
        }
    }
    out
}

pub fn get_turn_config_view() -> TurnConfigView {
    let cfg = ICE_CONFIG.read().unwrap_or_else(|e| e.into_inner());
    TurnConfigView {
        stun_urls: cfg.stun_urls.clone(),
        turn_urls: cfg.turn_urls.clone(),
        username: cfg.turn_username.clone(),
        has_credential: !cfg.turn_credential.is_empty(),
        force_relay: cfg.force_relay,
    }
}

pub fn set_turn_config(
    turn_urls: String,
    username: String,
    credential: String,
    force_relay: bool,
) -> Result<(), String> {
    let urls = parse_url_list(&turn_urls);
    if force_relay && urls.is_empty() {
        return Err("Force relay requires at least one TURN URL".into());
    }
    for url in &urls {
        if !url.starts_with("turn:") && !url.starts_with("turns:") {
            return Err(format!(
                "Invalid TURN URL '{}': must start with turn: or turns:",
                url
            ));
        }
    }

    let mut cfg = ICE_CONFIG.write().unwrap_or_else(|e| e.into_inner());
    cfg.turn_urls = urls;
    cfg.turn_username = username.trim().to_string();
    cfg.turn_credential = credential;
    cfg.force_relay = force_relay;

    println!(
        "  WebRTC: TURN config updated ({} URL(s), force_relay={})",
        cfg.turn_urls.len(),
        force_relay
    );
    Ok(())
}

pub fn build_rtc_configuration() -> RTCConfiguration {
    let cfg = ICE_CONFIG.read().unwrap_or_else(|e| e.into_inner());

    let mut ice_servers = Vec::new();
    let expanded_turn = expand_turn_urls(&cfg.turn_urls);

    if cfg.force_relay {
        if expanded_turn.is_empty() {
            eprintln!("  WebRTC: force_relay set but no TURN URLs — falling back to STUN only");
        } else {
            ice_servers.push(RTCIceServer {
                urls: expanded_turn,
                username: cfg.turn_username.clone(),
                credential: cfg.turn_credential.clone(),
                ..Default::default()
            });
            return RTCConfiguration {
                ice_servers,
                ..Default::default()
            };
        }
    }

    if !cfg.stun_urls.is_empty() {
        ice_servers.push(RTCIceServer {
            urls: cfg.stun_urls.clone(),
            ..Default::default()
        });
    }

    if !expanded_turn.is_empty() {
        ice_servers.push(RTCIceServer {
            urls: expanded_turn,
            username: cfg.turn_username.clone(),
            credential: cfg.turn_credential.clone(),
            ..Default::default()
        });
    }

    RTCConfiguration {
        ice_servers,
        ..Default::default()
    }
}

pub fn attach_ice_state_handler(
    peer: &Arc<RTCPeerConnection>,
    app: AppHandle,
    peer_id: String,
    role: &'static str,
) {
    peer.on_ice_connection_state_change(Box::new(move |state: RTCIceConnectionState| {
        let app = app.clone();
        let peer_id = peer_id.clone();
        Box::pin(async move {
            let state_name = format!("{:?}", state);
            println!("  ICE [{role}] {peer_id} -> {state_name}");

            let (state_key, hint) = match state {
                RTCIceConnectionState::Connected | RTCIceConnectionState::Completed => {
                    ("connected", None)
                }
                RTCIceConnectionState::Failed => (
                    "failed",
                    Some(
                        "Direct connection failed. Configure a TURN server (Advanced settings) \
                         on both Ground Station and Client, then reconnect. \
                         Use the same TURN_URLS, TURN_USERNAME, and TURN_CREDENTIAL on each device."
                            .to_string(),
                    ),
                ),
                RTCIceConnectionState::Disconnected => ("disconnected", None),
                RTCIceConnectionState::Closed => ("closed", None),
                RTCIceConnectionState::Checking => ("checking", None),
                _ => ("unknown", None),
            };

            let mut payload = serde_json::json!({
                "id": peer_id,
                "state": state_key,
                "role": role,
            });
            if let Some(h) = hint {
                payload["hint"] = serde_json::Value::String(h);
            }

            let _ = app.emit("webrtc-ice-state", payload);
        })
    }));
}
