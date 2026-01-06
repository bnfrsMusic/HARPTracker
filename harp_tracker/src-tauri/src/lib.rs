
pub mod track_lib;

// Imports
use chrono::Utc;
use once_cell::sync::Lazy;
use track_lib::tracker::Tracker;
use std::{sync::Mutex, time::{SystemTime, UNIX_EPOCH}};
use dotenvy::dotenv;
use std::env;
use std::fs;
use serde::{Serialize, Deserialize};

pub struct Coords {
    lat: f64,
    long: f64,
    alt: f64 
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrackingPoint {
    lat: f64,
    lon: f64,
    alt: f64,
    time: u64,
    track_type: String
}

impl Coords {
    pub fn new() -> Coords {
        Self { lat: 0.0, long: 0.0, alt: 0.0 }
    }
    
    pub fn update(&mut self, pos: (f64, f64, f64)) {
        self.lat = pos.0;
        self.long = pos.1;
        self.alt = pos.2;
    }
}

// Globals
pub static IRIDIUM_MODEM: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
pub static APRS_CALLSIGN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
pub static TRACKER: Lazy<Mutex<Tracker>> = Lazy::new(|| Mutex::new(Tracker::new()));
pub static LOCATION: Lazy<Mutex<Coords>> = Lazy::new(|| Mutex::new(Coords::new()));
pub static FILTERING_METHOD: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::from("Average")));

//API Keys
pub static APRSFI_API_KEY: Lazy<String> = Lazy::new(|| {
    dotenv().ok(); // Load environment variables from `.env`
    env::var("APRSFI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: APRS_KEY not set in .env file!");
        String::new()
    })
});


// Return the current UTC time formatted
#[tauri::command]
fn utc() -> String {
    format!(
        "UTC {}",
        Utc::now()
            .time()
            .to_string()
            .get(0..10)
            .expect("INVALID UTC")
    )
}

// Return the current date
#[tauri::command]
fn date() -> String {
    format!("Date: {}", Utc::now().date_naive().to_string())
}

// Set the Iridium modem ID
#[tauri::command]
fn set_irr_modem(id: String) {
    println!("Setting IRIDIUM_MODEM to: {}", id);
    *IRIDIUM_MODEM.lock().unwrap() = id;
}

// Get the Iridium modem ID
#[tauri::command]
fn get_irr_modem() -> String {
    let modem = IRIDIUM_MODEM.lock().unwrap().to_string();
    println!("IRIDIUM MODEM: {}", modem);
    modem
}

// Set the APRS callsign
#[tauri::command]
fn set_aprs_callsign(id: String) {
    println!("Setting APRS_CALLSIGN to: {}", id);
    *APRS_CALLSIGN.lock().unwrap() = id;
}

// Get the APRS callsign
#[tauri::command]
fn get_aprs_callsign() -> String {
    let callsign = APRS_CALLSIGN.lock().unwrap().to_string();
    println!("APRS CALLSIGN: {}", callsign);
    callsign
}

// Init APRS with current callsign
#[tauri::command]
fn set_aprs() -> bool {
    let aprs_call = APRS_CALLSIGN.lock().unwrap();
    if !aprs_call.is_empty() {
        TRACKER.lock().unwrap().new_aprs(APRSFI_API_KEY.as_str(), aprs_call.as_str());

        true
    } else {
        false
    }
}

///Create the Arduino Module and collect errors into String
// #[tauri::command]
// fn set_arduino() -> String{
//     let mut tracker =TRACKER.lock().unwrap();
    
//     if !tracker.is_arduino_active(){
//         TRACKER.lock().unwrap().new_arduino(None, None);
//         let t= TRACKER.lock().unwrap().setup_arduino();
//         let mut r = String::new();
//         for err in t {
//             r = format!("{}\nERROR: {:?}\n", r, err);
//         }
//         return r;
//     }

//     "".into()
// }

// Init Iridium modem with current ID
#[tauri::command]
fn set_iridium() -> bool {
    let modem = IRIDIUM_MODEM.lock().unwrap();
    if !modem.is_empty() {
        println!("Setting up iridium with modem: {}", modem);
        TRACKER.lock().unwrap().new_iridium("https://borealis.rci.montana.edu", modem.as_str());
        true
    } else {
        println!("Cannot set up iridium: modem is empty");
        false
    }
}

// Update tracker position
#[tauri::command]
fn update() -> String {
    let t = TRACKER.lock().unwrap().update();
    let mut r = String::new();
    for err in t {
        r = format!("{}\nERROR: {:?}\n", r, err);
    }
    
    let filtering_method = FILTERING_METHOD.lock().unwrap().clone();
    let estimation_type = match filtering_method.as_str() {
        "Average" => track_lib::position_time::EstimationType::Average,
        "Median" => track_lib::position_time::EstimationType::Median,
        "Recent" => track_lib::position_time::EstimationType::Recent,
        _ => track_lib::position_time::EstimationType::Recent,
    };
    
    let pos = TRACKER.try_lock().unwrap().get_position();
    let pos_filtered = TRACKER.try_lock().unwrap().get_position_with_filtering(estimation_type);
    LOCATION.try_lock().unwrap().update((
        (pos_filtered.0 * 1000.0).round() / 1000.0,
        (pos_filtered.1 * 1000.0).round() / 1000.0,
        (pos_filtered.2 * 1000.0).round() / 1000.0
    ));
    
    println!("Update result: {}", r);
    println!("Raw position: {:?}, Filtered position: {:?}", pos, pos_filtered);
    r
}

// Get full position
#[tauri::command]
fn get_position() -> (f64, f64, f64) {
    let (l1, l2, alt) = TRACKER.try_lock().unwrap().get_position();
    println!("LOCATION: {}, {}, {}", l1, l2, alt);
    (
        (l1 * 1000.0).round() / 1000.0,
        (l2 * 1000.0).round() / 1000.0,
        (alt * 1000.0).round() / 1000.0
    )
}

// Get latitude
#[tauri::command]
fn get_lat() -> f64 {
    LOCATION.lock().unwrap().lat
}

// Get longitude
#[tauri::command]
fn get_long() -> f64 {
    LOCATION.lock().unwrap().long
}

// Get altitude
#[tauri::command]
fn get_alt() -> f64 {
    LOCATION.lock().unwrap().alt
}

// Get time since last update in seconds
#[tauri::command]
fn get_last_update() -> u64 {
    let last = TRACKER.try_lock().unwrap().get_last_update();
    if last != 0 {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        current_time.saturating_sub(last)
    } else {
        0
    }
}

//Returns if APRS is currently active
#[tauri::command]
fn is_aprs_active() -> bool{
    let a = TRACKER.try_lock().unwrap().return_aprs();
    let s = TRACKER.try_lock().unwrap().return_sondehub();

    for aprs in a{
        if aprs.is_some(){
            let aprs_unwrapped = aprs.unwrap();
            if aprs_unwrapped.get_last_update() != 0{
                return true
            }
        }
    }
    for sondehub in s {
        if sondehub.is_some(){
            let s = sondehub.unwrap();
            if s.get_last_update() != 0{
                return true
            }
        }
    }
    return false;
}

//Returns if Iridium is currently active
#[tauri::command]
fn is_iridium_active() -> bool{
    let a = TRACKER.try_lock().unwrap().return_iridium();
    for iridium in a {
        if iridium.is_some(){
            let a = iridium.unwrap();
            if a.get_last_update() != 0{
                return true
            }
        }
    }
    return false;
}

// Get current filtering method
#[tauri::command]
fn get_filtering_method() -> String {
    FILTERING_METHOD.lock().unwrap().clone()
}

// Set filtering method
#[tauri::command]
fn set_filtering_method(method: String) {
    *FILTERING_METHOD.lock().unwrap() = method;
}

/// count of active APRS instances
#[tauri::command]
fn get_aprs_count() -> usize {
    TRACKER.try_lock().unwrap().return_aprs().iter().filter(|a| a.is_some()).count()
}

/// count of active Iridium instances
#[tauri::command]
fn get_iridium_count() -> usize {
    TRACKER.try_lock().unwrap().return_iridium().iter().filter(|i| i.is_some()).count()
}

// count of active SondeHub instances
#[tauri::command]
fn get_sondehub_count() -> usize {
    TRACKER.try_lock().unwrap().return_sondehub().iter().filter(|s| s.is_some()).count()
}

/// Check if APRS instances have legit position data
#[tauri::command]
fn get_aprs_validity() -> Vec<bool> {
    TRACKER.try_lock().unwrap().return_aprs().iter().map(|a| {
        if let Some(aprs) = a {
            aprs.get_last_update() != 0
        } else {
            false
        }
    }).collect()
}

/// Check if Iridium instances have legit position data
#[tauri::command]
fn get_iridium_validity() -> Vec<bool> {
    TRACKER.try_lock().unwrap().return_iridium().iter().map(|i| {
        if let Some(iridium) = i {
            iridium.get_last_update() != 0
        } else {
            false
        }
    }).collect()
}

///Read most recent CSV file from the Launch Data folder and return tracking points
#[tauri::command]
fn get_tracking_history() -> Vec<TrackingPoint> {
    let current_dir = std::env::current_dir().expect("Could not determine current directory");
    let folder_path = current_dir.join("Launch Data");
    
    // Check if folder exists
    if !folder_path.exists() {
        return vec![];
    }
    
    // Find  most recent CSV file
    let mut latest_file = None;
    let mut latest_time = 0u64;
    
    if let Ok(entries) = fs::read_dir(&folder_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "csv") {
                // Extract timestamp from filename (e.g., "data1767321506.csv")
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(time_str) = filename.strip_prefix("data").and_then(|s| s.strip_suffix(".csv")) {
                        if let Ok(time) = time_str.parse::<u64>() {
                            if time > latest_time {
                                latest_time = time;
                                latest_file = Some(path);
                            }
                        }
                    }
                }
            }
        }
    }
    
    let mut points = vec![];
    
    if let Some(file_path) = latest_file {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for line in content.lines().skip(1) { // Skip header
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 5 {
                    if let (Ok(lat), Ok(lon), Ok(alt), Ok(time)) = (
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                        parts[3].parse::<f64>(),
                        parts[4].parse::<u64>()
                    ) {
                        let track_type = parts[0].to_string();
                        points.push(TrackingPoint {
                            lat,
                            lon,
                            alt,
                            time,
                            track_type
                        });
                    }
                }
            }
        }
    }
    
    //Sort by time
    points.sort_by_key(|p| p.time);
    points
}

// Application run
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            utc, date, 
            set_irr_modem, get_irr_modem, 
            set_aprs_callsign, get_aprs_callsign, 
            set_aprs, set_iridium,
            update, 
            get_position, get_lat, get_long, get_alt,
            get_last_update, is_aprs_active, is_iridium_active,
            get_filtering_method, set_filtering_method,
            get_aprs_count, get_iridium_count, get_sondehub_count,
            get_aprs_validity, get_iridium_validity,
            get_tracking_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}