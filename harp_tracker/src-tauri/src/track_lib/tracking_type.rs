
use std::fmt::Display;

/// Helper function for the Tracker Module
#[derive(Debug, Clone, Copy)]
pub enum TrackingType{
    APRS,
    Iridium,
    SondeHub,
}

impl Display for TrackingType{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            TrackingType::APRS => "APRS",
            TrackingType::Iridium => "Iridium",
            TrackingType::SondeHub => "SondeHub",
        };
        write!(f, "{}", name)
    }
}
