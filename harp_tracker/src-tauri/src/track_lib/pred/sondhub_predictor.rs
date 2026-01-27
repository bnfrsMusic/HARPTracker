use super::super::position_time::PositionTime;
use super::predictor::{Predictor, PredictionParams, PredictionResult};
use std::error::Error;
use std::fs::File;
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_urlencoded;
use chrono::DateTime;

#[derive(Clone, Debug)]
pub struct SondeHubPredictor {
    /// Historical positions loaded from CSV
    pub history: Vec<HistoricalPosition>,
    /// API endpoint for predictions
    pub api_endpoint: String,
    /// Reusable HTTP client
    client: reqwest::blocking::Client,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HistoricalPosition {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    #[serde(alias = "time", alias = "timestamp")]
    pub timestamp: u64,
}

#[derive(Debug, Serialize)]
struct PredictionRequest {
    profile: String,
    launch_latitude: f64,
    launch_longitude: f64,
    launch_altitude: f64,
    launch_datetime: String,
    ascent_rate: f64,
    burst_altitude: f64,
    descent_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    v_speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    h_speed: Option<f64>,
}

// --- API RESPONSE STRUCTS ---

// 1. The Root wrapper (The API returns an object with a "prediction" key)
#[derive(Debug, Deserialize, Clone)]
struct TawhiriResponse {
    prediction: Vec<StageTrajectory>,
}

// 2. The Stage wrapper
#[derive(Debug, Deserialize, Clone)]
struct StageTrajectory {
    stage: String,
    trajectory: Vec<TawhiriPoint>, 
}

// 3. The Point struct (Matches API field names exactly)
#[derive(Debug, Deserialize, Clone)]
struct TawhiriPoint {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    datetime: String,
}

impl SondeHubPredictor {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            api_endpoint: "https://api.v2.sondehub.org/tawhiri".to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            history: Vec::new(),
            api_endpoint: endpoint,
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn load_history(&mut self, csv_path: &str) -> Result<(), Box<dyn Error>> {
        let file = File::open(csv_path)?;
        let mut rdr = ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(file);

        self.history.clear();
        
        for result in rdr.deserialize() {
            let record: HistoricalPosition = result?;
            self.history.push(record);
        }

        Ok(())
    }

    pub fn get_latest_position(&self) -> Option<PositionTime> {
        self.history.last().map(|h| PositionTime {
            lat: h.lat,
            lon: h.lon,
            alt: h.alt,
            last_update: h.timestamp,
            horiz_vel: 0.0,
            vert_vel: 0.0,
        })
    }

    pub fn calculate_ascent_rate(&self) -> Option<f64> {
        if self.history.len() < 2 {
            return None;
        }

        let mut total_rate = 0.0;
        let mut count = 0;

        for i in 1..self.history.len() {
            let prev = &self.history[i - 1];
            let curr = &self.history[i];
            
            let dt = (curr.timestamp - prev.timestamp) as f64;
            if dt > 0.0 {
                let dh = curr.alt - prev.alt;
                total_rate += dh / dt;
                count += 1;
            }
        }

        if count > 0 {
            Some(total_rate / count as f64)
        } else {
            None
        }
    }

    fn predict_internal(
        &self,
        current_pos: &PositionTime,
        ascent_rate: f64,
        burst_alt: f64,
        descent_rate: f64,
        v_speed: Option<f64>,
        h_speed: Option<f64>,
    ) -> Result<PredictionResult, Box<dyn Error>> {
        
        let datetime = chrono::DateTime::from_timestamp(current_pos.last_update as i64, 0)
            .unwrap_or_else(|| panic!("Invalid timestamp"))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        let mut launch_lon = current_pos.lon;
        if launch_lon < 0.0 {
            launch_lon = 360.0 + launch_lon;
        }

        let request = PredictionRequest {
            profile: "standard_profile".to_string(),
            launch_latitude: current_pos.lat,
            launch_longitude: launch_lon,
            launch_altitude: current_pos.alt,
            launch_datetime: datetime.clone(),
            ascent_rate,
            burst_altitude: burst_alt,
            descent_rate,
            v_speed,
            h_speed,
        };

        println!("DEBUG: Sending request to {}", self.api_endpoint);
        let full_url = format!("{}?{}", self.api_endpoint, serde_urlencoded::to_string(&request).unwrap());
        println!("DEBUG: Full URL: {}", full_url);

        let response = self.client
            .get(&self.api_endpoint)
            .query(&request)
            .send()?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().unwrap_or_else(|_| "Could not read error body".to_string());
            return Err(format!("API request failed: {} - {}", status, error_body).into());
        }

        // --- FIXED DESERIALIZATION ---
        // Decode into the wrapper struct first
        let tawhiri_resp: TawhiriResponse = response.json()?;
        let pred_stages = tawhiri_resp.prediction;

        let mut ascent = Vec::new();
        let mut descent = Vec::new();
        let mut burst: Option<PositionTime> = None;
        let mut landing: Option<PositionTime> = None;

        for stage in &pred_stages {
            let is_ascent = stage.stage.to_lowercase().contains("ascent");
            let is_descent = stage.stage.to_lowercase().contains("descent");

            for point in &stage.trajectory {
                // Normalize longitude
                let mut lon = point.longitude;
                if lon > 180.0 {
                    lon = lon - 360.0;
                }

                // Parse the datetime string to u64 timestamp
                let ts = match DateTime::parse_from_rfc3339(&point.datetime) {
                    Ok(dt) => dt.timestamp() as u64,
                    Err(_) => 0,
                };

                // Map TawhiriPoint to internal PositionTime
                let pt = PositionTime {
                    lat: point.latitude,
                    lon: lon,
                    alt: point.altitude,
                    last_update: ts,
                    horiz_vel: 0.0,
                    vert_vel: 0.0,
                };

                if is_ascent {
                    ascent.push(pt.clone());
                } else if is_descent {
                    if burst.is_none() {
                        burst = Some(pt.clone());
                    }
                    descent.push(pt.clone());
                }
            }
        }

        landing = descent.last().cloned();

        Ok(PredictionResult { ascent, burst, descent, landing })
    }
}

impl Predictor for SondeHubPredictor {
    fn predict(
        &self,
        current_pos: &PositionTime,
        params: &PredictionParams,
    ) -> Result<PredictionResult, Box<dyn Error>> {
        let ascent_rate = params.ascent_rate.unwrap_or_else(|| {
            self.calculate_ascent_rate().unwrap_or(5.0)
        });

        let v_speed = if current_pos.vert_vel != 0.0 {
            Some(current_pos.vert_vel)
        } else {
            None
        };

        let h_speed = if current_pos.horiz_vel != 0.0 {
            Some(current_pos.horiz_vel)
        } else {
            None
        };

        self.predict_internal(
            current_pos,
            ascent_rate,
            params.burst_altitude,
            params.descent_rate,
            v_speed,
            h_speed,
        )
    }

    fn name(&self) -> &str {
        "SondeHub"
    }
}

impl Default for SondeHubPredictor {
    fn default() -> Self {
        Self::new()
    }
}