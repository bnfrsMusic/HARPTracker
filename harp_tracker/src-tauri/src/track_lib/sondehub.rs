use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::track_lib::position_time::PositionTime;
use crate::track_lib::tracking_type::TrackingType;

#[derive(Clone)]
pub struct SondeHub {
    active: bool,
    tracking_type: TrackingType,
    base_url: String,
    call_sign: String,
    client: Client,
    position_time: PositionTime,
    ground_speed: f64,
    comment: String,
    source_type: String, // "APRS" or "WSPR"
    // Failsafe velocity calculation state
    prev_position: Option<(f64, f64, f64, u64)>, // (lat, lon, alt, timestamp)
}

impl SondeHub {
    pub fn new(call_sign: &str) -> Self {
        Self::new_with_type(call_sign, "APRS")
    }

    pub fn new_with_type(call_sign: &str, source_type: &str) -> Self {
        Self {
            active: true,
            tracking_type: TrackingType::SondeHub,
            base_url: "https://api.v2.sondehub.org/amateur?callsign=".to_string(),
            call_sign: call_sign.to_string(),
            client: Client::new(),
            position_time: PositionTime {
                lat: 0.0,
                lon: 0.0,
                alt: 0.0,
                last_update: 0,
                horiz_vel: 0.0,
                vert_vel: 0.0,
            },
            ground_speed: 0.0,
            comment: String::new(),
            source_type: source_type.to_string(),
            prev_position: None,
        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Build request URL using the configured base_url and callsign
        let url = format!("{}{}", self.base_url, self.call_sign);
        let response: Value = self.client.get(&url).send()?.json()?;

        // response may be either an object keyed by callsign or the callsign object directly
        let call_val = if response.is_object() && response.get(&self.call_sign).is_some() {
            response.get(&self.call_sign).cloned()
        } else {
            Some(response.clone())
        };

        if let Some(call) = call_val {
            if call.is_object() {
                // Extract the position data
                let lat = call["lat"].as_f64().unwrap_or(0.0);
                let lon = call["lon"].as_f64().unwrap_or(0.0);
                let alt = call["alt"].as_f64().unwrap_or(0.0);
                
                // Try time_received first, then fall back to datetime field
                let mut datetime_str = call["time_received"].as_str().unwrap_or("").to_string();
                if datetime_str.is_empty() {
                    datetime_str = call["datetime"].as_str().unwrap_or("").to_string();
                }
                
                let datetime: DateTime<Utc> = if datetime_str.is_empty() {
                    // If both time_received and datetime are missing, use current time
                    eprintln!("WARNING: SondeHub API response missing both time_received and datetime fields, using current time");
                    eprintln!("API URL: {}", url);
                    Utc::now()
                } else {
                    datetime_str.parse().unwrap_or_else(|_| {
                        eprintln!("WARNING: Failed to parse datetime '{}', using current time", datetime_str);
                        eprintln!("API URL: {}", url);
                        Utc::now()
                    })
                };
                let dte = datetime.timestamp() as u64;

                // Try to extract horizontal and vertical speeds from commonly used keys
                let horiz = call["ground_speed"]
                    .as_f64()
                    .or_else(|| call["speed"].as_f64())
                    .or_else(|| call["ascent_rate"].as_f64())
                    .or_else(|| call["hspd"].as_f64())
                    .unwrap_or(0.0);

                let vert = call["vertical_velocity"]
                    .as_f64()
                    .or_else(|| call["vel_h"].as_f64())
                    .unwrap_or(0.0);

                //calculate velocity from position delta if API doesn't provide it
                let (final_horiz, final_vert) = if horiz == 0.0 && vert == 0.0 {
                    if let Some((prev_lat, prev_lon, prev_alt, prev_time)) = self.prev_position {
                        // Only use previous position if not (0,0,0)
                        if (prev_lat, prev_lon, prev_alt) != (0.0, 0.0, 0.0) {
                            let dt = (dte as f64) - (prev_time as f64);
                            if dt > 0.0 && (lat != prev_lat || lon != prev_lon) {
                            let to_rad = |deg: f64| deg * std::f64::consts::PI / 180.0;
                            let r = 6371000.0_f64;
                            let dlat = to_rad(lat - prev_lat);
                            let dlon = to_rad(lon - prev_lon);
                            let a = (dlat / 2.0).sin().powi(2)
                                + to_rad(prev_lat).cos() * to_rad(lat).cos() * (dlon / 2.0).sin().powi(2);
                            let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
                            let dist_m = r * c;
                            
                            let calc_horiz = dist_m / dt;
                            let calc_vert = (alt - prev_alt) / dt;
                            
                            println!(
                                "SondeHub Failsafe velocity: horiz={:.2} m/s, vert={:.2} m/s (dt={:.0}s, dist={:.0}m)",
                                calc_horiz, calc_vert, dt, dist_m
                            );
                                (calc_horiz, calc_vert)
                            } else {
                                (0.0, 0.0)
                            }
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (horiz, vert)
                };

                self.ground_speed = final_horiz;
                self.position_time.update(lat, lon, alt, dte, final_horiz, final_vert);

                // Store current position for next velocity calculation, only if not (0,0,0)
                if (lat, lon, alt) != (0.0, 0.0, 0.0) {
                    self.prev_position = Some((lat, lon, alt, dte));
                }

                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let age_seconds = current_time.saturating_sub(self.position_time.last_update);

                println!(
                    "SondeHub Position: Call: {}, Lat: {}, Lon: {}, Alt: {}m, Last Update: {}s ago",
                    self.call_sign,
                    self.position_time.lat,
                    self.position_time.lon,
                    self.position_time.alt,
                    age_seconds
                );

                return Ok(());
            }
        }

        Err("No SondeHub telemetry data found for this callsign".into())
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

    pub fn get_source_type(&self) -> &str {
        &self.source_type
    }
}
