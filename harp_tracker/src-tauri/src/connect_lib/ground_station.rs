// The Ground Station waits for Clients to send WebRTC offers via
// the signaling server, accepts them on demand, then manages the resulting data channels.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use dashmap::DashMap;
use webrtc::{
    data_channel::RTCDataChannel,
    ice_transport::ice_candidate::RTCIceCandidateInit,
    peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection},
};

use crate::connect_lib::{
    gen::generate_human_id,
    ice_config,
    peer_factory::create_peer,
    signaling::{connect_signaling, SignalMsg, SignalingConnection},
};
use tokio_util::sync::CancellationToken;

use tauri::{AppHandle, Emitter, State};

pub struct GsState {
    pub gs_id: String,
    pub signal_tx: tokio::sync::mpsc::UnboundedSender<SignalMsg>,
    pub signaling_cancel: CancellationToken,
    pub peer_map: Arc<DashMap<String, Arc<RTCPeerConnection>>>,
    pub node_channels: Arc<DashMap<String, Arc<RTCDataChannel>>>,
    pub pending_offers: Arc<DashMap<String, String>>,
    pub pending_ice: Arc<DashMap<String, Vec<RTCIceCandidateInit>>>,
    pub node_count: Arc<Mutex<usize>>,
}

#[tauri::command]
pub async fn gs_run(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<String, String> {
    {
        let guard = state.lock().unwrap();
        if guard.is_some() {
            return Err(
                "Ground Station is already running. Disconnect before starting again.".into(),
            );
        }
    }

    let my_id = generate_human_id();
    let signaling = connect_signaling(&my_id).await?;
    let SignalingConnection {
        tx: signal_tx,
        rx: signal_rx,
        cancel: signaling_cancel,
    } = signaling;

    let id_for_task = my_id.clone();
    let the_state = state.inner().clone();

    let peer_map: Arc<DashMap<String, Arc<RTCPeerConnection>>> = Arc::new(DashMap::new());
    let node_channels: Arc<DashMap<String, Arc<RTCDataChannel>>> = Arc::new(DashMap::new());
    let pending_offers: Arc<DashMap<String, String>> = Arc::new(DashMap::new());
    let pending_ice: Arc<DashMap<String, Vec<RTCIceCandidateInit>>> = Arc::new(DashMap::new());
    let node_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    {
        let mut s = the_state.lock().unwrap();
        *s = Some(GsState {
            gs_id: id_for_task.clone(),
            signal_tx: signal_tx.clone(),
            signaling_cancel: signaling_cancel.clone(),
            peer_map: peer_map.clone(),
            node_channels: node_channels.clone(),
            pending_offers: pending_offers.clone(),
            pending_ice: pending_ice.clone(),
            node_count: node_count.clone(),
        });
    }

    let app_loop = app.clone();
    let mut signal_rx = signal_rx;
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = signal_rx.recv().await {
            match msg {
                SignalMsg::Offer { from: node_id, sdp, .. } => {
                    println!("  Received WebRTC offer from client '{}'", node_id);
                    pending_offers.insert(node_id.clone(), sdp);
                    let _ = app_loop.emit(
                        "pending-client",
                        serde_json::json!({ "id": node_id }),
                    );
                }
                SignalMsg::Ice {
                    from: node_id,
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                    ..
                } => {
                    let ice = RTCIceCandidateInit {
                        candidate,
                        sdp_mid,
                        sdp_mline_index,
                        ..Default::default()
                    };
                    if let Some(peer) = peer_map.get(&node_id) {
                        let _ = peer.add_ice_candidate(ice).await;
                    } else if pending_offers.contains_key(&node_id) {
                        pending_ice
                            .entry(node_id)
                            .or_insert_with(Vec::new)
                            .push(ice);
                    }
                }
                SignalMsg::Error { message } => {
                    eprintln!("  Signaling error: {}", message);
                }
                _ => {}
            }
        }
    });

    let _ = app.emit(
        "gs-online",
        serde_json::json!({ "id": my_id }),
    );

    println!("  Ground Station registered on signaling server as '{}'", my_id);
    Ok(my_id)
}

#[tauri::command]
pub fn gs_list_pending_offers(
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Vec<String> {
    let guard = state.lock().unwrap();
    let Some(gs) = guard.as_ref() else {
        return vec![];
    };
    gs.pending_offers
        .iter()
        .map(|entry| entry.key().clone())
        .collect()
}

#[tauri::command]
pub async fn gs_accept_offer(
    node_id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let gs = {
        let guard = state.lock().unwrap();
        let Some(gs) = guard.as_ref() else {
            return Err("Ground Station is not running".into());
        };
        (
            gs.signal_tx.clone(),
            gs.gs_id.clone(),
            gs.peer_map.clone(),
            gs.node_channels.clone(),
            gs.pending_offers.clone(),
            gs.pending_ice.clone(),
            gs.node_count.clone(),
        )
    };

    let (
        signal_tx,
        gs_id,
        peer_map,
        node_channels,
        pending_offers,
        pending_ice,
        node_count,
    ) = gs;

    let offer_sdp = pending_offers
        .remove(&node_id)
        .map(|(_, sdp)| sdp)
        .ok_or_else(|| format!("No pending offer from '{}'", node_id))?;

    let buffered_ice = pending_ice
        .remove(&node_id)
        .map(|(_, v)| v)
        .unwrap_or_default();

    let peer = create_peer().await;
    ice_config::attach_ice_state_handler(&peer, app.clone(), node_id.clone(), "ground_station");
    peer_map.insert(node_id.clone(), peer.clone());

    let signal_tx_cl = signal_tx.clone();
    let gs_id_cl = gs_id.clone();
    let node_id_cl = node_id.clone();
    let node_channels_cl = node_channels.clone();
    let node_count_cl = node_count.clone();
    let app_cl = app.clone();

    tauri::async_runtime::spawn(async move {
        handle_incoming_connection(
            peer,
            offer_sdp,
            buffered_ice,
            node_id_cl,
            gs_id_cl,
            signal_tx_cl,
            node_channels_cl,
            node_count_cl,
            app_cl,
        )
        .await;
    });

    Ok(())
}

#[tauri::command]
pub async fn gs_reject_offer(
    node_id: String,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let guard = state.lock().unwrap();
    let Some(gs) = guard.as_ref() else {
        return Err("Ground Station is not running".into());
    };
    gs.pending_offers.remove(&node_id);
    gs.pending_ice.remove(&node_id);
    Ok(())
}

#[tauri::command]
pub async fn gs_remove_client(
    node_id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let (peer_map, node_channels) = {
        let guard = state.lock().unwrap();
        let Some(gs) = guard.as_ref() else {
            return Err("Ground Station is not running".into());
        };
        (gs.peer_map.clone(), gs.node_channels.clone())
    };

    if let Some((_, ch)) = node_channels.remove(&node_id) {
        let _ = ch.close().await;
    }
    if let Some((_, peer)) = peer_map.remove(&node_id) {
        let _ = peer.close().await;
    }

    let _ = app.emit(
        "client-removed",
        serde_json::json!({ "id": node_id }),
    );

    Ok(())
}

async fn handle_incoming_connection(
    peer: Arc<RTCPeerConnection>,
    offer_sdp: String,
    buffered_ice: Vec<RTCIceCandidateInit>,
    node_id: String,
    gs_id: String,
    signal_tx: tokio::sync::mpsc::UnboundedSender<SignalMsg>,
    node_channels: Arc<DashMap<String, Arc<RTCDataChannel>>>,
    node_count: Arc<Mutex<usize>>,
    app: tauri::AppHandle,
) {
    let offer = RTCSessionDescription::offer(offer_sdp).unwrap();
    peer.set_remote_description(offer).await.unwrap();

    for ice in buffered_ice {
        let _ = peer.add_ice_candidate(ice).await;
    }

    let dc_node_id = node_id.clone();
    let dc_app = app.clone();

    peer.on_data_channel(Box::new(move |channel| {
        let node_channels = node_channels.clone();
        let node_count = node_count.clone();
        let ch_for_msg = channel.clone();
        let async_app = dc_app.clone();
        let async_node_id = dc_node_id.clone();

        Box::pin(async move {
            channel.on_open(Box::new(|| Box::pin(async {})));

            let msg_node_channels = node_channels.clone();
            let msg_node_count = node_count.clone();
            let msg_ch = ch_for_msg.clone();
            let app_for_msg = async_app.clone();
            let id_for_msg = async_node_id.clone();

            channel.on_message(Box::new(move |msg| {
                let node_channels = msg_node_channels.clone();
                let node_count = msg_node_count.clone();
                let ch = msg_ch.clone();
                let app_inner = app_for_msg.clone();
                let id_inner = id_for_msg.clone();

                Box::pin(async move {
                    let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
                    let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
                        return;
                    };

                    if data["type"] == "role-announcement" && data["role"] == "client" {
                        let node_name = {
                            let mut count = node_count.lock().unwrap();
                            *count += 1;
                            format!("client{}", count)
                        };

                        let assignment = serde_json::json!({
                            "type": "assignment",
                            "name": node_name,
                        });
                        let _ = ch.send(&Bytes::from(assignment.to_string())).await;

                        node_channels.insert(id_inner.clone(), ch.clone());

                        let n = node_channels.len();
                        println!("  Status : Connected — {} Node(s)", n);

                        let _ = app_inner.emit(
                            "new-client",
                            serde_json::json!({
                                "id": id_inner,
                                "role": "Client",
                                "name": node_name,
                            }),
                        );
                    } else if data.get("type").is_none() {
                        let _ = ch.close().await;
                    }
                })
            }));

            let node_channels_cl = node_channels.clone();
            let app_close = async_app.clone();
            let id_close = async_node_id.clone();

            channel.on_close(Box::new(move || {
                let node_channels = node_channels_cl.clone();
                let app = app_close.clone();
                let id = id_close.clone();
                Box::pin(async move {
                    node_channels.remove(&id);
                    let n = node_channels.len();
                    if n > 0 {
                        println!("  Status : Connected — {} Node(s)", n);
                    } else {
                        println!("  Status : GS Online — Waiting for Nodes");
                    }
                    let _ = app.emit(
                        "client-removed",
                        serde_json::json!({ "id": id }),
                    );
                })
            }));
        })
    }));

    let signal_tx_ice = signal_tx.clone();
    let gs_id_ice = gs_id.clone();
    let node_id_ice = node_id.clone();

    peer.on_ice_candidate(Box::new(move |candidate| {
        let tx = signal_tx_ice.clone();
        let from = gs_id_ice.clone();
        let to = node_id_ice.clone();
        Box::pin(async move {
            if let Some(c) = candidate {
                if let Ok(c_json) = c.to_json() {
                    let _ = tx.send(SignalMsg::Ice {
                        from,
                        to,
                        candidate: c_json.candidate,
                        sdp_mid: c_json.sdp_mid,
                        sdp_mline_index: c_json.sdp_mline_index,
                    });
                }
            }
        })
    }));

    let answer = peer.create_answer(None).await.unwrap();
    peer.set_local_description(answer.clone()).await.unwrap();

    let _ = signal_tx.send(SignalMsg::Answer {
        from: gs_id,
        to: node_id,
        sdp: answer.sdp,
    });
}

#[tauri::command]
pub async fn gs_disconnect(
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let gs_opt = {
        let mut s = state.lock().unwrap();
        s.take()
    };

    if let Some(gs) = gs_opt {
        gs.signaling_cancel.cancel();
        for entry in gs.node_channels.iter() {
            let _ = entry.value().close().await;
        }
        gs.node_channels.clear();

        for entry in gs.peer_map.iter() {
            let _ = entry.value().close().await;
        }
        gs.peer_map.clear();
        gs.pending_offers.clear();
        gs.pending_ice.clear();
    }

    Ok(())
}
