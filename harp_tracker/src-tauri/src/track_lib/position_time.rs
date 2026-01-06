


/** Struct to handle position and time data.

lat -> Latitude 

lon -> Longitude

alt -> Altitude in meters

last_update -> Unix timestamp of the last update
*/
#[derive(Debug, Clone)]
pub struct PositionTime{
    
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub last_update: u64
    
}



#[derive(Debug, Clone, Copy)]
pub enum EstimationType {
    Average,
    Median,
    Recent

    // Future additions:
    // Kalman,
    // LastNMean,
}



impl PositionTime {


    //------------------------Initializing Functions------------------------
    pub fn new() -> Self {
        Self {lat:0.0, lon:0.0, alt:0.0, last_update:0}
    }

    pub fn new_with_value(lat:f64,lon:f64,alt:f64,last_update:u64) -> Self {
        Self {lat, lon, alt, last_update}
    }
    
    pub fn update(&mut self, lat:f64, lon:f64, alt: f64, last_update:u64){
        self.lat = lat;
        self.lon = lon;
        self.alt = alt;
        self.last_update = last_update;
    }



    //------------------------Functions------------------------

    ///Function to filter out any outliers in given gps data based on method specified by user
    pub fn return_valid_pos_time(data: Vec<PositionTime>, method: EstimationType) -> Option<PositionTime> {
        if data.is_empty() {
            return None;
        }

        match method {
            EstimationType::Average => Some(PositionTime::average(data)),
            EstimationType::Median => Some(PositionTime::median(data)),
            EstimationType::Recent => Some(PositionTime::recent(data)),
        }
    }


    //------------------------Estimation Functions------------------------
 
    pub fn average(pos_time: Vec<PositionTime>) -> PositionTime{
        let mut avg_pos_time = PositionTime::new();
        for i in pos_time.iter(){
            avg_pos_time.update(avg_pos_time.lat + i.lat,
                                avg_pos_time.lon + i.lon, 
                                avg_pos_time.alt + i.alt, 
                                avg_pos_time.last_update + i.last_update);
        }
        avg_pos_time.update(avg_pos_time.lat/(pos_time.len() as f64) ,
            avg_pos_time.lon/(pos_time.len() as f64),
            avg_pos_time.alt/(pos_time.len() as f64),
            avg_pos_time.last_update/(pos_time.len() as u64));
        avg_pos_time
    }

    pub fn median(pos_time: Vec<PositionTime>) -> PositionTime{
        
        //Create the sorted array so that we can work with it
        let mut sorted= pos_time.clone();
        
        //Sort array if needed
        if Self::check_sort(&sorted){
            Self::quick_sort(&mut sorted);
        }

        let len = sorted.len();

        match len {
            0 => panic!("Cannot compute median of empty list."),
            1 => sorted[0].clone(),
            2 => Self::average(vec![sorted[0].clone(), sorted[1].clone()]),
            _ => {
                if len % 2 == 0 {
                    let mid1 = sorted[len / 2 - 1].clone();
                    let mid2 = sorted[len / 2].clone();
                    Self::average(vec![mid1, mid2])
                } else {
                    sorted[len / 2].clone()
                }
            }
        }
    }

    pub fn recent(pos_time: Vec<PositionTime>) -> PositionTime{
        if pos_time.is_empty() {
            panic!("Cannot get most recent PositionTime");
        }

        pos_time
            .into_iter()
            .max_by_key(|pt| pt.last_update)
            .expect("PositionTime Recent Estimation max_by_key failed")
    }

    //------------------------Sort Functions------------------------

    pub fn quick_sort(pos_time: &mut [PositionTime]) {
        let len = pos_time.len();
        if len <= 1 {
            return;
        }

        let pivot_index = len / 2;
        let pivot_value = pos_time[pivot_index].last_update;

        //move pivot to the end
        pos_time.swap(pivot_index, len - 1);

        let mut i = 0;
        for j in 0..len - 1 {
            if pos_time[j].last_update <= pivot_value {
                pos_time.swap(i, j);
                i += 1;
            }
        }

        //move pivot to final place
        pos_time.swap(i, len - 1);

        let (left, right) = pos_time.split_at_mut(i);
        Self::quick_sort(left);
        Self::quick_sort(&mut right[1..]); //Skip pivot
    }

    fn check_sort(pos_time: &Vec<PositionTime>) -> bool{
        // Returns true if array needs sorting (is NOT sorted)
        // Returns false if array is already sorted
        if pos_time.len() <= 1 {
            return false; // Single or empty array is already sorted
        }
        
        for i in 0..pos_time.len() - 1 {
            if pos_time[i].last_update > pos_time[i + 1].last_update {
                return true; // Found an inversion, array needs sorting
            }
        }
        return false; // Array is already sorted
    }

}