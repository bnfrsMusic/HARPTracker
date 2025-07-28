use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::SystemTime,
};
use serialport::{SerialPort, SerialPortInfo, SerialPortType, COMPort};





//-------const values-------
//Known Arduino USB vendor/product IDs
const ARDUINO_VIDS: [u16; 2] = [0x2341, 0x2A03];
const ARDUINO_PIDS: [u16; 2] = [0x0043, 0x0001];

//Default baud rate
const DEFAULT_BAUD: u32 = 9600;

//Default read timeout (ms)
const DEFAULT_TIMEOUT_MS: u64 = 100;

/// Arduino Module:
pub struct Arduino {
    pub active: bool,
    pub tracking: bool,
    pub serial_port: Option<Arc<Mutex<Box<dyn SerialPort + Send>>>>,
    pub com_port: Option<COMPort>,
    pub time1: SystemTime,
    pub time2: SystemTime,

}

impl Arduino {

    //-----------------------Initialaztion Functions-----------------------

    pub fn new(
        serial: Option<Arc<Mutex<Box<dyn SerialPort + Send>>>>,
        com: Option<COMPort>,
    ) -> Self {
        Self {
            active: serial.is_some(),
            tracking: false,
            serial_port: serial,
            com_port: com,
            time1: SystemTime::now(),
            time2: SystemTime::now(),

        }
    }

    /// Discover & open the first detected Arduino port (blocking). Sets `active=true` on success.
    pub fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let arduino_ports = Self::detect_arduino_ports();
        println!("Arduino Ports: {:?}", arduino_ports);

        if let Some(port_name) = arduino_ports.get(0) {
            match serialport::new(port_name, DEFAULT_BAUD)
                .timeout(std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS))
                .open()
            {
                Ok(port) => {
                    self.serial_port = Some(Arc::new(Mutex::new(port)));
                    self.active = true;
                    println!("Arduino Connected at port {port_name:?}");
                    self.send_command("0,0").unwrap_or(eprintln!("Error sending command to Arduino"));
                    Ok(())
                }
                Err(e) => {
                    Err(format!("Arduino: Failed to open {port_name}: {e}").into())
                }
            }
        } else {

            Err("No Arduino Detected".into())

        }
    }


    //-----------------------Main Functions-----------------------


    /// Send command to the Arduino.
    pub fn send_command(&mut self, cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.active {
            return Err("No Arduino detected; cannot send command.".into());
        }
        if let Some(mut guard) = self.lock_port() {

            println!("Sending {cmd} to serial port.");
            let cmd = cmd.to_owned() + "\n";

            guard.write_all(cmd.as_bytes())?;
            guard.flush().unwrap_or_else(|e| {
                eprintln!("Error flushing data: {e}");
            });
        } else {
            return Err("Serial port not set.".into());
        }
        Ok(())
    }


    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>>  {
        //Return Error if not active
        if !self.active {
            return Err("No Arduino detected; cannot run update function.".into());
        }

        self.send_command("0,0").unwrap_or(eprintln!("Error sending command to Arduino"));

        Ok(())
    }


    pub fn set_tracking(&mut self, val: bool){
        if !(self.active){eprintln!("Can't set Tracking due to no active Arduino")}
        self.tracking = val;
    }

    //-----------------------Helper Functions-----------------------

    //Scan connected serial ports for arduino
    fn detect_arduino_ports() -> Vec<String> {
        match serialport::available_ports() {
            Ok(ports) => ports
                .into_iter()
                .filter_map(|p: SerialPortInfo| match &p.port_type {
                    SerialPortType::UsbPort(usb_info) => {
                        let is_vid = ARDUINO_VIDS.contains(&usb_info.vid);
                        let is_pid = ARDUINO_PIDS.contains(&usb_info.pid);
                        if is_vid && is_pid {
                            Some(p.port_name)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .collect(),
            Err(e) => {
                eprintln!("Port enumeration failed: {e}");
                vec![]
            }
        }
    }


    /// Internal helper: lock the serial mutex and return a guard.
    fn lock_port(&self) -> Option<std::sync::MutexGuard<'_, Box<dyn SerialPort + Send>>> {
        let serial_arc = self.serial_port.as_ref()?;
        // If the mutex is poisoned, recover the inner value.
        match serial_arc.lock() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                eprintln!("Serial port mutex poisoned; recovering.");
                Some(poisoned.into_inner())
            }
        }
    }


    /// Private debugging function to stream serial data from the arduino
    fn stream_data(&mut self) {
        if !self.active {
            println!("No Arduino active; cannot stream data.");
            return;
        }
        println!("Starting to stream data from Arduino...");
        let mut buffer = [0u8; 1024];

        loop {
            // Scope the lock each loop to avoid holding it across print I/O.
            let n_res = {
                if let Some(mut guard) = self.lock_port() {
                    guard.read(&mut buffer)
                } else {
                    eprintln!("Serial port not set.");
                    break;
                }
            };

            match n_res {
                Ok(n) if n > 0 => {
                    let data = String::from_utf8_lossy(&buffer[..n]);
                    print!("{}", data);
                }
                Ok(_) => { /* n == 0: ignore */ }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Continue polling.
                    continue;
                }
                Err(e) => {
                    eprintln!("Error reading from serial port: {e:?}");
                    break;
                }
            }
        }
    }

}