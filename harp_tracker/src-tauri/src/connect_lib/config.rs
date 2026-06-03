use serde::Serialize;

use crate::connect_lib::{
    network::list_lan_ipv4,
    server::SIGNAL_PORT,
    signaling::{current_signal_server_url, default_local_signal_url},
};

#[derive(Serialize)]
pub struct SignalingConnectHints {
    pub port: u16,
    pub local_url: String,
    pub current_url: String,
    /// ws:// URLs remote clients on the same LAN should use (Ground Station host).
    pub remote_urls: Vec<String>,
}

pub fn signaling_connect_hints() -> SignalingConnectHints {
    let remote_urls: Vec<String> = list_lan_ipv4()
        .into_iter()
        .map(|ip| format!("ws://{}:{}", ip, SIGNAL_PORT))
        .collect();

    SignalingConnectHints {
        port: SIGNAL_PORT,
        local_url: default_local_signal_url(),
        current_url: current_signal_server_url(),
        remote_urls,
    }
}
