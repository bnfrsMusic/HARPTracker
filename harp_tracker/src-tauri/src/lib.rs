pub mod connect_lib;
pub mod track_lib;

// Imports
use chrono::Utc;
use dotenvy::dotenv;
use once_cell::sync::Lazy;
use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use track_lib::api;
use track_lib::pred::predictor::{PredictionManager, PredictionParams};
use track_lib::pred::sondhub_predictor::SondeHubPredictor;
use track_lib::tracker::Tracker;

#[tauri::command]
fn get_module_catalog() -> Vec<track_lib::module::ModuleDefinition> {
    TRACKER.lock().unwrap().module_catalog()
}

#[tauri::command]
fn get_module_snapshots() -> Vec<track_lib::module::ModuleSnapshot> {
    TRACKER.lock().unwrap().module_snapshots()
}

#[tauri::command]
fn configure_module(
    module_type: String,
    module_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    TRACKER
        .lock()
        .unwrap()
        .configure_module(&module_type, module_id, config)
}

#[tauri::command]
fn remove_module(module_id: String) -> bool {
    TRACKER.lock().unwrap().remove_module(&module_id)
}

// ==================== External API Commands ====================

/// Get the external API server settings (enabled + port).
#[tauri::command]
fn get_api_settings() -> track_lib::api::ApiSettings {
    api::load_settings()
}

/// Save the external API server settings and (re)start the server to match.
#[tauri::command]
fn set_api_settings(enabled: bool, port: u16) -> Result<track_lib::api::ApiSettings, String> {
    let settings = api::ApiSettings { enabled, port };
    api::save_settings(&settings)?;
    api::apply_api_settings(&settings)?;
    Ok(settings)
}

use crate::connect_lib::{
    client::{client_disconnect, client_run, is_client_connected, ClientState},
    config::{self, SignalingConnectHints},
    ground_station::{
        gs_accept_offer, gs_disconnect, gs_list_pending_offers, gs_reject_offer, gs_remove_client,
        gs_run, GsState,
    },
    ice_config::{self, TurnConfigView},
    server::start_signaling_server,
    signaling::{self, set_signal_server_url},
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

pub struct Coords {
    lat: f64,
    long: f64,
    alt: f64,
}

impl Default for Coords {
    fn default() -> Self {
        Self {
            lat: 0.0,
            long: 0.0,
            alt: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackingPoint {
    lat: f64,
    lon: f64,
    alt: f64,
    time: u64,
    track_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PredictionPoint {
    lat: f64,
    lon: f64,
    alt: f64,
    time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PredictionData {
    ascent: Vec<PredictionPoint>,
    burst: Option<PredictionPoint>,
    landing: Option<PredictionPoint>,
    descent: Vec<PredictionPoint>,
}

impl Coords {
    pub fn new() -> Coords {
        Self {
            lat: 0.0,
            long: 0.0,
            alt: 0.0,
        }
    }
    pub fn update(&mut self, pos: (f64, f64, f64, f64, f64)) {
        self.lat = pos.0;
        self.long = pos.1;
        self.alt = pos.2;
    }
}

// Globals
pub static TRACKER: Lazy<Mutex<Tracker>> = Lazy::new(|| Mutex::new(Tracker::new()));
pub static LOCATION: Lazy<Mutex<Coords>> = Lazy::new(|| Mutex::new(Coords::new()));
pub static FILTERING_METHOD: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::from("Recent")));
pub static PREDICTION_MANAGER: Lazy<Mutex<PredictionManager>> =
    Lazy::new(|| Mutex::new(PredictionManager::new()));
pub static SONDEHUB_PREDICTOR: Lazy<SondeHubPredictor> = Lazy::new(|| SondeHubPredictor::new());

pub static STADIA_MAPS_API_KEY: Lazy<String> = Lazy::new(|| {
    let key = env::var("STADIAMAPS_API_KEY").unwrap_or_else(|_| String::new());
    if key.is_empty() {
        eprintln!("Warning: STADIAMAPS_API_KEY not set in .env file!");
    } else {
        eprintln!("Stadia Maps API key loaded: {}", key);
    }
    key
});

/// Addresses to share with remote clients (LAN ws:// URLs).
#[tauri::command]
fn get_signaling_connect_hints() -> SignalingConnectHints {
    config::signaling_connect_hints()
}

/// Set signaling server target (IP, host:port, or ws:// URL). Returns normalized URL.
#[tauri::command]
fn set_signal_server_host(host_or_url: String) -> Result<String, String> {
    set_signal_server_url(&host_or_url)
}

#[tauri::command]
fn get_signal_server_url() -> String {
    signaling::current_signal_server_url()
}

#[tauri::command]
fn get_turn_config() -> TurnConfigView {
    ice_config::get_turn_config_view()
}

#[tauri::command]
fn set_turn_config(
    turn_urls: String,
    username: String,
    credential: String,
    force_relay: bool,
) -> Result<(), String> {
    ice_config::set_turn_config(turn_urls, username, credential, force_relay)
}

/// Whether a peer id is registered on the shared signaling server (works across app instances).
#[tauri::command]
async fn signaling_peer_online(peer_id: String) -> bool {
    signaling::lookup_peer_online(peer_id.trim())
        .await
        .unwrap_or(false)
}

/// Debug: list peer ids currently on the signaling server.
#[tauri::command]
async fn list_signaling_peers() -> Vec<String> {
    signaling::list_peers_remote().await.unwrap_or_default()
}

// Return the current UTC time formatted
#[tauri::command]
fn utc() -> String {
    format!(
        "UTC {}",
        Utc::now()
            .time()
            .to_string()
            .get(0..10)
            .expect("INVALID UTC")
    )
}

// Return the current date
#[tauri::command]
fn date() -> String {
    format!("Date: {}", Utc::now().date_naive().to_string())
}

// Get the Stadia Maps API key from .env
#[tauri::command]
fn get_stadia_api_key() -> String {
    let val = STADIA_MAPS_API_KEY.as_str().to_string();
    println!(
        "get_stadia_api_key called, returning '{}'",
        if val.is_empty() {
            "<empty>"
        } else {
            "<redacted>"
        }
    );
    val
}

// Update tracker position
#[tauri::command]
fn update() -> String {
    let filtering_method = FILTERING_METHOD.lock().unwrap().clone();
    let estimation_type = match filtering_method.as_str() {
        "Average" => track_lib::position_time::EstimationType::Average,
        "Median" => track_lib::position_time::EstimationType::Median,
        "Recent" => track_lib::position_time::EstimationType::Recent,
        _ => track_lib::position_time::EstimationType::Recent,
    };
    let t = TRACKER.lock().unwrap().update(estimation_type);
    let mut r = String::new();
    for err in t {
        r = format!("{}\nERROR: {:?}\n", r, err);
    }

    let tracker_guard = TRACKER.lock().unwrap();
    let pos = tracker_guard.get_position();
    let velocities = tracker_guard.get_velocities();
    let last_update = tracker_guard.get_last_update();

    LOCATION.try_lock().unwrap().update((
        (pos.0 * 1000.0).round() / 1000.0,
        (pos.1 * 1000.0).round() / 1000.0,
        (pos.2 * 1000.0).round() / 1000.0,
        (velocities.0 * 1000.0).round() / 1000.0,
        (velocities.1 * 1000.0).round() / 1000.0,
    ));

    println!("Update result: {}", r);
    println!("Position: lat={}, lon={}, alt={}", pos.0, pos.1, pos.2);
    println!(
        "Stored velocities: horiz_vel={}, vert_vel={}, last_update={}",
        velocities.0, velocities.1, last_update
    );

    drop(tracker_guard);

    if connect_lib::sync::connected_client_count() > 0 {
        tauri::async_runtime::spawn(async {
            connect_lib::sync::broadcast_position().await;
        });
    }

    r
}

// Get full position
#[tauri::command]
fn get_position() -> (f64, f64, f64) {
    let (l1, l2, alt) = TRACKER.try_lock().unwrap().get_position();
    println!("LOCATION: {}, {}, {}", l1, l2, alt);
    (
        (l1 * 1000.0).round() / 1000.0,
        (l2 * 1000.0).round() / 1000.0,
        (alt * 1000.0).round() / 1000.0,
    )
}

#[tauri::command]
fn get_horiz_vel() -> f64 {
    TRACKER.lock().unwrap().get_velocities().0
}

#[tauri::command]
fn get_vert_vel() -> f64 {
    TRACKER.lock().unwrap().get_velocities().1
}

// Get latitude
#[tauri::command]
fn get_lat() -> f64 {
    LOCATION.lock().unwrap().lat
}

// Get longitude
#[tauri::command]
fn get_long() -> f64 {
    LOCATION.lock().unwrap().long
}

// Get altitude
#[tauri::command]
fn get_alt() -> f64 {
    LOCATION.lock().unwrap().alt
}

// Get time since last update in seconds
#[tauri::command]
fn get_last_update() -> u64 {
    let last = TRACKER.try_lock().unwrap().get_last_update();
    if last != 0 {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        current_time.saturating_sub(last)
    } else {
        0
    }
}

// Get current filtering method
#[tauri::command]
fn get_filtering_method() -> String {
    FILTERING_METHOD.lock().unwrap().clone()
}

// Set filtering method
#[tauri::command]
fn set_filtering_method(method: String) {
    *FILTERING_METHOD.lock().unwrap() = method;
}

///Read the CSV file the current session is writing to and return its tracking points.
///Only data written during this session is returned; CSV files left in the Launch
///Data folder by previous sessions are intentionally ignored.
pub fn read_tracking_history() -> Vec<TrackingPoint> {
    // Grab the current session's CSV path, then drop the lock before doing file I/O.
    let file_path = {
        let tracker = TRACKER.lock().unwrap();
        tracker.csv_path()
    };
    let Some(file_path) = file_path else {
        return vec![];
    };

    let mut points = vec![];

    if let Ok(content) = fs::read_to_string(&file_path) {
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 7 {
                if let (Ok(lat), Ok(lon), Ok(alt), Ok(time)) = (
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<f64>(),
                    parts[6].parse::<u64>(),
                ) {
                    let track_type = parts[0].to_string();
                    points.push(TrackingPoint {
                        lat,
                        lon,
                        alt,
                        time,
                        track_type,
                    });
                }
            }
        }
    }

    points.sort_by_key(|p| p.time);
    points
}

#[tauri::command]
fn get_tracking_history() -> Vec<TrackingPoint> {
    read_tracking_history()
}

// ==================== Prediction Commands ====================

/// Set prediction parameters
#[tauri::command]
fn set_prediction_params(
    payload_mass: f64,
    balloon_mass: f64,
    parachute_drag_coeff: f64,
    burst_altitude: f64,
    ascent_rate: Option<f64>,
    descent_rate: f64,
) {
    let params = PredictionParams {
        payload_mass,
        balloon_mass,
        parachute_drag_coeff,
        burst_altitude,
        ascent_rate,
        descent_rate,
    };

    PREDICTION_MANAGER.lock().unwrap().set_params(params);
    println!("Prediction parameters updated");
}

/// Get current prediction parameters
#[tauri::command]
fn get_prediction_params() -> (f64, f64, f64, f64, Option<f64>, f64) {
    let manager = PREDICTION_MANAGER.lock().unwrap();
    let params = manager.get_params();
    (
        params.payload_mass,
        params.balloon_mass,
        params.parachute_drag_coeff,
        params.burst_altitude,
        params.ascent_rate,
        params.descent_rate,
    )
}

/// Set the active predictor
#[tauri::command]
fn set_predictor(name: String) {
    PREDICTION_MANAGER.lock().unwrap().set_predictor(&name);
    println!("Predictor set to: {}", name);
}

/// Get the active predictor name
#[tauri::command]
fn get_predictor() -> String {
    PREDICTION_MANAGER
        .lock()
        .unwrap()
        .get_predictor()
        .to_string()
}

/// Run prediction with current position and parameters
#[tauri::command]
fn run_prediction() -> Result<PredictionData, String> {
    println!("Starting prediction run...");

    // Get current position from tracker
    let tracker = TRACKER.lock().unwrap();
    let (lat, lon, alt) = tracker.get_position();
    let (horiz_vel, vert_vel) = tracker.get_velocities();
    let last_update = tracker.get_last_update();
    drop(tracker);

    if last_update == 0 {
        return Err("No position data available for prediction".to_string());
    }

    let current_pos = track_lib::position_time::PositionTime {
        lat,
        lon,
        alt,
        last_update,
        horiz_vel,
        vert_vel,
    };

    // Run prediction using the selected predictor
    let mut manager = PREDICTION_MANAGER.lock().unwrap();
    let predictor_name = manager.get_predictor().to_string();
    let params = manager.get_params().clone();

    let pred_result = match predictor_name.as_str() {
        "SondeHub" => manager.run_prediction(&current_pos, &*SONDEHUB_PREDICTOR),
        _ => {
            return Err(format!("Unknown predictor: {}", predictor_name));
        }
    };

    let output = match pred_result {
        Ok(pred_result) => {
            println!("Prediction completed successfully");

            //check if balloon has already burst
            let has_burst = current_pos.alt >= params.burst_altitude || current_pos.vert_vel < 0.0;

            // Convert to serializable format
            let mut ascent: Vec<PredictionPoint> = pred_result
                .ascent
                .iter()
                .map(|p| PredictionPoint {
                    lat: p.lat,
                    lon: p.lon,
                    alt: p.alt,
                    time: p.last_update,
                })
                .collect();

            let burst = pred_result.burst.map(|p| PredictionPoint {
                lat: p.lat,
                lon: p.lon,
                alt: p.alt,
                time: p.last_update,
            });

            let landing = pred_result.landing.map(|p| PredictionPoint {
                lat: p.lat,
                lon: p.lon,
                alt: p.alt,
                time: p.last_update,
            });

            let descent: Vec<PredictionPoint> = pred_result
                .descent
                .iter()
                .map(|p| PredictionPoint {
                    lat: p.lat,
                    lon: p.lon,
                    alt: p.alt,
                    time: p.last_update,
                })
                .collect();

            //if burst already, remove ascent points and ensure burst is the starting point for descent
            if has_burst {
                println!("Balloon has already burst (alt: {}, burst_alt: {}, vert_vel: {}). Clearing ascent trajectory.",
                         current_pos.alt, params.burst_altitude, current_pos.vert_vel);
                ascent.clear();
            }

            Ok(PredictionData {
                ascent,
                burst,
                landing,
                descent,
            })
        }
        Err(e) => {
            println!("Prediction failed: {}", e);
            Err(format!("Prediction failed: {}", e))
        }
    };

    if let Ok(ref data) = output {
        let data = data.clone();
        tauri::async_runtime::spawn(async move {
            connect_lib::sync::broadcast_prediction(data).await;
        });
    }

    output
}

#[tauri::command]
fn fetch_opensky_states(
    lamin: f64,
    lomin: f64,
    lamax: f64,
    lomax: f64,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://opensky-network.org/api/states/all?lamin={}&lomin={}&lamax={}&lomax={}",
        lamin, lomin, lamax, lomax
    );
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("OpenSky request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("OpenSky API returned {}", resp.status()));
    }
    resp.json()
        .map_err(|e| format!("OpenSky response parse failed: {}", e))
}

// Application run
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env file at startup
    dotenv().ok();

    tauri::Builder::default()
        .manage(std::sync::Arc::new(std::sync::Mutex::new(None::<GsState>)))
        .manage(std::sync::Arc::new(std::sync::Mutex::new(
            None::<ClientState>,
        )))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            utc,
            date,
            get_module_catalog,
            get_module_snapshots,
            configure_module,
            remove_module,
            get_api_settings,
            set_api_settings,
            update,
            get_position,
            get_lat,
            get_long,
            get_alt,
            get_horiz_vel,
            get_vert_vel,
            get_last_update,
            get_filtering_method,
            set_filtering_method,
            get_tracking_history,
            set_prediction_params,
            get_prediction_params,
            set_predictor,
            get_predictor,
            run_prediction,
            fetch_opensky_states,
            get_stadia_api_key,
            client_run,
            gs_run,
            client_disconnect,
            is_client_connected,
            gs_disconnect,
            gs_list_pending_offers,
            gs_accept_offer,
            gs_reject_offer,
            gs_remove_client,
            get_signaling_connect_hints,
            set_signal_server_host,
            get_signal_server_url,
            get_turn_config,
            set_turn_config,
            signaling_peer_online,
            list_signaling_peers,
        ])
        // Server iniatialization
        .setup(|_app| {
            ice_config::load_from_env();
            tauri::async_runtime::spawn(async move {
                println!("Starting embedded signaling server...");
                start_signaling_server().await;
            });

            // External API server (default port 8560, configurable in Settings)
            let api_settings = api::load_settings();
            match api::apply_api_settings(&api_settings) {
                Ok(()) if api_settings.enabled => { /* logged by the server itself */ }
                Ok(()) => println!(
                    "External API server disabled by settings (http://localhost:{})",
                    api_settings.port
                ),
                Err(error) => eprintln!("Failed to start external API server: {error}"),
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
