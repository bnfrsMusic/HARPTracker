use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::track_lib::position_time::PositionTime;
use crate::track_lib::tracking_type::TrackingType;

#[derive(Clone)]
pub struct APRS {
    active: bool,
    tracking_type: TrackingType,
    api_key: String,
    base_url: String,
    call_sign: String,
    client: Client,
    position_time: PositionTime,
    vertical_velocity: f64,
    ground_speed: f64,
    datetime: f64,
    comment: String,
    symbol: String,
    path: String,
}

impl APRS {
    pub fn new(api_key: &str, call_sign: &str) -> Self {
        Self {
            active: true,
            tracking_type: TrackingType::APRS,
            api_key: api_key.to_string(),
            base_url: "https://api.aprs.fi/api".to_string(),
            call_sign: call_sign.to_string(),
            client: Client::new(),
            position_time: PositionTime {lat:0.0, lon:0.0, alt:0.0, last_update:0, horiz_vel:0.0, vert_vel:0.0},
            vertical_velocity: 0.0,
            ground_speed: 0.0,
            datetime: 0.0,
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
        let response: Value = self.client.get(&url).send().map_err(|e| {
            eprintln!("APRS API Request Error: {}", url);
            eprintln!("Error details: {}", e);
            Box::new(e) as Box<dyn std::error::Error>
        })?.json().map_err(|e| {
            eprintln!("Failed to parse APRS response from: {}", url);
            eprintln!("Error details: {}", e);
            Box::new(e) as Box<dyn std::error::Error>
        })?;

        // Check if request was successful
        if response["result"].as_str() != Some("ok") {
            eprintln!("APRS API URL: {}", url);
            eprintln!("APRS API Response: {}", response.to_string());
            return Err(format!("API error: {}", response["description"].as_str().unwrap_or("Unknown error")).into());
        }

        // any entries found?
        let found = response["found"].as_u64().unwrap_or(0);
        if found == 0 {
            eprintln!("APRS API URL: {}", url);
            eprintln!("APRS API Response: {}", response.to_string());
            return Err("No entries found for APRS".into());
        }

        // Extract the entries data
        if let Some(entries) = response["entries"].as_array() {
            if let Some(latest_entry) = entries.first() {
                eprintln!("DEBUG: APRS entry raw data: {}", latest_entry.to_string());
                
                // Extract position data
                let lat = latest_entry["lat"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                let lon = latest_entry["lng"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                // Try to extract altitude from various possible fields
                let mut alt = latest_entry["altitude"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if alt == 0.0 {
                    alt = latest_entry["alt"].as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .or_else(|| latest_entry["alt"].as_f64())
                        .unwrap_or(0.0);
                }
                
                // Try to parse altitude from comment field if it follows the format "A=XXXXX"
                if alt == 0.0 {
                    if let Some(comment_str) = latest_entry["comment"].as_str() {
                        // Look for altitude pattern like "A=046181" (in feet)
                        if let Some(a_pos) = comment_str.find("A=") {
                            let after_a = &comment_str[a_pos + 2..];
                            let alt_str: String = after_a.chars().take_while(|c| c.is_numeric()).collect();
                            if let Ok(alt_feet) = alt_str.parse::<f64>() {
                                alt = alt_feet * 0.3048; // Convert feet to meters
                                eprintln!("APRS: Parsed altitude from comment: {} ft -> {} m", alt_feet, alt);
                            }
                        }
                    }
                }
                
                if alt == 0.0 {
                    eprintln!("DEBUG: APRS altitude field not found in response, using 0.0");
                }
                
                self.ground_speed = latest_entry["speed"].as_f64()
                    .unwrap_or(0.0);
                
                // We dont have vertical velocity in APRS API
                self.vertical_velocity = 0.0;
                
                // Extract time data
                if let Some(time_str) = latest_entry["lasttime"].as_str() {
                    if let Ok(time) = time_str.parse::<f64>() {
                        self.datetime = time;
                        // update position_time with velocities
                        self.position_time.update(lat, lon, alt, time as u64, self.ground_speed, self.vertical_velocity);
                    } else {
                        eprintln!("APRS: Failed to parse lasttime '{}' as f64", time_str);
                        self.position_time.update(lat, lon, alt, 0, self.ground_speed, self.vertical_velocity);
                    }
                } else {
                    // fallback update even if time missing
                    eprintln!("APRS: lasttime field missing from API response");
                    self.position_time.update(lat, lon, alt, 0, self.ground_speed, self.vertical_velocity);
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
                
                let age_seconds = current_time.saturating_sub(self.position_time.last_update);

                
                println!(
                    "APRS Position: Call: {}, Lat: {}, Lon: {}, Alt: {}m, Speed: {} km/h, Last Update: {}s ago",
                    self.call_sign, self.position_time.lat, self.position_time.lon, self.position_time.alt, self.ground_speed, age_seconds
                );
                
                return Ok(());
            }
        }
        
        eprintln!("APRS API URL: {}", url);
        eprintln!("APRS API Response: {}", response.to_string());
        Err("Failed to parse position data from response".into())
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

    pub fn get_call_sign(&self) -> &str {
        &self.call_sign
    }
}