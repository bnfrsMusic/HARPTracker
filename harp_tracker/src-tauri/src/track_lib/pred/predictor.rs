use super::super::position_time::PositionTime;
use std::error::Error;

pub trait Predictor: Send + Sync {
    /// Predict trajectory based on current position and parameters
    fn predict(
        &self,
        current_pos: &PositionTime,
        params: &PredictionParams,
    ) -> Result<PredictionResult, Box<dyn Error>>;
    
    /// Get the name of this predictor
    fn name(&self) -> &str;
}

/// Common parameters for all prediction algorithms
#[derive(Clone, Debug)]
pub struct PredictionParams {
    pub payload_mass: f64,        // kg
    pub balloon_mass: f64,        // kg
    pub parachute_drag_coeff: f64,
    pub burst_altitude: f64,      // meters
    pub ascent_rate: Option<f64>, // m/s, calculated if None
    pub descent_rate: f64,        // m/s
}

impl Default for PredictionParams {
    fn default() -> Self {
        Self {
            payload_mass: 2.0,
            balloon_mass: 1.5,
            parachute_drag_coeff: 0.5,
            burst_altitude: 30000.0,
            ascent_rate: None,
            descent_rate: 5.0,
        }
    }
}

/// Common result structure for all predictors
#[derive(Clone, Debug)]
pub struct PredictionResult {
    pub ascent: Vec<PositionTime>,
    pub burst: Option<PositionTime>,
    pub landing: Option<PositionTime>,
    pub descent: Vec<PositionTime>,
}

/// Main prediction manager that handles all predictors
pub struct PredictionManager {
    current_predictor: String,
    params: PredictionParams,
    last_result: Option<PredictionResult>,
}

impl PredictionManager {
    pub fn new() -> Self {
        Self {
            current_predictor: "SondeHub".to_string(),
            params: PredictionParams::default(),
            last_result: None,
        }
    }
    
    /// Set which predictor to use
    pub fn set_predictor(&mut self, name: &str) {
        self.current_predictor = name.to_string();
    }
    
    /// Get current predictor name
    pub fn get_predictor(&self) -> &str {
        &self.current_predictor
    }
    
    /// Update prediction parameters
    pub fn set_params(&mut self, params: PredictionParams) {
        self.params = params;
    }
    
    /// Get current parameters
    pub fn get_params(&self) -> &PredictionParams {
        &self.params
    }
    
    /// Run prediction using the selected predictor
    pub fn run_prediction(
        &mut self,
        current_pos: &PositionTime,
        predictor: &dyn Predictor,
    ) -> Result<PredictionResult, Box<dyn Error>> {
        let result = predictor.predict(current_pos, &self.params)?;
        self.last_result = Some(result.clone());
        Ok(result)
    }
    
    /// Get the last prediction result
    pub fn get_last_result(&self) -> Option<&PredictionResult> {
        self.last_result.as_ref()
    }
}

impl Default for PredictionManager {
    fn default() -> Self {
        Self::new()
    }
}