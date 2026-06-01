// json message types sent over webrtc data channels
use serde::{Deserialize, Serialize};

use crate::{PredictionData, TrackingPoint};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PositionPayload {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub horiz_vel: f64,
    pub vert_vel: f64,
    pub last_update: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SyncMsg {
    RoleAnnouncement {
        role: String,
        id: String,
        name: String,
    },
    Assignment {
        name: String,
        gs_id: String,
    },
    SyncFull {
        history: Vec<TrackingPoint>,
        position: Option<PositionPayload>,
        prediction: Option<PredictionData>,
    },
    SyncPosition {
        position: PositionPayload,
    },
    SyncPrediction {
        prediction: PredictionData,
    },
}

impl SyncMsg {
    pub fn to_bytes(&self) -> Result<bytes::Bytes, String> {
        serde_json::to_string(self)
            .map(|s| bytes::Bytes::from(s))
            .map_err(|e| e.to_string())
    }

    pub fn from_text(text: &str) -> Option<SyncMsg> {
        serde_json::from_str(text).ok()
    }
}
