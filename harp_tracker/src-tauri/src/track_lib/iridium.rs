use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde_json::Value;

use crate::track_lib::module::{Module, ModuleDefinition, ModuleDescriptor, ModuleField, ModuleRegistration, ModuleStatus, TelemetryEvent};
use crate::track_lib::position_time::PositionTime;


#[derive(Clone)]
pub struct Iridium {
    active: bool,
    debug: bool,
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
            debug: false,
            base_url: base_url.to_string(),
            modem: modem.to_string(),
            client: Client::new(),
            position_time: PositionTime {lat:0.0, lon:0.0, alt:0.0, last_update:0, horiz_vel:0.0, vert_vel:0.0},
            vertical_velocity: 0.0,
            ground_speed: 0.0,

        }
    }

    pub fn update_position(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/meta/flights?modem_name={}",
            self.base_url, self.modem
        );

        if self.debug {
            eprintln!("[Iridium] GET {}", url);
        }

        //makes GET response to URL
        let response: Value = self.client.get(&url).send()?.json()?;

        //extracts the flights data
        let flights = response.as_array().ok_or("Invalid response")?;

        if let Some(latest_flight) = flights.last() {
            if let Some(uid) = latest_flight["uid"].as_str() {
                let flight_url = format!("{}/api/flight?uid={}", self.base_url, uid);
                if self.debug {
                    eprintln!("[Iridium] GET {}", flight_url);
                }
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
                        let lat = latest_entry[lat_idx].as_f64().unwrap_or(0.0);
                        let lon = latest_entry[lon_idx].as_f64().unwrap_or(0.0);
                        let alt = latest_entry[alt_idx].as_f64().unwrap_or(0.0);
                        self.vertical_velocity = latest_entry[vert_idx].as_f64().unwrap_or(0.0);
                        self.ground_speed = latest_entry[grnd_idx].as_f64().unwrap_or(0.0);
                        let dte = latest_entry[dte_idx].as_u64().unwrap_or(0);
                        self.position_time.update(lat, lon, alt, dte, self.ground_speed, self.vertical_velocity);

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
/// module for tracking Iridium position
impl Module for Iridium {
    fn id(&self) -> &str {
        &self.modem
    }

    fn name(&self) -> &str {
        "Iridium"
    }

    fn module_type(&self) -> &str {
        "iridium"
    }

    fn supports_source(&self, source: &str) -> bool {
        source.eq_ignore_ascii_case(&self.modem) || source.eq_ignore_ascii_case(self.name())
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
            id: self.modem.clone(),
            name: self.name().to_string(),
            enabled: self.active,
            connected: self.active && self.position_time.last_update != 0,
            last_update: if self.position_time.last_update != 0 { Some(self.position_time.last_update) } else { None },
            module_type: self.module_type().to_string(),
        }
    }
}

fn iridium_definition() -> ModuleDefinition {
    ModuleDefinition {
        module_type: "iridium".to_string(),
        display_name: "Iridium".to_string(),
        description: "Track an Iridium modem through the configured flight API.".to_string(),
        fields: vec![ModuleField { key: "modem".to_string(), label: "Modem ID".to_string(), field_type: "text".to_string(), required: true, secret: false, placeholder: Some("Modem ID".to_string()) }],
    }
}

fn create_iridium(id: String, config: Value) -> Result<Box<dyn Module>, String> {
    let modem = config.get("modem").and_then(Value::as_str).unwrap_or(&id);
    let base_url = config.get("base_url").and_then(Value::as_str).unwrap_or("https://borealis.rci.montana.edu");
    Ok(Box::new(Iridium::new(base_url, modem)))
}

inventory::submit! {
    ModuleRegistration { definition: iridium_definition, create: create_iridium }
}
