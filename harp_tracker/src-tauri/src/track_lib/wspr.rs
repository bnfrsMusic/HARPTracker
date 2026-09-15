//This module is currently a wip
//biggest issue to fix is getting the API to work with the trailing -XXXX suffix

use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use urlencoding::encode;

use crate::track_lib::module::{Module, ModuleDefinition, ModuleDescriptor, ModuleField, ModuleRegistration, ModuleStatus, TelemetryEvent};
use crate::track_lib::position_time::PositionTime;

#[derive(Clone)]
pub struct WSPR {
    active: bool,
    debug: bool,
    base_url: String,
    call_sign: String,
    client: Client,
    position_time: PositionTime,
    ground_speed: f64,
    distance: f64,
    comment: String,
    // Failsafe velocity calculation state
    prev_position: Option<(f64, f64, f64, u64)>, // (lat, lon, alt, timestamp)
}

impl WSPR {
    //disabled for now until error is fixed
    const DISABLED: bool = true;

    fn strip_wspr_suffix(call: &str) -> String {
        call.to_uppercase()
            // .split('-')
            // .next()
            // .unwrap_or(call)
            // .to_string()
    }

    pub fn new(call_sign: &str) -> Self {
        Self {
            active: true,
            debug: false,
            base_url: "https://db1.wspr.live/".to_string(),
            call_sign: Self::strip_wspr_suffix(call_sign),
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
            position_time: PositionTime {
                lat: 0.0,
                lon: 0.0,
                alt: 0.0,
                last_update: 0,
                horiz_vel: 0.0,
                vert_vel: 0.0,
            },
            ground_speed: 0.0,
            distance: 0.0,
            comment: String::new(),
            prev_position: None,
        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if disabled
        if Self::DISABLED {
            return Ok(());
        }
        //SQL Query
        let query = format!(
            r#"
SELECT 
    time, 
    tx_sign, 
    tx_lat, 
    tx_lon, 
    tx_loc,
    power,
    distance
FROM wspr.rx 
WHERE tx_sign LIKE '{}%'
  AND time >= now() - INTERVAL 24 HOUR
ORDER BY time DESC 
LIMIT 1 
FORMAT JSON
"#,
            Self::strip_wspr_suffix(&self.call_sign)
        );

        //Build URL
        let encoded_query = encode(&query);
        
        //Add max_execution_time to fail faster if query is slow (5 seconds)
        let url = format!("{}?query={}&max_execution_time=5", self.base_url, encoded_query);

        if self.debug {
            eprintln!("[WSPR] GET {}", url);
        }

        let response = self.client.get(&url).send().map_err(|e| {
            eprintln!("WSPR API Request Error: {}", url);
            eprintln!("Error details: {}", e);
            Box::new(e) as Box<dyn std::error::Error>
        })?;
        
        //get raw text first 
        let body_text = response.text().map_err(|e| {
            eprintln!("WSPR: Failed to read response body as text");
            eprintln!("WSPR API URL: {}", url);
            eprintln!("Error details: {}", e);
            Box::new(e) as Box<dyn std::error::Error>
        })?;
        
        //debug
        eprintln!("WSPR: Raw response body length: {} bytes", body_text.len());
        if body_text.is_empty() {
            eprintln!("WSPR: Response body is empty!");
            eprintln!("WSPR API URL: {}", url);
            return Err("WSPR API returned empty response".into());
        }
        
        //Check for database timeout error
        if body_text.contains("TIMEOUT_EXCEEDED") || body_text.contains("Timeout exceeded") {
            eprintln!("WSPR: Database query timed out - callsign search may have too many results");
            eprintln!("WSPR API URL: {}", url);
            eprintln!("Suggestion: Try with a more specific callsign or wait for database to calm down");
            return Err("WSPR database timeout - query took too long".into());
        }
        
        //check for other ClickHouse errors
        if body_text.contains("Code:") && body_text.contains("Exception") {
            eprintln!("WSPR: Database returned an error");
            eprintln!("WSPR API URL: {}", url);
            eprintln!("Error response: {}", body_text);
            return Err(format!("WSPR database error: {}", body_text).into());
        }
        
        // Print clickable request URL for debugging
        eprintln!("WSPR API Request: {}", url);
        
        // Parse as JSON
        let response: Value = serde_json::from_str(&body_text).map_err(|e| {
            eprintln!("WSPR: Failed to parse response as JSON");
            eprintln!("WSPR API URL: {}", url);
            eprintln!("Error details: {}", e);
            eprintln!("Full response body: {}", body_text);
            Box::new(e) as Box<dyn std::error::Error>
        })?;

        //Parse the JSON response
        if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
            if data.is_empty() {
                return Err(format!("No WSPR spots found for callsign: {}", self.call_sign).into());
            }

            //get first row from result
            if let Some(row) = data.first() {
                if !row.is_object() {
                    return Err("Invalid WSPR data format".into());
                }

                //Extract fields from the response object
                let time_str = row["time"]
                    .as_str()
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_string();

                let tx_lat = row["tx_lat"].as_f64().unwrap_or(0.0);
                let tx_lon = row["tx_lon"].as_f64().unwrap_or(0.0);
                let altitude = 0.0; // WSPR doesn't provide altitude

                //Parse the timestamp
                let timestamp: u64 = if !time_str.is_empty() {
                    // Try parsing as RFC3339 first
                    match DateTime::parse_from_rfc3339(&time_str) {
                        Ok(dt) => dt.with_timezone(&Utc).timestamp() as u64,
                        Err(_) => {
                            //try parsing as NaiveDateTime
                            match NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%d %H:%M:%S") {
                                Ok(naive_dt) => {
                                    //Assume UTC
                                    naive_dt.and_utc().timestamp() as u64
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse WSPR timestamp '{}': {}", time_str, e);
                                    0
                                }
                            }
                        }
                    }
                } else {
                    0
                };

                //extract distance
                self.distance = row["distance"].as_f64().unwrap_or(0.0);

                //Extract callsign 
                let tx_sign = row["tx_sign"].as_str().unwrap_or("").to_string();

                //Calculate velocity from position delta
                let (calc_horiz_vel, calc_vert_vel) = if let Some((prev_lat, prev_lon, prev_alt, prev_time)) = self.prev_position {
                    //Only use previous position if not (0,0,0)
                    if (prev_lat, prev_lon, prev_alt) != (0.0, 0.0, 0.0) {
                        let dt = (timestamp as f64) - (prev_time as f64);
                        if dt > 0.0 && (tx_lat != prev_lat || tx_lon != prev_lon) {
                            
                            //Haversine distance
                            let to_rad = |deg: f64| deg * std::f64::consts::PI / 180.0;
                            let r = 6371000.0_f64;
                            let dlat = to_rad(tx_lat - prev_lat);
                            let dlon = to_rad(tx_lon - prev_lon);
                            let a = (dlat / 2.0).sin().powi(2)
                                + to_rad(prev_lat).cos() * to_rad(tx_lat).cos() * (dlon / 2.0).sin().powi(2);
                            let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
                            let dist_m = r * c;
                            
                            let horiz_vel = dist_m / dt; // m/s
                            let vert_vel = (altitude - prev_alt) / dt; // m/s
                            
                            println!(
                                "WSPR Failsafe velocity: horiz={:.2} m/s, vert={:.2} m/s (dt={:.0}s, dist={:.0}m)",
                                horiz_vel, vert_vel, dt, dist_m
                            );
                            (horiz_vel, vert_vel)
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0)
                };

                self.ground_speed = calc_horiz_vel;

                //Update position_time with calculated vel
                self.position_time.update(
                    tx_lat,
                    tx_lon,
                    altitude,
                    timestamp,
                    calc_horiz_vel,
                    calc_vert_vel,
                );

                //store current pos for next velocity calculation, only if not (0,0,0)
                if (tx_lat, tx_lon, altitude) != (0.0, 0.0, 0.0) {
                    self.prev_position = Some((tx_lat, tx_lon, altitude, timestamp));
                }

                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let age_seconds = current_time.saturating_sub(self.position_time.last_update);

                println!(
                    "WSPR Position: Call: {}, Lat: {:.4}, Lon: {:.4}, Distance: {} km, Last Update: {}s ago",
                    tx_sign,
                    self.position_time.lat,
                    self.position_time.lon,
                    self.distance,
                    age_seconds
                );

                return Ok(());
            }
        }

        Err("No valid WSPR data received from API".into())
    }

    pub fn get_pos_time(&self) -> PositionTime {
        self.position_time.clone()
    }

    pub fn get_position(&self) -> (f64, f64, f64) {
        (
            self.position_time.lat,
            self.position_time.lon,
            self.position_time.alt,
        )
    }

    pub fn get_speed(&self) -> f64 {
        self.ground_speed
    }

    pub fn get_last_update(&self) -> u64 {
        self.position_time.last_update
    }

    pub fn get_comment(&self) -> &str {
        &self.comment
    }

    pub fn get_call_sign(&self) -> &str {
        &self.call_sign
    }

    pub fn get_distance(&self) -> f64 {
        self.distance
    }
}
/// module implementation for WSPR
impl Module for WSPR {
    fn id(&self) -> &str {
        &self.call_sign
    }

    fn name(&self) -> &str {
        "WSPR"
    }

    fn module_type(&self) -> &str {
        "wspr"
    }

    fn supports_source(&self, source: &str) -> bool {
        source.eq_ignore_ascii_case(&self.call_sign) || source.eq_ignore_ascii_case(self.name())
    }

    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        let lat = event.lat.unwrap_or(self.position_time.lat);
        let lon = event.lon.unwrap_or(self.position_time.lon);
        let alt = event.alt.unwrap_or(self.position_time.alt);
        self.position_time.update(lat, lon, alt, event.timestamp, 0.0, 0.0);
        self.active = true;
        Ok(())
    }

    fn update(&mut self) -> Result<(), String> {
        self.update_position().map_err(|error| error.to_string())
    }

    fn position(&self) -> Option<PositionTime> {
        (self.position_time.last_update != 0).then(|| self.position_time.clone())
    }

    fn status(&self) -> ModuleStatus {
        ModuleStatus {
            enabled: self.active,
            connected: self.active && self.position_time.last_update != 0,
            last_update: if self.position_time.last_update != 0 { Some(self.position_time.last_update) } else { None },
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
        ModuleDescriptor {
            id: self.call_sign.clone(),
            name: self.name().to_string(),
            enabled: self.active,
            connected: self.active && self.position_time.last_update != 0,
            last_update: if self.position_time.last_update != 0 { Some(self.position_time.last_update) } else { None },
            module_type: self.module_type().to_string(),
        }
    }
}

fn wspr_definition() -> ModuleDefinition {
    ModuleDefinition {
        module_type: "wspr".to_string(),
        display_name: "WSPR".to_string(),
        description: "Track WSPR transmitter positions.".to_string(),
        fields: vec![ModuleField { key: "call_sign".to_string(), label: "Callsign".to_string(), field_type: "text".to_string(), required: true, secret: false, placeholder: Some("Callsign".to_string()) }],
    }
}

fn create_wspr(id: String, config: Value) -> Result<Box<dyn Module>, String> {
    let call_sign = config.get("call_sign").and_then(Value::as_str).unwrap_or(&id);
    Ok(Box::new(WSPR::new(call_sign)))
}

inventory::submit! {
    ModuleRegistration { definition: wspr_definition, create: create_wspr }
}
