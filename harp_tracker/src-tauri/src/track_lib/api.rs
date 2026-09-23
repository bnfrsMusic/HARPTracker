use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Query, Request},
    http::{HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::track_lib::module::{
    Module, ModuleDefinition, ModuleDescriptor, ModuleSnapshot, ModuleRegistration, ModuleStatus,
    TelemetryEvent,
};
use crate::track_lib::position_time::PositionTime;

// ========================= Telemetry Module =========================

pub const API_MODULE_TYPE: &str = "api";

/// A single connection fed to the tracker over the external API.
/// module is passive (no update) because data arrives by push (POST requests) rather than being polled by the tracker.
pub struct ApiConnection {
    name: String,
    active: bool,
    position_time: PositionTime,
}

impl ApiConnection {
    pub fn new(name: String) -> Self {
        Self {
            name,
            active: true,
            position_time: PositionTime::new(),
        }
    }

    pub fn position_time(&self) -> PositionTime {
        self.position_time.clone()
    }

    pub fn last_update(&self) -> u64 {
        self.position_time.last_update
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Module for ApiConnection {
    fn id(&self) -> &str {
        &self.name
    }

    fn name(&self) -> &str {
        "External API"
    }

    fn module_type(&self) -> &str {
        API_MODULE_TYPE
    }

    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        // Missing points fall back to the last known value (api.md spec).
        let lat = event.lat.unwrap_or(self.position_time.lat);
        let lon = event.lon.unwrap_or(self.position_time.lon);
        let alt = event.alt.unwrap_or(self.position_time.alt);
        if !lat.is_finite() || !lon.is_finite() || !alt.is_finite() {
            return Err("API telemetry contains non-finite values".to_string());
        }

        let timestamp = if event.timestamp != 0 {
            event.timestamp
        } else {
            now_unix()
        };

        // Missing velocities keep the last known value
        let vertical_velocity = event
            .metadata
            .get("vertical_velocity")
            .and_then(Value::as_f64)
            .unwrap_or(self.position_time.vert_vel);
        let horizontal_velocity = event
            .metadata
            .get("ground_speed")
            .or_else(|| event.metadata.get("horizontal_velocity"))
            .and_then(Value::as_f64)
            .unwrap_or(self.position_time.horiz_vel);

        self.position_time
            .update(lat, lon, alt, timestamp, horizontal_velocity, vertical_velocity);
        self.active = true;
        Ok(())
    }

    fn update(&mut self) -> Result<(), String> {
        // Passive module: the tracker never requests data from us.
        Ok(())
    }

    fn position(&self) -> Option<PositionTime> {
        (self.position_time.last_update != 0).then(|| self.position_time.clone())
    }

    fn status(&self) -> ModuleStatus {
        ModuleStatus {
            enabled: self.active,
            connected: self.position_time.last_update != 0,
            last_update: if self.position_time.last_update != 0 {
                Some(self.position_time.last_update)
            } else {
                None
            },
            error: None,
        }
    }

    fn set_status(&mut self, status: ModuleStatus) {
        self.active = status.enabled;
        if let Some(last_update) = status.last_update {
            self.position_time.last_update = last_update;
        }
    }

    fn descriptor(&self) -> ModuleDescriptor {
        let last_update = if self.position_time.last_update != 0 {
            Some(self.position_time.last_update)
        } else {
            None
        };
        ModuleDescriptor {
            id: self.name.clone(),
            name: self.name().to_string(),
            enabled: self.active,
            connected: self.position_time.last_update != 0,
            last_update,
            module_type: self.module_type().to_string(),
        }
    }
}

fn api_definition() -> ModuleDefinition {
    ModuleDefinition {
        module_type: API_MODULE_TYPE.to_string(),
        display_name: "External API".to_string(),
        description: "Connection fed to the tracker via the external HTTP API.".to_string(),
        // No configuration fields: the connection is fully described by its
        // name and is created automatically on first POST.
        fields: vec![],
    }
}

fn create_api_connection(id: String, _config: Value) -> Result<Box<dyn Module>, String> {
    if id.trim().is_empty() {
        return Err("ConnectionName must not be empty".to_string());
    }
    Ok(Box::new(ApiConnection::new(id)))
}

inventory::submit! {
    ModuleRegistration { definition: api_definition, create: create_api_connection }
}

// ========================= HTTP API Types =========================

/// Accepted POST payload (api.md). All fields except `ConnectionName` are
/// missing values are filled with the last known value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTelemetry {
    #[serde(rename = "ConnectionName")]
    #[serde(default)]
    pub connection_name: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    #[serde(default)]
    pub alt: Option<f64>,
    #[serde(default)]
    pub vertical_velocity: Option<f64>,
    #[serde(default)]
    pub ground_speed: Option<f64>,
    /// Unix timestamp of the telemetry sample.
    #[serde(default)]
    pub last_updated: Option<u64>,
}

/// Response shape for a single connection (api.md).
#[derive(Debug, Clone, Serialize)]
pub struct ApiConnectionView {
    #[serde(rename = "ConnectionName")]
    pub connection_name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub vertical_velocity: f64,
    pub ground_speed: f64,
    pub last_updated: u64,
}

pub fn connection_view(snapshot: &ModuleSnapshot, position: Option<&PositionTime>) -> ApiConnectionView {
    let default_position = PositionTime::new();
    let position = position.unwrap_or(&default_position);
    ApiConnectionView {
        // `id` is the name the connection was registered under — the key
        // `?ConnectionName=` is matched against.
        connection_name: snapshot.id.clone(),
        lat: position.lat,
        lon: position.lon,
        alt: position.alt,
        vertical_velocity: position.vert_vel,
        ground_speed: position.horiz_vel,
        last_updated: position.last_update,
    }
}

// ========================= HTTP Handlers =========================

fn json_response(status: StatusCode, body: Option<Value>) -> Response {
    let body = body.unwrap_or(Value::Null);
    let mut response = (status, axum::Json(body)).into_response();
    response.headers_mut().extend([
        (
            HeaderName::from_static("access-control-allow-origin"),
            HeaderValue::from_static("*"),
        ),
        (
            HeaderName::from_static("access-control-allow-methods"),
            HeaderValue::from_static("GET, POST, OPTIONS"),
        ),
        (
            HeaderName::from_static("access-control-allow-headers"),
            HeaderValue::from_static("content-type"),
        ),
    ]);
    response
}

async fn cors_middleware(request: Request, next: Next) -> Response {
    if request.method() == Method::OPTIONS {
        return json_response(StatusCode::NO_CONTENT, None);
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(HeaderName::from_static("access-control-allow-origin"), HeaderValue::from_static("*"));
    response
}

/// POST / — upsert a connection from external telemetry.
async fn post_telemetry(body: String) -> Response {
    let telemetry: ApiTelemetry = match serde_json::from_str(&body) {
        Ok(telemetry) => telemetry,
        Err(error) => {
            return json_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                Some(json!({ "error": format!("Invalid telemetry payload: {error}") })),
            );
        }
    };

    let name = telemetry
        .connection_name
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "ConnectionName is required" })),
        );
    }

    let result = {
        let mut tracker = crate::TRACKER.lock().unwrap();
        tracker.upsert_api_connection(&telemetry)
    };

    match result {
        Ok(status) => json_response(
            StatusCode::OK,
            Some(json!({ "status": status, "ConnectionName": name })),
        ),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(json!({ "error": error })),
        ),
    }
}

/// GET / — current position of one connection (?ConnectionName=...) or of all
/// active connections.
async fn get_connections(query: Query<HashMap<String, String>>) -> Response {
    let name = query
        .0
        .get("ConnectionName")
        .or_else(|| query.0.get("connectionName"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());

    let tracker = crate::TRACKER.lock().unwrap();
    let snapshots = tracker.module_snapshots();
    let positions = tracker.positions_by_id();

    match name {
        Some(name) => match snapshots
            .iter()
            .find(|snapshot| snapshot.id.eq_ignore_ascii_case(&name))
        {
            Some(snapshot) => {
                let position = positions
                    .iter()
                    .find_map(|(id, pos)| (id == &snapshot.id).then_some(pos));
                json_response(
                    StatusCode::OK,
                    Some(json!(connection_view(snapshot, position))),
                )
            }
            None => json_response(
                StatusCode::NOT_FOUND,
                Some(json!({ "error": format!("No connection found with name '{name}'") })),
            ),
        },
        None => {
            let views: Vec<ApiConnectionView> = snapshots
                .iter()
                .map(|snapshot| {
                    let position = positions
                        .iter()
                        .find_map(|(id, pos)| (id == &snapshot.id).then_some(pos));
                    connection_view(snapshot, position)
                })
                .collect();
            json_response(StatusCode::OK, Some(json!(views)))
        }
    }
}

/// axum router for the external API server.
pub fn build_router() -> Router {
    Router::new()
        .route("/", post(post_telemetry).get(get_connections))
        .route("/health", get(|| async { axum::Json(json!({ "status": "ok" })) }))
        .layer(middleware::from_fn(cors_middleware))
}

/// Serve the router on an already-bound listener until `cancel` fires.
pub async fn run_server(listener: TcpListener, cancel: CancellationToken) {
    let app = build_router();
    let shutdown_token = cancel.clone();
    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown_token.cancelled().await })
        .await
    {
        eprintln!("External API server error: {error}");
    }
    println!("External API server stopped");
}

// ========================= Server Lifecycle + Settings =========================

pub const DEFAULT_API_PORT: u16 = 8560;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: DEFAULT_API_PORT,
        }
    }
}

fn settings_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|dir| dir.join("harp-tracker").join("api.json"))
}

pub fn load_settings() -> ApiSettings {
    settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str::<ApiSettings>(&content).ok())
        .filter(|settings| (1..=65535).contains(&settings.port))
        .unwrap_or_default()
}

pub fn save_settings(settings: &ApiSettings) -> Result<(), String> {
    let path = settings_path().ok_or("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create config directory: {error}"))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize API settings: {error}"))?;
    std::fs::write(&path, content).map_err(|error| format!("Could not save API settings: {error}"))
}

static API_SERVER: Lazy<Mutex<Option<(u16, CancellationToken)>>> =
    Lazy::new(|| Mutex::new(None));

/// Start, stop, or restart the API server so it matches `settings`.
pub fn apply_api_settings(settings: &ApiSettings) -> Result<(), String> {
    if !(1..=65535).contains(&settings.port) {
        return Err(format!("API port must be between 1 and 65535, got {}", settings.port));
    }
    let mut guard = API_SERVER.lock().unwrap();

    if !settings.enabled {
        if let Some((port, cancel)) = guard.take() {
            cancel.cancel();
            println!("External API server on port {port} stopped");
        }
        return Ok(());
    }

    if let Some((port, cancel)) = guard.as_ref() {
        if *port == settings.port {
            return Ok(()); // already running on the requested port
        }
        cancel.cancel();
        println!("External API server on port {port} stopping...");
    }

    let cancel = CancellationToken::new();
    let server_token = cancel.clone();
    let port = settings.port;
    tauri::async_runtime::spawn(async move {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => {
                println!("External API server listening on http://localhost:{}", port);
                run_server(listener, server_token).await;
            }
            Err(error) => eprintln!("Failed to bind external API server on port {port}: {error}"),
        }
    });
    *guard = Some((port, cancel));
    Ok(())
}


}
