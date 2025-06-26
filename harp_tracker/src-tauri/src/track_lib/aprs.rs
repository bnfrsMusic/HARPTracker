use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct APRS {
    active: bool,
    api_key: String,
    base_url: String,
    call_sign: String,
    client: Client,
    lat: f64,
    long: f64,
    alt: f64,
    vertical_velocity: f64,
    ground_speed: f64,
    datetime: f64,
    last_update: u64,
    comment: String,
    symbol: String,
    path: String,
}

impl APRS {
    pub fn new(api_key: &str, call_sign: &str) -> Self {
        Self {
            active: true,
            api_key: api_key.to_string(),
            base_url: "https://api.aprs.fi/api".to_string(),
            call_sign: call_sign.to_string(),
            client: Client::new(),
            lat: 0.0,
            long: 0.0,
            alt: 0.0,
            vertical_velocity: 0.0,
            ground_speed: 0.0,
            datetime: 0.0,
            last_update: 0,
            comment: String::new(),
            symbol: String::new(),
            path: String::new(),
        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/get?name={}&what=loc&apikey={}&format=json",
            self.base_url, self.call_sign, self.api_key
        );

        //make GET request to URL
        let response: Value = self.client.get(&url).send()?.json()?;

        // Check if request was successful
        if response["result"].as_str() != Some("ok") {
            return Err(format!("API error: {}", response["description"].as_str().unwrap_or("Unknown error")).into());
        }

        // any entries found?
        let found = response["found"].as_u64().unwrap_or(0);
        if found == 0 {
            return Err("No entries found for APRS".into());
        }

        // Extract the entries data
        if let Some(entries) = response["entries"].as_array() {
            if let Some(latest_entry) = entries.first() {
                // Extract position data
                self.lat = latest_entry["lat"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                self.long = latest_entry["lng"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                self.alt = latest_entry["altitude"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                self.ground_speed = latest_entry["speed"].as_f64()
                    // .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                // We dont have vertical velocity in APRS API (I dont think so anyway)
                self.vertical_velocity = 0.0;
                
                // Extract time data
                if let Some(time_str) = latest_entry["lasttime"].as_str() {
                    if let Ok(time) = time_str.parse::<f64>() {
                        self.datetime = time;
                        self.last_update = time as u64;
                    }
                }
                
                // Extract additional useful info
                self.comment = latest_entry["comment"].as_str().unwrap_or("").to_string();
                self.symbol = latest_entry["symbol"].as_str().unwrap_or("").to_string();
                self.path = latest_entry["path"].as_str().unwrap_or("").to_string();
                
                // Print current position data
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let age_seconds = current_time.saturating_sub(self.last_update);

                
                println!(
                    "APRS Position: Call: {}, Lat: {}, Lon: {}, Alt: {}m, Speed: {} km/h, Last Update: {}s ago",
                    self.call_sign, self.lat, self.long, self.alt, self.ground_speed, age_seconds
                );
                
                return Ok(());
            }
        }
        
        Err("Failed to parse position data from response".into())
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