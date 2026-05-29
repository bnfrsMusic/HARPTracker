//Non-WebRTC related functions
use rand::Rng;

//Id generation
const ADJECTIVES: [&str; 10] = [
    "Swift",
    "Agile",
    "Nimble",
    "Brilliant",
    "Clever",
    "Resourceful",
    "Inventive",
    "Ingenious",
    "Astute",
    "Shrewd",
];

const ANIMALS: [&str; 10] = [
    "Falcon",
    "Panther",
    "Eagle",
    "Wolf",
    "Tiger",
    "Lion",
    "Hawk",
    "Cheetah",
    "Leopard",
    "Jaguar",
];

const COLORS: [&str; 10] = [
    "Crimson",
    "Azure",
    "Emerald",
    "Sapphire",
    "Amber",
    "Violet",
    "Indigo",
    "Scarlet",
    "Turquoise",
    "Magenta",
];

//*Pulled into JavaScript
#[tauri::command]
pub fn generate_human_id() -> String {
    let mut rng = rand::thread_rng();
    let adjective = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let animal = ANIMALS[rng.gen_range(0..ANIMALS.len())];
    let color = COLORS[rng.gen_range(0..COLORS.len())];

    return format!("{} {} {}", adjective, color, animal);
}

#[tauri::command]
pub fn return_id(id: &str) -> String {
    return id.to_string();
}

#[tauri::command]
pub fn set_status(msg: &str) -> String {
    return format!("[STATUS] {msg}");
}
 
