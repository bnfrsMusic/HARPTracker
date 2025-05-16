    use std::{fs::{self, File, OpenOptions}, io, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
    use io::Write;


    use chrono::Utc;

    use super::{aprs::APRS, iridium::{Iridium}, sondehub::SondeHub};

    pub struct Tracker {
        active: bool,
        aprs: Option<APRS>,
        iridium: Option<Iridium>,
        sondehub: Option<SondeHub>,
        lat: f64,
        long: f64,
        alt: f64,
        last_update: u64,
        csv_path: Option<PathBuf>
    }

    impl Tracker{
        
        
        // ------------------------Initializing Functions------------------------
        
        // Create a new Tracker
        pub fn new() -> Self{
            Self { active:false, aprs: None,  iridium: None, sondehub:None, lat: 0.0, long: 0.0, alt: 0.0,  last_update: 0, csv_path: None}
        }

        // The following is the initializers for the supported protocols. 
        // It also creates the data folder and assigns it to csv_path so the data can be stored there
        pub fn new_aprs(&mut self, api_key: &str, call_sign: &str){
            self.aprs = Some(APRS::new(api_key, call_sign));
            if !self.active{self.csv_path = self.create_folder()}
            self.active = true;
        }


        pub fn new_iridium(&mut self, base_url: &str, modem: &str){
            self.iridium = Some(Iridium::new(base_url, modem));
            if !self.active{self.csv_path = self.create_folder()}
            self.active = true;
        }

        pub fn new_sondehub(&mut self, call_sign: &str){
            self.sondehub = Some(SondeHub::new(call_sign));
            if !self.active{self.csv_path = self.create_folder()}
            self.active = true;
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

        fn update_tracker(&mut self) -> Vec<Result<(), Box<(dyn std::error::Error + 'static)>>>{

            let mut v: Vec<Result<(), Box<(dyn std::error::Error + 'static)>>> = vec![];
            
            v.push(self.update_aprs());
            v.push(self.update_iridium());
            v.push(self.update_sondehub());
            
            v

        }

        // Creates a data storage folder, if not already existing. Then creates and returns the csv file
        fn create_folder(&self) -> Option<PathBuf> {
            let folder_name = format!("Launch Data");

            let current_dir = std::env::current_dir().expect("Could not determine current directory");
            let folder_path = current_dir.join(folder_name);

            fs::create_dir_all(&folder_path).expect("Unable to create csv data directory");
            
            let file_path: PathBuf = folder_path.join(format!("data{:?}.csv", Utc::now().timestamp()));

            let mut f = File::create(&file_path).expect("Unable to create CSV file");

            f.write("lat,lon,alt,time\n".as_bytes()).expect("Unable to write to CSV file");

            println!("Folder created at: {:?}", file_path);
            Some(file_path)
}

        // Function to write the data to csv
        fn write_to_csv(&self, csv_path:PathBuf) -> io::Result<()> {
            let mut file = OpenOptions::new()
                .append(true)
                .open(csv_path)?;
            writeln!(
                file,
                "{:.6},{:.6},{:.2},{}",
                self.lat, self.long, self.alt, self.last_update
            )?;
            Ok(())
        }


        // ------------------------Public Functions------------------------
        
        pub fn update(&mut self) -> Vec<Box<(dyn std::error::Error + 'static)>>{


            let mut err: Vec<Box<(dyn std::error::Error + 'static)>> = vec![];

            for opt in self.update_tracker(){
                if opt.is_err(){err.push(opt.err().unwrap());}
            }

            //Collect latest time and pos

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
            
            //Update struct and log to CSV
            if self.csv_path.is_some() && most_recent_time.is_some() && most_recent_time.unwrap() != self.last_update{
                if let (Some((lat, lon, alt)), Some(update_time)) = (most_recent_position, most_recent_time) {
                    self.lat = lat;
                    self.long = lon;
                    self.alt = alt;
                    self.last_update = update_time;
                    if let Err(e) = self.write_to_csv(self.csv_path.clone().unwrap()) {
                        panic!("Failed to write to CSV: {}", e);
                    }
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