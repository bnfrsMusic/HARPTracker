use chrono::DateTime;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::track_lib::module::{
    Module, ModuleDefinition, ModuleDescriptor, ModuleField, ModuleRegistration, ModuleStatus,
    TelemetryEvent,
};
use crate::track_lib::position_time::PositionTime;

#[derive(Clone)]
pub struct SondeHub {
    active: bool,
    debug: bool,
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

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|number| number as f64))
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn parse_position(value: Option<&Value>) -> Option<(f64, f64)> {
    let position = value?.as_str()?;
    let mut parts = position.split(',').map(str::trim);
    let lat = parts.next()?.parse::<f64>().ok()?;
    let lon = parts.next()?.parse::<f64>().ok()?;
    Some((lat, lon))
}

fn parse_timestamp(value: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.timestamp().max(0) as u64)
}

fn record_timestamp(record: &Value) -> Option<u64> {
    ["datetime", "time_received", "upload_time"]
        .iter()
        .filter_map(|key| {
            record
                .get(*key)
                .and_then(Value::as_str)
                .and_then(parse_timestamp)
        })
        .max()
}

fn is_wspr_source(source: &str) -> bool {
    source.trim().to_ascii_lowercase().starts_with("wspr")
}

fn select_record<'a>(response: &'a Value, call_sign: &str) -> Option<&'a Value> {
    if response.get("lat").is_some() || response.get("position").is_some() {
        return response.as_object().map(|_| response);
    }

    let object = response.as_object()?;
    object
        .get(call_sign)
        .or_else(|| {
            object
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(call_sign))
                .map(|(_, value)| value)
        })
        .or_else(|| {
            object.values().find(|value| {
                value
                    .get("payload_callsign")
                    .and_then(Value::as_str)
                    .is_some_and(|payload| payload.eq_ignore_ascii_case(call_sign))
            })
        })
}

impl SondeHub {
    pub fn new(call_sign: &str) -> Self {
        Self::new_with_type(call_sign, "APRS")
    }

    pub fn new_with_type(call_sign: &str, source_type: &str) -> Self {
        Self {
            active: true,
            debug: false,
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
        let url = format!("{}{}", self.base_url, self.call_sign);
        if self.debug {
            eprintln!("[SondeHub] GET {}", url);
        }
        let response: Value = self.client.get(&url).send()?.json()?;

        let call = select_record(&response, &self.call_sign).ok_or_else(|| {
            format!(
                "No SondeHub telemetry found for '{}': {}",
                self.call_sign, response
            )
        })?;

        let (lat, lon) = match (number(call.get("lat")), number(call.get("lon"))) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => parse_position(call.get("position"))
                .ok_or("SondeHub record is missing valid latitude and longitude")?,
        };
        let alt = number(call.get("alt")).unwrap_or(0.0);

        let dte = record_timestamp(call)
            .filter(|timestamp| *timestamp != 0)
            .or_else(|| {
                (self.position_time.last_update != 0).then_some(self.position_time.last_update)
            })
            .ok_or("SondeHub record has no valid datetime, time_received, or upload_time")?;

        if self.debug {
            eprintln!(
                "[SondeHub] {} record timestamp {} ({})",
                self.call_sign,
                dte,
                chrono::DateTime::from_timestamp(dte as i64, 0)
                    .map(|timestamp| timestamp.to_rfc3339())
                    .unwrap_or_else(|| "invalid timestamp".to_string())
            );
        }

        let horiz = ["vel_h", "ground_speed", "speed", "hspd"]
            .iter()
            .find_map(|key| number(call.get(*key)))
            .unwrap_or(0.0);
        let vert = ["vel_v", "vertical_velocity", "ascent_rate"]
            .iter()
            .find_map(|key| number(call.get(*key)))
            .unwrap_or(0.0);

        self.comment = call
            .get("comment")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if let Some(modulation) = call.get("modulation").and_then(Value::as_str) {
            self.source_type = modulation.to_string();
        }

        //calculate velocity from position delta if API doesn't provide it
        let (final_horiz, final_vert) = if horiz == 0.0 && vert == 0.0 {
            if let Some((prev_lat, prev_lon, prev_alt, prev_time)) = self.prev_position {
                if (prev_lat, prev_lon, prev_alt) != (0.0, 0.0, 0.0) {
                    let dt = (dte as f64) - (prev_time as f64);
                    if dt > 0.0 && (lat != prev_lat || lon != prev_lon) {
                        let to_rad = |deg: f64| deg * std::f64::consts::PI / 180.0;
                        let r = 6371000.0_f64;
                        let dlat = to_rad(lat - prev_lat);
                        let dlon = to_rad(lon - prev_lon);
                        let a = (dlat / 2.0).sin().powi(2)
                            + to_rad(prev_lat).cos()
                                * to_rad(lat).cos()
                                * (dlon / 2.0).sin().powi(2);
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
        self.position_time
            .update(lat, lon, alt, dte, final_horiz, final_vert);

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

        Ok(())
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
// module implementation for SondeHub
impl Module for SondeHub {
    fn id(&self) -> &str {
        &self.call_sign
    }

    fn name(&self) -> &str {
        "SondeHub"
    }

    fn module_type(&self) -> &str {
        if is_wspr_source(&self.source_type) {
            "wspr"
        } else {
            "sondehub"
        }
    }

    fn supports_source(&self, source: &str) -> bool {
        source.eq_ignore_ascii_case(&self.call_sign) || source.eq_ignore_ascii_case(self.name())
    }

    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        let lat = event.lat.unwrap_or(self.position_time.lat);
        let lon = event.lon.unwrap_or(self.position_time.lon);
        let alt = event.alt.unwrap_or(self.position_time.alt);
        self.position_time
            .update(lat, lon, alt, event.timestamp, 0.0, 0.0);
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
        ModuleDescriptor {
            id: self.call_sign.clone(),
            name: self.name().to_string(),
            enabled: self.active,
            connected: self.active && self.position_time.last_update != 0,
            last_update: if self.position_time.last_update != 0 {
                Some(self.position_time.last_update)
            } else {
                None
            },
            module_type: if is_wspr_source(&self.source_type) {
                "wspr".to_string()
            } else {
                self.module_type().to_string()
            },
        }
    }
}

fn sondehub_definition() -> ModuleDefinition {
    ModuleDefinition {
        module_type: "sondehub".to_string(),
        display_name: "SondeHub".to_string(),
        description: "Track an amateur balloon through SondeHub.".to_string(),
        fields: vec![ModuleField {
            key: "call_sign".to_string(),
            label: "Callsign".to_string(),
            field_type: "text".to_string(),
            required: true,
            secret: false,
            placeholder: Some("Callsign".to_string()),
        }],
    }
}

fn create_sondehub(id: String, config: Value) -> Result<Box<dyn Module>, String> {
    let call_sign = config
        .get("call_sign")
        .and_then(Value::as_str)
        .unwrap_or(&id);
    Ok(Box::new(SondeHub::new(call_sign)))
}

inventory::submit! {
    ModuleRegistration { definition: sondehub_definition, create: create_sondehub }
}

#[cfg(test)]
mod tests {
    use super::{number, parse_position, parse_timestamp, select_record};
    use serde_json::json;

    #[test]
    fn selects_callsign_key_from_sondehub_response() {
        let response = json!({
            "ZS6WBT-1": {
                "payload_callsign": "ZS6WBT-1",
                "lat": -14.8125,
                "lon": 93.625,
                "alt": 15780
            },
            "OTHER": {"lat": 1.0, "lon": 2.0}
        });

        let record = select_record(&response, "zs6wbt-1").expect("record should be found");
        assert_eq!(number(record.get("lat")), Some(-14.8125));
        assert_eq!(number(record.get("alt")), Some(15780.0));
    }

    #[test]
    fn parses_sondehub_fallback_fields() {
        let response = json!({
            "WE4UAH-8": {
                "lat": "35.575833333333335",
                "lon": "-86.79933333333334",
                "position": "35.575833333333335,-86.79933333333334",
                "datetime": "2026-09-15T15:25:20.227429Z"
            }
        });

        let record = select_record(&response, "WE4UAH-8").unwrap();
        assert_eq!(number(record.get("lat")), Some(35.575833333333335));
        assert_eq!(
            parse_position(record.get("position")),
            Some((35.575833333333335, -86.79933333333334))
        );
        assert_eq!(
            parse_timestamp(record["datetime"].as_str().unwrap()),
            Some(1789485920)
        );
    }
}
