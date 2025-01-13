//imports
use chrono::Utc;
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub static IMEI: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
pub static APRS_CALLSIGN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

//Returns the current UTC time
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

#[tauri::command]
fn date() -> String {
    format!("Date: {}", Utc::now().date_naive().to_string())
}

#[tauri::command]
fn settings() {
    println!("Open Settings");
}

//--------------------------------Setters and Getters--------------------------------

//IRIDIUM_MODEM
#[tauri::command]
fn set_imei(id: String) {
    panic!("AAAAAAAAAAAAH");
    //*IMEI.lock().unwrap() = id;
}
#[tauri::command]
fn get_imei() {
    println!("IMEI: {:?}", IMEI.lock().unwrap().to_string())
}

//--------------------------------Run--------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            settings, utc, date, set_imei, get_imei,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

struct class_iridium {
    active: bool,
    lat: f64,
    long: f64,
    alt: f64,
    ascent_rate: f64,
}

impl class_iridium {
    pub fn setup(&mut self) {
        self.active = true;
    }
}
