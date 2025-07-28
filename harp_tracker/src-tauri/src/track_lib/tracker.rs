use std::{fs::{self, File, OpenOptions}, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use io::Write;


use chrono::Utc;
// use serialport::{COMPort, SerialPort};

use crate::track_lib::{position_time::EstimationType, tracking_type::TrackingType};

use super::{aprs::APRS, iridium::Iridium, sondehub::SondeHub, position_time::PositionTime};




pub struct Tracker {
    active: bool,
    
    aprs: Option<APRS>,
    iridium: Option<Iridium>,
    sondehub: Option<SondeHub>,
    
    //Arduino and calculations modules have been commented out or removed to be worked on in the future
    // arduino: Option<Arduino>,
    // tracking: bool,

    position_time: PositionTime,
    csv_path: Option<PathBuf>
}

impl Tracker{
    
    
    // ------------------------Initializing Functions------------------------
    
    /// Create a new Tracker
    pub fn new() -> Self{
        Self { active:false, aprs: None,  iridium: None, sondehub:None, position_time: PositionTime {lat:0.0, lon:0.0, alt:0.0, last_update:0}, csv_path: None}
    }

    /// Create a new APRS Module
    pub fn new_aprs(&mut self, api_key: &str, call_sign: &str){
        self.aprs = Some(APRS::new(api_key, call_sign));
        if !self.active{self.csv_path = self.create_folder()}
        self.active = true;
    }

    /// Create a new Iridium Module
    pub fn new_iridium(&mut self, base_url: &str, modem: &str){
        self.iridium = Some(Iridium::new(base_url, modem));
        if !self.active{self.csv_path = self.create_folder()}
        self.active = true;
    }

    /// Create a new SondeHub Module
    pub fn new_sondehub(&mut self, call_sign: &str){
        self.sondehub = Some(SondeHub::new(call_sign));
        if !self.active{self.csv_path = self.create_folder()}
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


    pub fn return_aprs(&mut self) -> Option<APRS>{
        self.aprs.clone()
    }
    pub fn return_iridium(&mut self) -> Option<Iridium>{
        self.iridium.clone()
    }
    pub fn return_sondehub(&mut self) -> Option<SondeHub>{
        self.sondehub.clone()
    }
    // pub fn is_arduino_active(&self) -> bool {
    //     if self.aprs.is_some(){
    //         return self.arduino.as_ref().unwrap().active;
    //     }
    //     return false;
    // }

    // ------------------------Update Helper Functions------------------------

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

    // fn update_arduino(&mut self) -> Result<(), Box<(dyn std::error::Error + 'static)>>{
    //     if self.arduino.is_some(){
    //         return self.arduino.as_mut().unwrap().update();
    //     }
    //     // Ok(())
    //     return Err("Error updating Arduino".into());
    // }        

    /// Internal function to update all modules on the tracker and return the errors as a vector
    fn update_tracker(&mut self) -> Vec<Result<(), Box<(dyn std::error::Error + 'static)>>>{

        let mut v: Vec<Result<(), Box<(dyn std::error::Error + 'static)>>> = vec![];
        
        v.push(self.update_aprs());
        v.push(self.update_iridium());
        v.push(self.update_sondehub());
        // v.push(self.update_arduino());

        v

    }

    /// Creates a data storage folder, if not already existing
    fn create_folder(&self) -> Option<PathBuf> {
        let folder_name = format!("Launch Data");

        let current_dir = std::env::current_dir().expect("Could not determine current directory");
        let folder_path = current_dir.join(folder_name);

        fs::create_dir_all(&folder_path).expect("Unable to create csv data directory");
        
        let file_path: PathBuf = folder_path.join(format!("data{:?}.csv", Utc::now().timestamp()));

        let mut f = File::create(&file_path).expect("Unable to create CSV file");

        f.write("track_type,lat,lon,alt,time\n".as_bytes()).expect("Unable to write to CSV file");

        println!("Folder created at: {:?}", file_path);
        Some(file_path)
    }

    /// Function to write the data to csv
    fn write_to_csv(track_type:TrackingType,pos_time:PositionTime,csv_path:Option<PathBuf>) -> io::Result<TrackingType> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(csv_path.unwrap())?;
        writeln!(
            file,
            "{},{:.6},{:.6},{:.2},{}",
            track_type, pos_time.lat, pos_time.lon, pos_time.alt, pos_time.last_update
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

        let mut positions: Vec<PositionTime> = vec![];

        if let Some(aprs) = &self.aprs {
            let t = aprs.get_last_update();
            if t != 0 {
                let pt = aprs.get_pos_time();
                positions.push(pt.clone());
                eprintln!("{:?}", Self::write_to_csv(TrackingType::APRS, pt, self.csv_path.clone()));
            }
        }

        if let Some(iridium) = &self.iridium {
            let t = iridium.get_last_update();
            if t != 0 {
                let pt = iridium.get_pos_time();
                positions.push(pt.clone());
                eprintln!("{:?}", Self::write_to_csv(TrackingType::Iridium, pt, self.csv_path.clone()));
            }
        }

        if let Some(sondehub) = &self.sondehub {
            let t = sondehub.get_last_update();
            if t != 0 {
                let pt = sondehub.get_pos_time();
                positions.push(pt.clone());
                eprintln!("{:?}", Self::write_to_csv(TrackingType::SondeHub, pt, self.csv_path.clone()));

            }
        }

        let most_recent_position: Option<PositionTime> = PositionTime::return_valid_pos_time(positions, EstimationType::Median);

        //Update struct and log to CSV if we have a new update
        if let Some(new_pos) = most_recent_position.clone() {
            self.position_time = new_pos;
        }

        err
    }




    /// Function to print the data of the Tracker
    pub fn print(&self){
                // Print current position data
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let age_seconds = current_time.saturating_sub(self.position_time.last_update);

        println!("Latitude: {}, Longitude: {}, Altitude: {}, Last Update: {}s ago", self.position_time.lat, self.position_time.lon, self.position_time.alt, age_seconds);
    }

    // ------------------------Getter Functions------------------------

    pub fn get_position(&self)->(f64,f64,f64){return (self.position_time.lat,self.position_time.lon,self.position_time.alt);}
    pub fn get_last_update(&self)->u64{return self.position_time.last_update;}

}