// ground station: auto-accepts client offers, manages data channels, syncs flight data

use std::sync::{Arc, Mutex};

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
    protocol::SyncMsg,
    signaling::{connect_signaling, SignalMsg, SignalingConnection},
    sync,
};
use tokio_util::sync::CancellationToken;

use tauri::{AppHandle, Emitter, State};

pub struct GsState {
    pub gs_id: String,
    pub signal_tx: tokio::sync::mpsc::UnboundedSender<SignalMsg>,
    pub signaling_cancel: CancellationToken,
    pub peer_map: Arc<DashMap<String, Arc<RTCPeerConnection>>>,
    pub node_channels: Arc<DashMap<String, Arc<RTCDataChannel>>>,
    pub client_names: Arc<DashMap<String, String>>,
    pub pending_ice: Arc<DashMap<String, Vec<RTCIceCandidateInit>>>,
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
    let client_names: Arc<DashMap<String, String>> = Arc::new(DashMap::new());
    let pending_ice: Arc<DashMap<String, Vec<RTCIceCandidateInit>>> = Arc::new(DashMap::new());

    {
        let mut s = the_state.lock().unwrap();
        *s = Some(GsState {
            gs_id: id_for_task.clone(),
            signal_tx: signal_tx.clone(),
            signaling_cancel: signaling_cancel.clone(),
            peer_map: peer_map.clone(),
            node_channels: node_channels.clone(),
            client_names: client_names.clone(),
            pending_ice: pending_ice.clone(),
        });
    }

    let app_loop = app.clone();
    let mut signal_rx = signal_rx;
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = signal_rx.recv().await {
            match msg {
                SignalMsg::Offer { from: node_id, sdp, .. } => {
                    println!("  auto-accepting offer from '{}'", node_id);
                    let _ = app_loop.emit(
                        "pending-client",
                        serde_json::json!({ "id": node_id, "auto": true }),
                    );
                    accept_offer_internal(
                        app_loop.clone(),
                        the_state.clone(),
                        node_id,
                        sdp,
                    )
                    .await;
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
                    } else {
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

    println!("  Ground Station registered as '{}'", my_id);
    Ok(my_id)
}

async fn accept_offer_internal(
    app: AppHandle,
    state: Arc<Mutex<Option<GsState>>>,
    node_id: String,
    offer_sdp: String,
) {
    let gs = {
        let guard = state.lock().unwrap();
        let Some(gs) = guard.as_ref() else {
            return;
        };
        (
            gs.signal_tx.clone(),
            gs.gs_id.clone(),
            gs.peer_map.clone(),
            gs.node_channels.clone(),
            gs.client_names.clone(),
            gs.pending_ice.clone(),
        )
    };

    let (signal_tx, gs_id, peer_map, node_channels, client_names, pending_ice) = gs;

    let buffered_ice = pending_ice
        .remove(&node_id)
        .map(|(_, v)| v)
        .unwrap_or_default();

    let peer = create_peer().await;
    ice_config::attach_ice_state_handler(&peer, app.clone(), node_id.clone(), "ground_station");
    peer_map.insert(node_id.clone(), peer.clone());

    tauri::async_runtime::spawn(handle_incoming_connection(
        peer,
        offer_sdp,
        buffered_ice,
        node_id,
        gs_id,
        signal_tx,
        node_channels,
        client_names,
        app,
    ));
}

#[tauri::command]
pub async fn gs_accept_offer(
    node_id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    // kept for compatibility; offers are auto-accepted now
    let _ = (node_id, app, state);
    Ok(())
}

#[tauri::command]
pub fn gs_list_pending_offers(
    _state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Vec<String> {
    vec![]
}

#[tauri::command]
pub async fn gs_reject_offer(
    node_id: String,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let peer = {
        let guard = state.lock().unwrap();
        if let Some(gs) = guard.as_ref() {
            gs.pending_ice.remove(&node_id);
            gs.peer_map.remove(&node_id).map(|(_, p)| p)
        } else {
            None
        }
    };
    if let Some(peer) = peer {
        let _ = peer.close().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn gs_remove_client(
    node_id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {
    let (peer_map, node_channels, client_names) = {
        let guard = state.lock().unwrap();
        let Some(gs) = guard.as_ref() else {
            return Err("Ground Station is not running".into());
        };
        (
            gs.peer_map.clone(),
            gs.node_channels.clone(),
            gs.client_names.clone(),
        )
    };

    sync::unregister_channel(&node_id);
    client_names.remove(&node_id);
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
    client_names: Arc<DashMap<String, String>>,
    app: AppHandle,
) {
    let offer = RTCSessionDescription::offer(offer_sdp).unwrap();
    peer.set_remote_description(offer).await.unwrap();

    for ice in buffered_ice {
        let _ = peer.add_ice_candidate(ice).await;
    }

    let dc_node_id = node_id.clone();
    let dc_app = app.clone();
    let dc_gs_id = gs_id.clone();
    let dc_names = client_names.clone();

    peer.on_data_channel(Box::new(move |channel| {
        let node_channels = node_channels.clone();
        let ch_for_msg = channel.clone();
        let async_app = dc_app.clone();
        let async_node_id = dc_node_id.clone();
        let async_gs_id = dc_gs_id.clone();
        let async_names = dc_names.clone();

        Box::pin(async move {
            channel.on_open(Box::new(|| Box::pin(async {})));

            let msg_node_channels = node_channels.clone();
            let msg_ch = ch_for_msg.clone();
            let app_for_msg = async_app.clone();
            let id_for_msg = async_node_id.clone();
            let gs_id_for_msg = async_gs_id.clone();
            let names_for_msg = async_names.clone();

            channel.on_message(Box::new(move |msg| {
                let node_channels = msg_node_channels.clone();
                let ch = msg_ch.clone();
                let app_inner = app_for_msg.clone();
                let id_inner = id_for_msg.clone();
                let gs_id_inner = gs_id_for_msg.clone();
                let names_inner = names_for_msg.clone();

                Box::pin(async move {
                    let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
                    let Some(data) = SyncMsg::from_text(&text) else {
                        return;
                    };

                    match data {
                        SyncMsg::RoleAnnouncement { role, id, name } if role == "client" => {
                            let client_id = {
                                let announced = id.trim().to_string();
                                if announced.is_empty() {
                                    id_inner.clone()
                                } else {
                                    announced
                                }
                            };
                            let display_name = if name.trim().is_empty() {
                                client_id.clone()
                            } else {
                                name.trim().to_string()
                            };
                            names_inner.insert(client_id.clone(), display_name.clone());

                            let assignment = SyncMsg::Assignment {
                                name: display_name.clone(),
                                gs_id: gs_id_inner.clone(),
                            };
                            if let Ok(bytes) = assignment.to_bytes() {
                                let _ = ch.send(&bytes).await;
                            }

                            node_channels.insert(client_id.clone(), ch.clone());
                            sync::register_channel(&client_id, ch.clone());

                            let n = node_channels.len();
                            println!("  connected — {} client(s)", n);

                            let _ = app_inner.emit(
                                "new-client",
                                serde_json::json!({
                                    "id": client_id,
                                    "role": "Client",
                                    "name": display_name,
                                }),
                            );

                            sync::send_full_sync(&client_id).await;
                        }
                        _ => {}
                    }
                })
            }));

            let node_channels_cl = node_channels.clone();
            let app_close = async_app.clone();
            let id_close = async_node_id.clone();
            let names_close = async_names.clone();

            channel.on_close(Box::new(move || {
                let node_channels = node_channels_cl.clone();
                let app = app_close.clone();
                let id = id_close.clone();
                let names = names_close.clone();
                Box::pin(async move {
                    sync::unregister_channel(&id);
                    node_channels.remove(&id);
                    names.remove(&id);
                    let n = node_channels.len();
                    println!("  clients connected: {}", n);
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
            sync::unregister_channel(entry.key());
            let _ = entry.value().close().await;
        }
        gs.node_channels.clear();
        gs.client_names.clear();

        for entry in gs.peer_map.iter() {
            let _ = entry.value().close().await;
        }
        gs.peer_map.clear();
        gs.pending_ice.clear();
    }

    Ok(())
}
