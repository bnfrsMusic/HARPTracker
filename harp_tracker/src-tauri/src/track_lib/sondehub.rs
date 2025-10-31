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
}

impl SondeHub {
    pub fn new(call_sign: &str) -> Self {
        Self {
            active: true,
            tracking_type:TrackingType::SondeHub,
            base_url: "https://api.v2.sondehub.org/amateur?callsign".to_string(),
            call_sign: call_sign.to_string(),
            client: Client::new(),
            position_time: PositionTime {lat:0.0, lon:0.0, alt:0.0, last_update:0},
            ground_speed: 0.0,
            comment: String::new(),
        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        
        let response: Value = self.client.get(&self.base_url).send()?.json()?;
        let call = Some(response[self.call_sign.clone()].clone());
        
        if call.is_some() && call.as_ref().unwrap().is_object(){
            // Extract the position data
        
            let call = &call.unwrap();
            self.position_time.lat = call["lat"].as_f64().unwrap();
            self.position_time.lon = call["lon"].as_f64().unwrap();
            self.position_time.alt = call["alt"].as_f64().unwrap();
            let datetime = call["time_received"].as_str().unwrap();
            let datetime: DateTime<Utc> = datetime.parse().expect("Failed to parse datetime");
            self.position_time.last_update = datetime.timestamp() as u64;

            let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
            let age_seconds = current_time.saturating_sub(self.position_time.last_update);

            
            println!(
                "SondeHub Position: Call: {}, Lat: {}, Lon: {}, Alt: {}m, Last Update: {}s ago",
                self.call_sign, self.position_time.lat, self.position_time.lon, self.position_time.alt, age_seconds
            );
            return Ok(());
        }

        Err("No SondeHub telemetry data found for this callsign".into())
    }
   
    pub fn get_pos_time(&self) -> PositionTime{
        self.position_time.clone()
    }
   
    pub fn get_position(&self) -> (f64, f64, f64) {
        (self.position_time.lat, self.position_time.lon, self.position_time.alt)
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
}