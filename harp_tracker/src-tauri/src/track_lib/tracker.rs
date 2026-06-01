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
    position_time::EstimationType, pred::sondhub_predictor::SondeHubPredictor,
    tracking_type::TrackingType,
};

use super::{
    aprs::APRS, iridium::Iridium, position_time::PositionTime, sondehub::SondeHub, wspr::WSPR,
};

pub struct Tracker {
    active: bool,

    aprs: Vec<Option<APRS>>,
    iridium: Vec<Option<Iridium>>,
    sondehub: Vec<Option<SondeHub>>,
    wspr: Vec<Option<WSPR>>,
    //Arduino and calculations modules have been commented out or removed to be worked on in the future
    // arduino: Option<Arduino>,
    // tracking: bool,
    predictor: Option<SondeHubPredictor>,

    position_time: PositionTime,
    csv_path: Option<PathBuf>,
}

impl Tracker {
    // ------------------------Initializing Functions------------------------

    /// Create a new Tracker
    pub fn new() -> Self {
        Self {
            active: false,
            aprs: vec![],
            iridium: vec![],
            sondehub: vec![],
            wspr: vec![],
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

    /// Create a new APRS Module
    pub fn new_aprs(&mut self, api_key: &str, call_sign: &str) {
        self.aprs.push(Some(APRS::new(api_key, call_sign)));
        // Also create a corresponding SondeHub
        self.sondehub.push(Some(SondeHub::new(call_sign)));
        if !self.active {
            self.csv_path = self.create_folder()
        }
        self.active = true;
    }

    /// Create a new Iridium Module
    pub fn new_iridium(&mut self, base_url: &str, modem: &str) {
        self.iridium.push(Some(Iridium::new(base_url, modem)));
        if !self.active {
            self.csv_path = self.create_folder()
        }
        self.active = true;
    }

    /// Create a new SondeHub Module
    pub fn new_sondehub(&mut self, call_sign: &str) {
        self.sondehub.push(Some(SondeHub::new(call_sign)));
        if !self.active {
            self.csv_path = self.create_folder()
        }
        self.active = true;
    }

    /// Create a new WSPR Module
    pub fn new_wspr(&mut self, call_sign: &str) {
        self.wspr.push(Some(WSPR::new(call_sign)));
        // Also create a corresponding SondeHub for WSPR
        self.sondehub.push(Some(SondeHub::new_with_type(call_sign, "WSPR")));
        if !self.active {
            self.csv_path = self.create_folder()
        }
        self.active = true;
    }

    // /// Create a new Arduino Module [In Progress]
    // pub fn new_arduino(&mut self, serial: Option<Arc<Mutex<Box<dyn SerialPort + Send>>>>,com: Option<COMPort>){
    //     self.arduino = Some(Arduino::new(serial, com));
    // }

    // /// Setup Arduino
    // pub fn setup_arduino(&mut self) -> Result<(), Box<dyn std::error::Error>>{
    //     self.arduino.as_mut().unwrap().setup()
    // }

    /// Function to set tracking on or off
    // pub fn set_tracking(&mut self, val: bool){
    //     self.arduino.as_mut().unwrap().set_tracking(val);
    // }

    // ------------------------Tracking Modules Return Functions------------------------

    pub fn return_aprs(&mut self) -> Vec<Option<APRS>> {
        self.aprs.clone()
    }
    pub fn return_iridium(&mut self) -> Vec<Option<Iridium>> {
        self.iridium.clone()
    }
    pub fn return_sondehub(&mut self) -> Vec<Option<SondeHub>> {
        self.sondehub.clone()
    }
    pub fn return_wspr(&mut self) -> Vec<Option<WSPR>> {
        self.wspr.clone()
    }
    // pub fn is_arduino_active(&self) -> bool {
    //     if self.aprs.is_some(){
    //         return self.arduino.as_ref().unwrap().active;
    //     }
    //     return false;
    // }

    // ------------------------Update Helper Functions------------------------

    fn update_aprs(&mut self) -> Vec<Result<(), Box<dyn std::error::Error + 'static>>> {
        if self.aprs.len() > 0 {
            let mut e = vec![];

            for aprs in &mut self.aprs {
                if aprs.is_some() {
                    e.push(aprs.as_mut().unwrap().update_position());
                }
            }
            return e;
        }
        // Ok(())
        return vec![Err("Error updating APRS".into())];
    }

    fn update_iridium(&mut self) -> Vec<Result<(), Box<dyn std::error::Error + 'static>>> {
        let mut e = vec![];
        for iridium in &mut self.iridium {
            if iridium.is_some() {
                e.push(iridium.as_mut().unwrap().update_position());
            }
        }
        if e.is_empty() {
            e.push(Err("Error updating Iridium".into()));
        }
        e
    }

    fn update_wspr(&mut self) -> Vec<Result<(), Box<dyn std::error::Error + 'static>>> {
        let mut e: Vec<Result<(), Box<dyn std::error::Error + 'static>>> = vec![];
        for wspr in &mut self.wspr {
            if wspr.is_some() {
                e.push(
                    wspr.as_mut()
                        .unwrap()
                        .update_position()
                        .map_err(|e| e as Box<dyn std::error::Error + 'static>),
                );
            }
        }
        if e.is_empty() {
            e.push(Err("Error updating WSPR".into()));
        }
        e
    }

    fn update_sondehub(&mut self) -> Vec<Result<(), Box<dyn std::error::Error + 'static>>> {
        let mut e = vec![];
        for sondehub in &mut self.sondehub {
            if sondehub.is_some() {
                e.push(sondehub.as_mut().unwrap().update_position());
            }
        }
        if e.is_empty() {
            e.push(Err("Error updating Sondehub".into()));
        }
        e
    }

    // fn update_arduino(&mut self) -> Result<(), Box<(dyn std::error::Error + 'static)>>{
    //     if self.arduino.is_some(){
    //         return self.arduino.as_mut().unwrap().update();
    //     }
    //     // Ok(())
    //     return Err("Error updating Arduino".into());
    // }

    /// Internal function to update all modules on the tracker and return the errors as a vector
    fn update_tracker(&mut self) -> Vec<Result<(), Box<dyn std::error::Error + 'static>>> {
        let mut v: Vec<Result<(), Box<dyn std::error::Error + 'static>>> = vec![];

        // Ensure APRS runs first so SondeHub updates can use APRS-provided velocities
        v.extend(self.update_aprs());
        v.extend(self.update_sondehub());
        v.extend(self.update_iridium());
        v.extend(self.update_wspr());
        // v.push(self.update_arduino());

        v
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
        track_type: TrackingType,
        pos_time: PositionTime,
        csv_path: Option<PathBuf>,
    ) -> io::Result<TrackingType> {
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
        Ok(track_type)
    }

    // ------------------------Public Functions------------------------

    pub fn update(&mut self) -> Vec<Box<dyn std::error::Error>> {
        //Error collection to display to users (only soft errors)
        let mut err: Vec<Box<dyn std::error::Error>> = vec![];

        //Collect soft errors from update_tracker
        for opt in self.update_tracker() {
            if let Err(e) = opt {
                err.push(e);
            }
        }
        
        let mut positions_src: Vec<(PositionTime, TrackingType)> = vec![];
        eprintln!("DEBUG: APRS vector length: {}", self.aprs.len());
        if self.aprs.len() > 0 {
            for (idx, aprs) in self.aprs.iter().enumerate() {
                eprintln!("DEBUG: APRS[{}] is_some: {}", idx, aprs.is_some());
                if let Some(aprs) = aprs {
                    let t = aprs.get_last_update();
                    eprintln!("DEBUG: APRS[{}] last_update: {}", idx, t);
                    if t != 0 {
                        let pt = aprs.get_pos_time();
                        eprintln!("DEBUG: APRS[{}] pos_time: {:?}", idx, pt);
                        //skip positions with (0.0, 0.0, 0.0) (invalid/startup positions)
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("DEBUG: Skipping APRS[{}] - invalid zero coordinates (0,0,0)", idx);
                            continue;
                        }
                        positions_src.push((pt.clone(), TrackingType::APRS));
                        eprintln!(
                            "{:?}",
                            Self::write_to_csv(TrackingType::APRS, pt, self.csv_path.clone())
                        );
                    }
                }
            }
        }

        eprintln!("DEBUG: SondeHub vector length: {}", self.sondehub.len());
        if self.sondehub.len() > 0 {
            for (idx, sondehub) in self.sondehub.iter().enumerate() {
                eprintln!("DEBUG: SondeHub[{}] is_some: {}", idx, sondehub.is_some());
                if let Some(sondehub) = sondehub {
                    let t = sondehub.get_last_update();
                    eprintln!("DEBUG: SondeHub[{}] last_update: {}", idx, t);
                    if t != 0 {
                        let pt = sondehub.get_pos_time();
                        eprintln!("DEBUG: SondeHub[{}] pos_time: {:?}", idx, pt);
                        //skip SondeHub positions that are all zeros (0,0,0)
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("DEBUG: Skipping SondeHub[{}] - invalid zero coordinates (0,0,0)", idx);
                            continue;
                        }
                        positions_src.push((pt.clone(), TrackingType::SondeHub));
                        eprintln!(
                            "{:?}",
                            Self::write_to_csv(TrackingType::SondeHub, pt, self.csv_path.clone())
                        );
                    }
                }
            }
        }

        if self.wspr.len() > 0 {
            for (idx, wspr) in self.wspr.iter().enumerate() {
                eprintln!("DEBUG: WSPR[{}] is_some: {}", idx, wspr.is_some());
                if let Some(wspr) = wspr {
                    let t = wspr.get_last_update();
                    eprintln!("DEBUG: WSPR[{}] last_update: {}", idx, t);
                    if t != 0 {
                        let pt = wspr.get_pos_time();
                        eprintln!("DEBUG: WSPR[{}] pos_time: {:?}", idx, pt);
                        //skip WSPR positions with lat=0 AND lon=0 (invalid/startup positions)
                        if pt.lat == 0.0 && pt.lon == 0.0 {
                            eprintln!("DEBUG: Skipping WSPR[{}] - invalid zero coordinates (0,0)", idx);
                            continue;
                        }
                        positions_src.push((pt.clone(), TrackingType::WSPR));
                        eprintln!(
                            "{:?}",
                            Self::write_to_csv(TrackingType::WSPR, pt, self.csv_path.clone())
                        );
                    }
                }
            }
        }

        if self.iridium.len() > 0 {
            for (idx, iridium) in self.iridium.iter().enumerate() {
                eprintln!("DEBUG: Iridium[{}] is_some: {}", idx, iridium.is_some());
                if let Some(iridium) = iridium {
                    let t = iridium.get_last_update();
                    eprintln!("DEBUG: Iridium[{}] last_update: {}", idx, t);
                    if t != 0 {
                        let pt = iridium.get_pos_time();
                        eprintln!("DEBUG: Iridium[{}] pos_time: {:?}", idx, pt);
                        //skip positions with (0.0, 0.0, 0.0) (invalid/startup positions)
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("DEBUG: Skipping Iridium[{}] - invalid zero coordinates (0,0,0)", idx);
                            continue;
                        }
                        positions_src.push((pt.clone(), TrackingType::Iridium));
                        eprintln!(
                            "{:?}",
                            Self::write_to_csv(TrackingType::Iridium, pt, self.csv_path.clone())
                        );
                    }
                }
            }
        }

        eprintln!("Collected {} positions for filtering", positions_src.len());

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
            let src = positions_src[i].1;
            if i == 0 {
                // first point: if it's SondeHub and lacks velocities, skip it
                if src == TrackingType::SondeHub && curr.horiz_vel == 0.0 && curr.vert_vel == 0.0 {
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

        let most_recent_position: Option<PositionTime> =
            PositionTime::return_valid_pos_time(positions_for_filter, EstimationType::Recent);

        //Update struct and log to CSV if we have a new update
        if let Some(new_pos) = most_recent_position.clone() {
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
    pub fn get_position_with_filtering(&self, method: EstimationType) -> (f64, f64, f64, f64, f64) {
        let mut positions: Vec<PositionTime> = vec![];
        eprintln!("DEBUG: get_position_with_filtering() collecting positions...");
        
        if self.aprs.len() > 0 {
            for aprs in &self.aprs {
                if let Some(aprs) = aprs {
                    let t = aprs.get_last_update();
                    if t != 0 {
                        let pt = aprs.get_pos_time();
                        // Skip positions with (0.0, 0.0, 0.0)
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("  Skipping APRS position (zero coords)");
                            continue;
                        }
                        positions.push(pt);
                        eprintln!("  Added APRS position");
                    }
                }
            }
        }

        if self.iridium.len() > 0 {
            for iridium in &self.iridium {
                if let Some(iridium) = iridium {
                    let t = iridium.get_last_update();
                    if t != 0 {
                        let pt = iridium.get_pos_time();
                        // Skip positions with (0.0, 0.0, 0.0)
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("  Skipping Iridium position (zero coords)");
                            continue;
                        }
                        positions.push(pt);
                        eprintln!("  Added Iridium position");
                    }
                }
            }
        }

        if self.sondehub.len() > 0 {
            for sondehub in &self.sondehub {
                if let Some(sondehub) = sondehub {
                    let t = sondehub.get_last_update();
                    if t != 0 {
                        let pt = sondehub.get_pos_time();
                        // Skip SondeHub positions that are all zeros (0,0,0) - no data found
                        if pt.lat == 0.0 && pt.lon == 0.0 && pt.alt == 0.0 {
                            eprintln!("  Skipping SondeHub position (zero coords)");
                            continue;
                        }
                        // Skip positions with zero altitude (will use other sources)
                        if pt.alt == 0.0 {
                            eprintln!("  Skipping SondeHub position (zero altitude)");
                            continue;
                        }
                        eprintln!("  Added SondeHub position: ({}, {})", pt.lat, pt.lon);
                        positions.push(pt);
                    }
                }
            }
        }

        if self.wspr.len() > 0 {
            for wspr in &self.wspr {
                if let Some(wspr) = wspr {
                    let t = wspr.get_last_update();
                    if t != 0 {
                        let pt = wspr.get_pos_time();
                        // Skip WSPR positions with (0.0, 0.0) - invalid position
                        if pt.lat == 0.0 && pt.lon == 0.0 {
                            eprintln!("  Skipping WSPR position (zero coords)");
                            continue;
                        }
                        // Skip positions with zero altitude (WSPR doesn't provide altitude, use other sources)
                        if pt.alt == 0.0 {
                            eprintln!("  Skipping WSPR position (zero altitude)");
                            continue;
                        }
                        eprintln!("  Added WSPR position: ({}, {})", pt.lat, pt.lon);
                        positions.push(pt);
                    }
                }
            }
        }

        eprintln!("DEBUG: Collected {} total positions for filtering", positions.len());

        if let Some(filtered_pos) = PositionTime::return_valid_pos_time(positions, method) {
            eprintln!("DEBUG: Filtered result: ({}, {})", filtered_pos.lat, filtered_pos.lon);
            (
                filtered_pos.lat,
                filtered_pos.lon,
                filtered_pos.alt,
                filtered_pos.horiz_vel,
                filtered_pos.vert_vel,
            )
        } else {
            eprintln!("DEBUG: No valid filtered position, returning raw position_time: ({}, {})", self.position_time.lat, self.position_time.lon);
            (
                self.position_time.lat,
                self.position_time.lon,
                self.position_time.alt,
                self.position_time.horiz_vel,
                self.position_time.vert_vel,
            )
        }
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
        return self.position_time.last_update;
    }
}
