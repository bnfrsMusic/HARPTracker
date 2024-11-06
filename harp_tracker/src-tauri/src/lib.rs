//imports
use chrono::Utc;

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
    println!("AAAAAAAAAAAAAAAAAAAH");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![settings, utc, date])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
