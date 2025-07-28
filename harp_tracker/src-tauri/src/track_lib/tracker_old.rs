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

        //Function to filter out any outliers in given gps data
        fn return_valid_pos_time(positions: Vec<(f64, f64, f64)>, times: Vec<u64>) -> Option<((f64, f64, f64), u64)> {
            if positions.is_empty() || times.is_empty() || positions.len() != times.len(){
                println!("Empty vectors or length mismatch");
                return None;
            }
            println!("Pos LEN {}", positions.len());
            println!("TIMES LEN {}", times.len());
            
            if times.len() == 1{
                println!("Single entry, returning it"); 
                return Some((positions.get(0).unwrap().to_owned(), times.get(0).unwrap().to_owned()));
            }

            if times.len() == 2{
                return Some((positions.get(1).unwrap().to_owned(), times.get(1).unwrap().to_owned()));
            }
            
            let n = positions.len();
            
            //Calc median position for each coordinate
            let mut x_coords: Vec<f64> = positions.iter().map(|p| p.0).collect();
            let mut y_coords: Vec<f64> = positions.iter().map(|p| p.1).collect();
            let mut z_coords: Vec<f64> = positions.iter().map(|p| p.2).collect();
            
            x_coords.sort_by(|a, b| a.partial_cmp(b).unwrap());
            y_coords.sort_by(|a, b| a.partial_cmp(b).unwrap());
            z_coords.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let median_x = if n % 2 == 0 {
                (x_coords[n/2 - 1] + x_coords[n/2]) / 2.0
            } else {
                x_coords[n/2]
            };
            
            let median_y = if n % 2 == 0 {
                (y_coords[n/2 - 1] + y_coords[n/2]) / 2.0
            } else {
                y_coords[n/2]
            };
            
            let median_z = if n % 2 == 0 {
                (z_coords[n/2 - 1] + z_coords[n/2]) / 2.0
            } else {
                z_coords[n/2]
            };
            
            println!("Medians - X: {}, Y: {}, Z: {}", median_x, median_y, median_z);
            
            //Calculates the acceptable margin
            let margin = 1.15; //I am currently using a 15% margin
            
            // Handle case where median might be zero or negative
            let x_range = if median_x.abs() < f64::EPSILON {
                // If median is essentially zero, use absolute margin
                (-margin, margin)
            } else {
                (median_x * (1.0 - margin), median_x * (1.0 + margin))
            };
            
            let y_range = if median_y.abs() < f64::EPSILON {
                (-margin, margin)
            } else {
                (median_y * (1.0 - margin), median_y * (1.0 + margin))
            };
            
            let z_range = if median_z.abs() < f64::EPSILON {
                (-margin, margin)
            } else {
                (median_z * (1.0 - margin), median_z * (1.0 + margin))
            };
            
            println!("Ranges - X: {:?}, Y: {:?}, Z: {:?}", x_range, y_range, z_range);
            
            // Find valid positions with their corresponding times, doesnt include outliers
            let mut valid_entries: Vec<((f64, f64, f64), u64)> = Vec::new();
            
            for i in 0..n {
                let pos = positions[i];
                let time = times[i];
                
                // Check if position is within acceptable range for all coordinates
                let x_valid = pos.0 >= x_range.0 && pos.0 <= x_range.1;
                let y_valid = pos.1 >= y_range.0 && pos.1 <= y_range.1;
                let z_valid = pos.2 >= z_range.0 && pos.2 <= z_range.1;
                
                println!("Position {}: {:?}, Time: {}", i, pos, time);
                println!("  X valid: {} ({} in range {:?})", x_valid, pos.0, x_range);
                println!("  Y valid: {} ({} in range {:?})", y_valid, pos.1, y_range);
                println!("  Z valid: {} ({} in range {:?})", z_valid, pos.2, z_range);
                
                if x_valid && y_valid && z_valid {
                    println!("  -> Adding to valid entries");
                    valid_entries.push((pos, time));
                } 
                else {
                    println!("  -> Rejected as outlier");
                }
            }
            
            println!("Found {} valid entries out of {}", valid_entries.len(), n);
            
            // Return the most recent valid entry
            let result = valid_entries.into_iter().max_by_key(|&(_, time)| time);
            if let Some(ref entry) = result {
                println!("Returning most recent valid entry: {:?}", entry);
            } else {
                println!("No valid entries found!");
            }
            result
        }

        // ------------------------Public Functions------------------------
        
        pub fn update(&mut self) -> Vec<Box<(dyn std::error::Error + 'static)>>{
            
            //Error collection to display to users (Only soft errors, not program breaking)
            let mut err: Vec<Box<(dyn std::error::Error + 'static)>> = vec![];

            for opt in self.update_tracker(){
                if opt.is_err(){err.push(opt.err().unwrap());}
            }

            let mut most_recent_time = None;
            let mut most_recent_position = None;
            
            let mut positions: Vec<(f64,f64,f64)> = vec![];
            let mut times: Vec<u64> = vec![];

            if let Some(aprs) = &self.aprs {
                let aprs = aprs.clone();

                //push updates to a vector

                let t = aprs.get_last_update();
                if t != 0{
                    times.push(t);
                    positions.push(aprs.get_position());
                }

            }
            
            if let Some(iridium) = &self.iridium {
                let iridium = iridium.clone();
                
                //push updates to a vector

                let t = iridium.get_last_update();
                if t != 0{
                    times.push(t);
                    positions.push(iridium.get_position());
                }
            }
            
            if let Some(sondehub) = &self.sondehub {
                let sondehub = sondehub.clone();
                
                //push updates to a vector

                let t = sondehub.get_last_update();
                if t != 0{
                    times.push(t);
                    positions.push(sondehub.get_position());
                }
            }   
            let res = Tracker::return_valid_pos_time(positions,times);


            if res.is_some(){
                most_recent_position = Some(res.unwrap().0);
                most_recent_time = Some(res.unwrap().1);
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