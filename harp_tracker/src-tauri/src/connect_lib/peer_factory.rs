// Equivalent of `new Peer(id)`
// The signaling half lives in signaling.rs.

use std::sync::Arc;
use webrtc::{
    api::{interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder},
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    peer_connection::{configuration::RTCConfiguration, RTCPeerConnection},
};

pub async fn create_peer() -> Arc<RTCPeerConnection> {
    // Factory object for creating peers
    let api = APIBuilder::new().build();

    // Google STUN only.
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    Arc::new(api.new_peer_connection(config).await.unwrap())
}