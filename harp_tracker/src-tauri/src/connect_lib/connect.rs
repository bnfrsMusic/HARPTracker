//human-readable IDs

const adjectives: [&str; 15] = 
    ["Swift", "Silent", "Brave", "Golden", "Azure", 
    "Crimson", "Quiet", "Bold", "Solar", "Lunar"
    "Hidden", "Apex", "Vibrant", "Steady", "Ancient"];

const nouns: [&str; 15] = 
    ["Falcon", "Eagle", "Wolf", "Tiger", "Nova", 
    "Pulsar", "Ghost", "Cipher", "Sentry", "Vortex",
    "Zenith", "Titan", "Kodiak", "Lynx", "Oryx"];

const colors : [&str; 15] = 
    ["Red", "Blue", "Green", "Yellow", "Purple", 
    "White", "Black", "Orange", "Cyan", "Magenta", 
    "Indigo", "Silver", "Gold", "Olive", "Teal"];

pub fn generate_human_id() -> String {
    let adj = adjectives[rand::random::<usize>() % adjectives.len()];
    let noun = nouns[rand::random::<usize>() % nouns.len()];
    let color = colors[rand::random::<usize>() % colors.len()];

    format!("{} {} {}", color, adj, noun)
}

//states per each client
let peer = null;
let role = null;