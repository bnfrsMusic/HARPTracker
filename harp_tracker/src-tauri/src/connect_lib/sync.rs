// broadcast flight data from ground station to connected clients
use std::sync::Mutex;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use webrtc::data_channel::RTCDataChannel;

use crate::{
    connect_lib::protocol::{PositionPayload, SyncMsg},
    read_tracking_history, PredictionData, TrackingPoint, LOCATION, TRACKER,
};

use std::sync::Arc;

static CLIENT_CHANNELS: Lazy<Arc<DashMap<String, Arc<RTCDataChannel>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

static LAST_PREDICTION: Lazy<Mutex<Option<PredictionData>>> =
    Lazy::new(|| Mutex::new(None));

pub fn register_channel(client_id: &str, channel: Arc<RTCDataChannel>) {
    CLIENT_CHANNELS.insert(client_id.to_string(), channel);
}

pub fn unregister_channel(client_id: &str) {
    CLIENT_CHANNELS.remove(client_id);
}

pub fn set_last_prediction(prediction: PredictionData) {
    *LAST_PREDICTION.lock().unwrap() = Some(prediction);
}

pub fn connected_client_count() -> usize {
    CLIENT_CHANNELS.len()
}

fn current_position() -> Option<PositionPayload> {
    let tracker = TRACKER.lock().ok()?;
    let last_update = tracker.get_last_update();
    if last_update == 0 {
        return None;
    }
    let (horiz_vel, vert_vel) = tracker.get_velocities();
    drop(tracker);

    let loc = LOCATION.lock().ok()?;
    Some(PositionPayload {
        lat: loc.lat,
        lon: loc.long,
        alt: loc.alt,
        horiz_vel,
        vert_vel,
        last_update,
    })
}

pub fn build_full_sync() -> SyncMsg {
    let history: Vec<TrackingPoint> = read_tracking_history();
    let position = current_position();
    let prediction = LAST_PREDICTION.lock().unwrap().clone();
    SyncMsg::SyncFull {
        history,
        position,
        prediction,
    }
}

async fn send_to_channel(channel: &RTCDataChannel, msg: &SyncMsg) {
    if let Ok(bytes) = msg.to_bytes() {
        let _ = channel.send(&bytes).await;
    }
}

pub async fn send_full_sync(client_id: &str) {
    let Some(channel) = CLIENT_CHANNELS.get(client_id).map(|e| e.value().clone()) else {
        return;
    };
    let msg = build_full_sync();
    send_to_channel(&channel, &msg).await;
}

pub async fn broadcast(msg: SyncMsg) {
    for entry in CLIENT_CHANNELS.iter() {
        send_to_channel(entry.value(), &msg).await;
    }
}

pub async fn broadcast_position() {
    if CLIENT_CHANNELS.is_empty() {
        return;
    }
    if let Some(position) = current_position() {
        broadcast(SyncMsg::SyncPosition { position }).await;
    }
}

pub async fn broadcast_prediction(prediction: PredictionData) {
    set_last_prediction(prediction.clone());
    if CLIENT_CHANNELS.is_empty() {
        return;
    }
    broadcast(SyncMsg::SyncPrediction { prediction }).await;
}
