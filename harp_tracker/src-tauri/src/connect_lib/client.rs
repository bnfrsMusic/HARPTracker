// client: connects to one ground station, receives synced flight data, auto-reconnects

use std::sync::{Arc, Mutex};
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
    protocol::SyncMsg,
    signaling::{connect_signaling, wait_for_peer_online, SignalMsg, SignalingConnection},
};

use tauri::{Emitter, State};

pub struct ClientState {
    pub my_id: String,
    pub gs_id: String,
    pub client_name: String,
    pub session_cancel: CancellationToken,
    pub reconnect_cancel: CancellationToken,
    pub peer: Option<Arc<RTCPeerConnection>>,
    pub channel: Option<Arc<RTCDataChannel>>,
}

fn emit_sync_event(app: &tauri::AppHandle, event: &str, payload: serde_json::Value) {
    let _ = app.emit(event, payload);
}

fn handle_sync_message(app: &tauri::AppHandle, msg: SyncMsg) {
    match msg {
        SyncMsg::Assignment { name, gs_id } => {
            emit_sync_event(
                app,
                "new-gs",
                serde_json::json!({ "id": gs_id, "role": "Ground Station", "name": name }),
            );
            emit_sync_event(
                app,
                "client-mode",
                serde_json::json!({ "connected": true }),
            );
        }
        SyncMsg::SyncFull {
            history,
            position,
            prediction,
        } => {
            emit_sync_event(
                app,
                "gs-sync-full",
                serde_json::json!({
                    "history": history,
                    "position": position,
                    "prediction": prediction,
                }),
            );
        }
        SyncMsg::SyncPosition { position } => {
            emit_sync_event(app, "gs-sync-position", serde_json::json!({ "position": position }));
        }
        SyncMsg::SyncPrediction { prediction } => {
            emit_sync_event(
                app,
                "gs-sync-prediction",
                serde_json::json!({ "prediction": prediction }),
            );
        }
        _ => {}
    }
}

async fn run_client_session(
    app: tauri::AppHandle,
    gs_id: String,
    my_id: String,
    client_name: String,
    state: Arc<Mutex<Option<ClientState>>>,
    cancel: CancellationToken,
) -> Result<(), String> {
    wait_for_peer_online(&gs_id, Duration::from_secs(45)).await?;

    let signaling = connect_signaling(&my_id).await?;
    let SignalingConnection {
        tx: signal_tx,
        rx: mut signal_rx,
        cancel: _signaling_cancel,
    } = signaling;

    let peer = create_peer().await;
    ice_config::attach_ice_state_handler(&peer, app.clone(), my_id.clone(), "client");

    let channel = peer.create_data_channel("main", None).await.unwrap();

    let ch_open = channel.clone();
    let announce_id = my_id.clone();
    let announce_name = client_name.clone();
    channel.on_open(Box::new(move || {
        let ch = ch_open.clone();
        let id = announce_id.clone();
        let name = announce_name.clone();
        Box::pin(async move {
            let announcement = SyncMsg::RoleAnnouncement {
                role: "client".to_string(),
                id,
                name,
            };
            if let Ok(bytes) = announcement.to_bytes() {
                let _ = ch.send(&bytes).await;
            }
        })
    }));

    let app_for_msg = app.clone();
    channel.on_message(Box::new(move |msg| {
        let app_inner = app_for_msg.clone();
        Box::pin(async move {
            let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
            if let Some(data) = SyncMsg::from_text(&text) {
                handle_sync_message(&app_inner, data);
            }
        })
    }));

    let app_for_close = app.clone();
    let state_for_close = state.clone();
    let cancel_for_close = cancel.clone();
    channel.on_close(Box::new(move || {
        let app = app_for_close.clone();
        let state = state_for_close.clone();
        let cancel = cancel_for_close.clone();
        Box::pin(async move {
            println!("  disconnected from ground station");
            cancel.cancel();
            emit_sync_event(&app, "gs-disconnected", serde_json::json!({}));
            spawn_reconnect(app, state);
        })
    }));

    let signal_tx_ice = signal_tx.clone();
    let node_id_ice = my_id.clone();
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

    signal_tx
        .send(SignalMsg::Offer {
            from: my_id.clone(),
            to: gs_id.clone(),
            sdp: offer.sdp,
        })
        .map_err(|_| "Signaling channel closed before offer was sent".to_string())?;

    println!("  client '{}' connecting to gs '{}'", my_id, gs_id);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = channel.close().await;
                let _ = peer.close().await;
                return Ok(());
            }
            msg = signal_rx.recv() => {
                match msg {
                    Some(SignalMsg::Answer { sdp, .. }) => {
                        let answer = RTCSessionDescription::answer(sdp).unwrap();
                        peer.set_remote_description(answer).await.unwrap();
                    }
                    Some(SignalMsg::Ice { candidate, sdp_mid, sdp_mline_index, .. }) => {
                        let _ = peer.add_ice_candidate(RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index,
                            ..Default::default()
                        }).await;
                    }
                    Some(SignalMsg::Error { message }) => {
                        eprintln!("  Signaling error: {}", message);
                        if !message.contains("not found") {
                            emit_sync_event(&app, "client-error", serde_json::json!({ "message": message }));
                        }
                    }
                    None => break,
                    _ => {}
                }

                let mut s = state.lock().unwrap();
                if let Some(client) = s.as_mut() {
                    client.peer = Some(peer.clone());
                    client.channel = Some(channel.clone());
                }
            }
        }
    }

    Ok(())
}

fn spawn_reconnect(app: tauri::AppHandle, state: Arc<Mutex<Option<ClientState>>>) {
    let params = {
        let guard = state.lock().unwrap();
        guard.as_ref().map(|c| (c.gs_id.clone(), c.my_id.clone(), c.client_name.clone()))
    };
    let Some((gs_id, my_id, client_name)) = params else {
        return;
    };

    let reconnect_cancel = CancellationToken::new();
    {
        let mut guard = state.lock().unwrap();
        if let Some(client) = guard.as_mut() {
            client.reconnect_cancel = reconnect_cancel.clone();
            client.peer = None;
            client.channel = None;
        }
    }

    tauri::async_runtime::spawn(async move {
        let mut delay = Duration::from_secs(2);
        let max_delay = Duration::from_secs(30);

        loop {
            if reconnect_cancel.is_cancelled() {
                return;
            }
            tokio::time::sleep(delay).await;
            if reconnect_cancel.is_cancelled() {
                return;
            }

            emit_sync_event(
                &app,
                "client-error",
                serde_json::json!({ "message": "Reconnecting to Ground Station…" }),
            );

            let session_cancel = CancellationToken::new();
            {
                let mut guard = state.lock().unwrap();
                if let Some(client) = guard.as_mut() {
                    client.session_cancel = session_cancel.clone();
                }
            }

            match run_client_session(
                app.clone(),
                gs_id.clone(),
                my_id.clone(),
                client_name.clone(),
                state.clone(),
                session_cancel,
            )
            .await
            {
                Ok(()) => {
                    // session ended; reconnect loop continues unless cancelled
                }
                Err(e) => {
                    eprintln!("  reconnect failed: {}", e);
                    delay = (delay * 2).min(max_delay);
                }
            }
        }
    });
}

#[tauri::command]
pub async fn client_run(
    app: tauri::AppHandle,
    gs_id: String,
    client_name: Option<String>,
    state: State<'_, Arc<Mutex<Option<ClientState>>>>,
) -> Result<String, String> {
    let gs_id = gs_id.trim().to_string();
    if gs_id.is_empty() {
        return Err("Ground Station ID is required".into());
    }

    let client_name = client_name
        .unwrap_or_default()
        .trim()
        .to_string();

    {
        let guard = state.lock().unwrap();
        if guard.is_some() {
            return Err("Client is already running. Disconnect before connecting again.".into());
        }
    }

    let my_id = generate_human_id();
    let session_cancel = CancellationToken::new();
    let reconnect_cancel = CancellationToken::new();
    let the_state = state.inner().clone();

    {
        let mut s = the_state.lock().unwrap();
        *s = Some(ClientState {
            my_id: my_id.clone(),
            gs_id: gs_id.clone(),
            client_name: client_name.clone(),
            session_cancel: session_cancel.clone(),
            reconnect_cancel: reconnect_cancel.clone(),
            peer: None,
            channel: None,
        });
    }

    let app_clone = app.clone();
    let gs_clone = gs_id.clone();
    let id_clone = my_id.clone();
    let name_clone = client_name.clone();
    let state_clone = the_state.clone();
    let cancel_clone = session_cancel.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_client_session(
            app_clone.clone(),
            gs_clone,
            id_clone,
            name_clone,
            state_clone,
            cancel_clone,
        )
        .await
        {
            emit_sync_event(
                &app_clone,
                "client-error",
                serde_json::json!({ "message": e }),
            );
        }
    });

    Ok(my_id)
}

#[tauri::command]
pub async fn client_disconnect(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Option<ClientState>>>>,
) -> Result<(), String> {
    let c_opt = {
        let mut s = state.lock().unwrap();
        s.take()
    };

    if let Some(client) = c_opt {
        client.reconnect_cancel.cancel();
        client.session_cancel.cancel();
        if let Some(ch) = client.channel {
            let _ = ch.close().await;
        }
        if let Some(peer) = client.peer {
            let _ = peer.close().await;
        }
    }

    let _ = app.emit("client-mode", serde_json::json!({ "connected": false }));

    Ok(())
}

#[tauri::command]
pub fn is_client_connected(state: State<'_, Arc<Mutex<Option<ClientState>>>>) -> bool {
    let guard = state.lock().unwrap();
    guard
        .as_ref()
        .map(|c| c.channel.is_some())
        .unwrap_or(false)
}
