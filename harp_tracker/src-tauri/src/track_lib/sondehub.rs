use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SondeHub {
    active: bool,
    base_url: String,
    call_sign: String,
    client: Client,
    lat: f64,
    long: f64,
    alt: f64,
    ground_speed: f64,
    last_update: u64,
    comment: String,
}

impl SondeHub {
    pub fn new(call_sign: &str) -> Self {
        Self {
            active: true,
            base_url: "https://api.v2.sondehub.org/amateur?callsign".to_string(),
            call_sign: call_sign.to_string(),
            client: Client::new(),
            lat: 0.0,
            long: 0.0,
            alt: 0.0,
            ground_speed: 0.0,
            last_update: 0,
            comment: String::new(),
        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        
        let response: Value = self.client.get(&self.base_url).send()?.json()?;
        let call = Some(response[self.call_sign.clone()].clone());
        
        if call.is_some() && call.as_ref().unwrap().is_object(){
            // Extract the position data
        
            let call = &call.unwrap();
            self.lat = call["lat"].as_f64().unwrap();
            self.long = call["lon"].as_f64().unwrap();
            self.alt = call["alt"].as_f64().unwrap();
            let datetime = call["time_received"].as_str().unwrap();
            let datetime: DateTime<Utc> = datetime.parse().expect("Failed to parse datetime");
            self.last_update = datetime.timestamp() as u64;

            let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
            let age_seconds = current_time.saturating_sub(self.last_update);

            
            println!(
                "SondeHub Position: Call: {}, Lat: {}, Lon: {}, Alt: {}m, Last Update: {}s ago",
                self.call_sign, self.lat, self.long, self.alt, age_seconds
            );
            return Ok(());
        }

        Err("No SondeHub telemetry data found for this callsign".into())
    }

    pub fn get_position(&self) -> (f64, f64, f64) {
        (self.lat, self.long, self.alt)
    }

    pub fn get_speed(&self) -> f64 {
        self.ground_speed
    }

    pub fn get_last_update(&self) -> u64 {
        self.last_update
    }

    pub fn get_comment(&self) -> &str {
        &self.comment
    }
}