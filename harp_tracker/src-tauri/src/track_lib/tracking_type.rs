use std::fmt::Display;

/// Helper function for the Tracker Module
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingType {
    APRS,
    Iridium,
    SondeHub,
    WSPR,
}

impl Display for TrackingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            TrackingType::APRS => "APRS",
            TrackingType::Iridium => "Iridium",
            TrackingType::SondeHub => "SondeHub",
            TrackingType::WSPR => "WSPR",
        };
        write!(f, "{}", name)
    }
}
