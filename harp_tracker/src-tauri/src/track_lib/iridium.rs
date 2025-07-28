use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde_json::Value;

use crate::track_lib::position_time::PositionTime;
use crate::track_lib::tracking_type::TrackingType;


#[derive(Clone)]
pub struct Iridium {
    active: bool,
    tracking_type: TrackingType,
    base_url: String,
    modem: String,
    client: Client,
    position_time: PositionTime,
    vertical_velocity: f64,
    ground_speed: f64,
}

impl Iridium {
    pub fn new(base_url: &str, modem: &str) -> Self {
        Self {
            active: true,
            tracking_type: TrackingType::Iridium,
            base_url: base_url.to_string(),
            modem: modem.to_string(),
            client: Client::new(),
            position_time: PositionTime {lat:0.0, lon:0.0, alt:0.0, last_update:0},
            vertical_velocity: 0.0,
            ground_speed: 0.0,

        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/meta/flights?modem_name={}",
            self.base_url, self.modem
        );

        //makes GET response to URL
        let response: Value = self.client.get(&url).send()?.json()?;

        //extracts the flights data
        let flights = response.as_array().ok_or("Invalid response")?;

        if let Some(latest_flight) = flights.last() {
            if let Some(uid) = latest_flight["uid"].as_str() {
                let flight_url = format!("{}/api/flight?uid={}", self.base_url, uid);
                let flight_data: Value = self.client.get(&flight_url).send()?.json()?;

                if let Some(data) = flight_data["data"].as_array() {
                    if let Some(latest_entry) = data.last() {
                        //extracts the current latitude, longitude, and altitude
                        let fields = flight_data["fields"].as_array().ok_or("Missing fields")?;
                        let lat_idx = fields.iter().position(|v| v == "latitude").unwrap();
                        let lon_idx = fields.iter().position(|v| v == "longitude").unwrap();
                        let alt_idx = fields.iter().position(|v| v == "altitude").unwrap();
                        let vert_idx = fields
                            .iter()
                            .position(|v| v == "vertical_velocity")
                            .unwrap();
                        let grnd_idx = fields.iter().position(|v| v == "ground_speed").unwrap();
                        let dte_idx = fields.iter().position(|v| v == "datetime").unwrap();

                        //Set the values
                        self.position_time.lat = latest_entry[lat_idx].as_f64().unwrap();
                        self.position_time.lon = latest_entry[lon_idx].as_f64().unwrap();
                        self.position_time.alt = latest_entry[alt_idx].as_f64().unwrap();
                        self.vertical_velocity = latest_entry[vert_idx].as_f64().unwrap();
                        self.ground_speed = latest_entry[grnd_idx].as_f64().unwrap();
                        self.position_time.last_update = latest_entry[dte_idx].as_u64().unwrap();

                        let current_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        
                        let age_seconds = current_time.saturating_sub(self.position_time.last_update);
                        println!(
                            "Iridium Position: Lat: {}, Lon: {}, Alt: {}m, Vertical Velocity: {}m/s, Ground Speed: {}m/s, Last Update: {}s ago",
                            self.position_time.lat, self.position_time.lon, self.position_time.alt, self.vertical_velocity, self.ground_speed, age_seconds
                        );
                    }
                }
            }
        }
        Ok(())
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

}
