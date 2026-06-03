// WebRTC peer factory — ICE/STUN/TURN from ice_config.

use std::sync::Arc;

use webrtc::{api::APIBuilder, peer_connection::RTCPeerConnection};

use crate::connect_lib::ice_config;

pub async fn create_peer() -> Arc<RTCPeerConnection> {
    let api = APIBuilder::new().build();
    let config = ice_config::build_rtc_configuration();
    Arc::new(api.new_peer_connection(config).await.unwrap())
}
