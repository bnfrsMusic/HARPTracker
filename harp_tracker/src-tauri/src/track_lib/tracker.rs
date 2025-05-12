use std::time::{SystemTime, UNIX_EPOCH};



use super::{aprs::APRS, iridium::{Iridium}, sondehub::SondeHub};

pub struct Tracker {
    aprs: Option<APRS>,
    iridium: Option<Iridium>,
    sondehub: Option<SondeHub>,
    lat: f64,
    long: f64,
    alt: f64,
    vertical_velocity: f64,
    ground_speed: f64,
    last_update: u64

}

impl Tracker{
    
    
    // ------------------------Initializing Functions------------------------
    
    // Create a new Tracker
    pub fn new() -> Self{
        Self { aprs: None,  iridium: None, sondehub:None, lat: 0.0, long: 0.0, alt: 0.0, vertical_velocity: 0.0, ground_speed: 0.0,  last_update: 0}
    }

    pub fn new_aprs(&mut self, api_key: &str, call_sign: &str){
        self.aprs = Some(APRS::new(api_key, call_sign));
    }


    pub fn new_iridium(&mut self, base_url: &str, modem: &str){
        self.iridium = Some(Iridium::new(base_url, modem))
    }

    pub fn new_sondehub(&mut self, call_sign: &str){
        self.sondehub = Some(SondeHub::new(call_sign));
    }


    pub fn return_aprs(&mut self) -> Option<APRS>{
        self.aprs.clone()
    }
    pub fn return_iridium(&mut self) -> Option<Iridium>{
        self.iridium.clone()
    }
    pub fn return_sondehub(&mut self) -> Option<SondeHub>{
        self.sondehub.clone()
    }
    // ------------------------Helper Functions------------------------

    fn update_aprs(&mut self) -> Result<(), Box<(dyn std::error::Error + 'static)>>{
        if self.aprs.is_some(){
            return self.aprs.as_mut().unwrap().update_position();
        }
        // Ok(())
        return Err("Error updating APRS".into());
    }


    fn update_iridium(&mut self) -> Result<(), Box<(dyn std::error::Error + 'static)>>{
        if self.iridium.is_some(){
            return self.iridium.as_mut().unwrap().update_position();
        }
        // Ok(())
        return Err("Error updating Iridium".into());
    }
    fn update_sondehub(&mut self) -> Result<(), Box<(dyn std::error::Error + 'static)>>{
        if self.sondehub.is_some(){
            return self.sondehub.as_mut().unwrap().update_position();
        }
        // Ok(())
        return Err("Error updating Sondehub".into());
    }

    fn update_tracker(&mut self) -> Vec<Result<(), Box<(dyn std::error::Error + 'static)>>>{

        let mut v: Vec<Result<(), Box<(dyn std::error::Error + 'static)>>> = vec![];
        
        v.push(self.update_aprs());
        v.push(self.update_iridium());
        v.push(self.update_sondehub());
        
        v

    }



    // ------------------------Public Functions------------------------
    
    pub fn update(&mut self) -> Vec<Box<(dyn std::error::Error + 'static)>>{
        
        let mut err: Vec<Box<(dyn std::error::Error + 'static)>> = vec![];

        for opt in self.update_tracker(){
            if opt.is_err(){err.push(opt.err().unwrap());}
        }

        let mut most_recent_time = None;
        let mut most_recent_position = None;
        
        if let Some(aprs) = &self.aprs {
            let aprs = aprs.clone();
            let update_time = aprs.get_last_update();
            most_recent_time = Some(update_time);
            most_recent_position = Some(aprs.get_position());
        }
        
        if let Some(iridium) = &self.iridium {
            let iridium = iridium.clone();
            let update_time = iridium.get_last_update();
            if most_recent_time.is_none() || update_time > most_recent_time.unwrap() {
                most_recent_time = Some(update_time);
                most_recent_position = Some(iridium.get_position());
            }
        }
        
        if let Some(sondehub) = &self.sondehub {
            let sondehub = sondehub.clone();
            let update_time = sondehub.get_last_update();
            if most_recent_time.is_none() || update_time >= most_recent_time.unwrap() {
                most_recent_time = Some(update_time);
                most_recent_position = Some(sondehub.get_position());
            }
        }
        
        // Finally, assign if we found any data
        if let (Some(pos), Some(update)) = (most_recent_position, most_recent_time) {
            (self.lat, self.long, self.alt) = pos;
            self.last_update = update;
        }

        err
        
    }

    pub fn print(&self){
                // Print current position data
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let age_seconds = current_time.saturating_sub(self.last_update);

        println!("Latitude: {}, Longitude: {}, Altitude: {}, Last Update: {}s ago", self.lat, self.long, self.alt, age_seconds);
    }

    // ------------------------Getter Functions------------------------

    pub fn get_position(&self)->(f64,f64,f64){return (self.lat,self.long,self.alt);}
    pub fn get_last_update(&self)->u64{return self.last_update;}

}