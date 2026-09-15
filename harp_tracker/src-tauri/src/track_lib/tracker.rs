use io::Write;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
// use serialport::{COMPort, SerialPort};

use crate::track_lib::{
    module::{ModuleDefinition, ModuleRegistry, ModuleSnapshot, TelemetryEvent},
    position_time::EstimationType,
    pred::sondhub_predictor::SondeHubPredictor,
};

use super::position_time::PositionTime;

pub struct Tracker {
    active: bool,
    registry: ModuleRegistry,
    predictor: Option<SondeHubPredictor>,
    position_time: PositionTime,
    csv_path: Option<PathBuf>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            active: false,
            registry: ModuleRegistry::new(),
            predictor: Some(SondeHubPredictor::new()),
            position_time: PositionTime {
                lat: 0.0,
                lon: 0.0,
                alt: 0.0,
                last_update: 0,
                horiz_vel: 0.0,
                vert_vel: 0.0,
            },
            csv_path: None,
        }
    }

    pub fn module_descriptors(&self) -> Vec<crate::track_lib::module::ModuleDescriptor> {
        self.registry.list()
    }

    pub fn ingest_event(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        self.registry.ingest(event)
    }

    fn update_tracker(&mut self) -> Vec<Result<(), String>> {
        self.registry.update_all()
    }

    /// Creates a data storage folder, if not already existing
    fn create_folder(&self) -> Option<PathBuf> {
        let folder_name = format!("Launch Data");

        let current_dir = std::env::current_dir().expect("Could not determine curxrent directory");
        let folder_path = current_dir.join(folder_name);

        fs::create_dir_all(&folder_path).expect("Unable  to create csv data directory");

        let file_path: PathBuf = folder_path.join(format!("data{:?}.csv", Utc::now().timestamp()));

        let mut f = File::create(&file_path).expect("Unable to create CSV file");

        f.write("track_type,lat,lon,alt,horiz_vel,vert_vel,time\n".as_bytes())
            .expect("Unable to write to CSV file");

        println!("Folder created at: {:?}", file_path);
        Some(file_path)
    }

    /// Function to write the data to csv
    fn write_to_csv(
        track_type: &str,
        pos_time: PositionTime,
        csv_path: Option<PathBuf>,
    ) -> io::Result<()> {
        let mut file = OpenOptions::new().append(true).open(csv_path.unwrap())?;
        writeln!(
            file,
            "{},{:.6},{:.6},{:.2},{:.2},{:.2},{}",
            track_type,
            pos_time.lat,
            pos_time.lon,
            pos_time.alt,
            pos_time.horiz_vel,
            pos_time.vert_vel,
            pos_time.last_update
        )?;
        Ok(())
    }

    // ------------------------Public Functions------------------------

    pub fn update(&mut self, method: EstimationType) -> Vec<Box<dyn std::error::Error>> {
        //Error collection to display to users (only soft errors)
        let mut err: Vec<Box<dyn std::error::Error>> = vec![];

        //Collect soft errors from update_tracker
        for opt in self.update_tracker() {
            if let Err(e) = opt {
                err.push(Box::new(io::Error::other(e)));
            }
        }

        let mut positions_src = self.registry.positions();
        positions_src.retain(|(position, _)| !(position.lat == 0.0 && position.lon == 0.0));
        for (position, module_type) in &positions_src {
            eprintln!(
                "{:?}",
                Self::write_to_csv(module_type, position.clone(), self.csv_path.clone())
            );
        }
        // Sort by time ascending
        positions_src.sort_by_key(|(p, _)| p.last_update);

        // Helper: haversine distance in meters
        fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
            let to_rad = |deg: f64| deg * std::f64::consts::PI / 180.0;
            let r = 6371000.0_f64; // Earth radius in meters
            let dlat = to_rad(lat2 - lat1);
            let dlon = to_rad(lon2 - lon1);
            let a = (dlat / 2.0).sin().powi(2)
                + to_rad(lat1).cos() * to_rad(lat2).cos() * (dlon / 2.0).sin().powi(2);
            let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
            r * c
        }

        // Compute velocities from previous point when missing; skip SondeHub if it's the first point
        let mut positions_for_filter: Vec<PositionTime> = Vec::new();
        for i in 0..positions_src.len() {
            let mut curr = positions_src[i].0.clone();
            let src = &positions_src[i].1;
            if i == 0 {
                // first point: if it's SondeHub and lacks velocities, skip it
                if src == "sondehub" && curr.horiz_vel == 0.0 && curr.vert_vel == 0.0 {
                    continue;
                }
                positions_for_filter.push(curr);
                continue;
            }

            let prev = &positions_src[i - 1].0;
            // Skip using previous position if it's (0,0) - invalid position
            if prev.lat != 0.0 || prev.lon != 0.0 {
                let dt = (curr.last_update as f64) - (prev.last_update as f64);
                if dt > 0.0 {
                    if curr.horiz_vel == 0.0 {
                        let dist = haversine_m(prev.lat, prev.lon, curr.lat, curr.lon);
                        curr.horiz_vel = dist / dt; // m/s
                    }
                    if curr.vert_vel == 0.0 {
                        curr.vert_vel = (curr.alt - prev.alt) / dt; // m/s
                    }
                }
            }

            positions_for_filter.push(curr);
        }

        // Collect best velocity data from all positions at the most recent timestamp
        let mut best_horiz_vel = 0.0;
        let mut best_vert_vel = 0.0;
        if let Some(most_recent_time) = positions_for_filter.last().map(|p| p.last_update) {
            for pos in &positions_for_filter {
                if pos.last_update == most_recent_time {
                    if pos.horiz_vel != 0.0 && best_horiz_vel == 0.0 {
                        best_horiz_vel = pos.horiz_vel;
                    }
                    if pos.vert_vel != 0.0 && best_vert_vel == 0.0 {
                        best_vert_vel = pos.vert_vel;
                    }
                }
            }
            eprintln!(
                "Best velocities from most recent timestamp: horiz={}, vert={}",
                best_horiz_vel, best_vert_vel
            );
        }

        let estimated_position: Option<PositionTime> =
            PositionTime::return_valid_pos_time(positions_for_filter, method);

        //Update struct and log to CSV if we have a new update
        if let Some(new_pos) = estimated_position.clone() {
            eprintln!("Updating position_time to: {:?}", new_pos);

            // Preserve existing velocities if new position has zero velocities
            let mut updated_pos = new_pos;

            // First try to use the best velocities from positions at the same timestamp
            if updated_pos.horiz_vel == 0.0 {
                if best_horiz_vel != 0.0 {
                    updated_pos.horiz_vel = best_horiz_vel;
                    eprintln!(
                        "Using best horiz_vel from same timestamp: {}",
                        updated_pos.horiz_vel
                    );
                } else if self.position_time.horiz_vel != 0.0 {
                    updated_pos.horiz_vel = self.position_time.horiz_vel;
                    eprintln!("Preserving previous horiz_vel: {}", updated_pos.horiz_vel);
                }
            }

            if updated_pos.vert_vel == 0.0 {
                if best_vert_vel != 0.0 {
                    updated_pos.vert_vel = best_vert_vel;
                    eprintln!(
                        "Using best vert_vel from same timestamp: {}",
                        updated_pos.vert_vel
                    );
                } else if self.position_time.vert_vel != 0.0 {
                    updated_pos.vert_vel = self.position_time.vert_vel;
                    eprintln!("Preserving previous vert_vel: {}", updated_pos.vert_vel);
                }
            }

            self.position_time = updated_pos;
        } else {
            eprintln!("No valid position found to update");
        }

        err
    }

    /// Function to print the data of the Tracker
    pub fn print(&self) {
        // Print current position data
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let age_seconds = current_time.saturating_sub(self.position_time.last_update);

        println!(
            "Latitude: {}, Longitude: {}, Altitude: {}, Last Update: {}s ago",
            self.position_time.lat, self.position_time.lon, self.position_time.alt, age_seconds
        );
    }

    // ------------------------Getter Functions------------------------

    pub fn get_position(&self) -> (f64, f64, f64) {
        return (
            self.position_time.lat,
            self.position_time.lon,
            self.position_time.alt,
        );
    }
    pub fn get_velocities(&self) -> (f64, f64) {
        (self.position_time.horiz_vel, self.position_time.vert_vel)
    }
    pub fn get_last_update(&self) -> u64 {
        self.position_time.last_update
    }

    pub fn csv_path(&self) -> Option<PathBuf> {
        self.csv_path.clone()
    }

    pub fn module_catalog(&self) -> Vec<ModuleDefinition> {
        self.registry.catalog()
    }

    pub fn module_snapshots(&self) -> Vec<ModuleSnapshot> {
        self.registry.snapshots()
    }

    pub fn configure_module(
        &mut self,
        module_type: &str,
        module_id: String,
        config: serde_json::Value,
    ) -> Result<(), String> {
        self.registry
            .create_module(module_type, module_id, config)?;
        if !self.active {
            self.csv_path = self.create_folder();
        }
        self.active = true;
        Ok(())
    }

    pub fn remove_module(&mut self, module_id: &str) -> bool {
        self.registry.remove(module_id)
    }
}
