// Clients are first connected and then make WebRTC offers for Ground Station to answer
// Signaling passes the offers and their given answers via a server
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use webrtc::{
    data_channel::RTCDataChannel,
    ice_transport::ice_candidate::RTCIceCandidateInit,
    peer_connection::{
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
};

use crate::connect_lib::{
    gen::generate_human_id,
    ice_config,
    peer_factory::create_peer,
    signaling::{connect_signaling, wait_for_peer_online, SignalMsg, SignalingConnection},
};

use tauri::{Emitter, State};

pub struct ClientState {
    pub signaling_cancel: CancellationToken,
    pub peer: Option<Arc<RTCPeerConnection>>,
    pub channel: Option<Arc<RTCDataChannel>>,
}

#[tauri::command]
pub async fn client_run(
    app: tauri::AppHandle,
    gs_id: String,
    state: State<'_, Arc<Mutex<Option<ClientState>>>>,
) -> Result<String, String> {
    let gs_id = gs_id.trim().to_string();
    if gs_id.is_empty() {
        return Err("Ground Station ID is required".into());
    }

    {
        let guard = state.lock().unwrap();
        if guard.is_some() {
            return Err("Client is already running. Disconnect before connecting again.".into());
        }
    }

    let my_id = generate_human_id();
    wait_for_peer_online(&gs_id, Duration::from_secs(45)).await?;

    let signaling = connect_signaling(&my_id).await?;
    let SignalingConnection {
        tx: signal_tx,
        rx: signal_rx,
        cancel: signaling_cancel,
    } = signaling;

    let id_for_task = my_id.clone();
    let the_state = state.inner().clone();

    {
        let mut s = the_state.lock().unwrap();
        *s = Some(ClientState {
            signaling_cancel: signaling_cancel.clone(),
            peer: None,
            channel: None,
        });
    }

    let mut signal_rx = signal_rx;
    tauri::async_runtime::spawn(async move {
        let peer = create_peer().await;
        ice_config::attach_ice_state_handler(&peer, app.clone(), id_for_task.clone(), "client");

        let channel = peer
            .create_data_channel("main", None)
            .await
            .unwrap();

        let ch_open = channel.clone();
        channel.on_open(Box::new(move || {
            let ch = ch_open.clone();
            Box::pin(async move {
                let announcement = serde_json::json!({
                    "type": "role-announcement",
                    "role": "client",
                });
                let _ = ch.send(&Bytes::from(announcement.to_string())).await;
            })
        }));

        let the_id = gs_id.clone();
        let app_for_msg = app.clone();

        channel.on_message(Box::new(move |msg| {
            let id_inner = the_id.clone();
            let app_inner = app_for_msg.clone();

            Box::pin(async move {
                let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    if data["type"] == "assignment" {
                        if let Some(name) = data["name"].as_str() {
                            println!("  Status  : Linked to GS as {}", name);
                            let _ = app_inner.emit(
                                "new-gs",
                                serde_json::json!({
                                    "id": id_inner,
                                    "role": "Ground Station",
                                }),
                            );
                        }
                    }
                }
            })
        }));

        channel.on_close(Box::new(move || {
            Box::pin(async move {
                println!("  Status  : Disconnected from GS");
            })
        }));

        let signal_tx_ice = signal_tx.clone();
        let node_id_ice = id_for_task.clone();
        let gs_id_ice = gs_id.clone();

        peer.on_ice_candidate(Box::new(move |candidate| {
            let tx = signal_tx_ice.clone();
            let from = node_id_ice.clone();
            let to = gs_id_ice.clone();

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

        let offer = peer.create_offer(None).await.unwrap();
        peer.set_local_description(offer.clone()).await.unwrap();

        if signal_tx
            .send(SignalMsg::Offer {
                from: id_for_task.clone(),
                to: gs_id.clone(),
                sdp: offer.sdp,
            })
            .is_err()
        {
            let _ = app.emit(
                "client-error",
                serde_json::json!({ "message": "Signaling channel closed before offer was sent" }),
            );
            return;
        }

        println!(
            "  Client '{}' sent offer to Ground Station '{}'",
            id_for_task, gs_id
        );

        while let Some(msg) = signal_rx.recv().await {
            match msg {
                SignalMsg::Answer { sdp, .. } => {
                    let answer = RTCSessionDescription::answer(sdp).unwrap();
                    peer.set_remote_description(answer).await.unwrap();
                }
                SignalMsg::Ice {
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                    ..
                } => {
                    let _ = peer
                        .add_ice_candidate(RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index,
                            ..Default::default()
                        })
                        .await;
                }
                SignalMsg::Error { message } => {
                    eprintln!("  Signaling error: {}", message);
                    if !message.contains("not found") {
                        let _ = app.emit("client-error", serde_json::json!({ "message": message }));
                    }
                }
                _ => {}
            }

            let mut s = the_state.lock().unwrap();
            if let Some(client) = s.as_mut() {
                client.peer = Some(peer.clone());
                client.channel = Some(channel.clone());
            }
        }
    });

    Ok(my_id)
}

#[tauri::command]
pub async fn client_disconnect(
    state: State<'_, Arc<Mutex<Option<ClientState>>>>,
) -> Result<(), String> {
    let c_opt = {
        let mut s = state.lock().unwrap();
        s.take()
    };

    if let Some(client) = c_opt {
        client.signaling_cancel.cancel();
        if let Some(ch) = client.channel {
            let _ = ch.close().await;
        }
        if let Some(peer) = client.peer {
            let _ = peer.close().await;
        }
    }

    Ok(())
}
