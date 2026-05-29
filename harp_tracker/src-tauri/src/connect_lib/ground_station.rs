// The Ground Station waits for Clients to send WebRTC offers via
// The signaling server, answers them, then manages the resulting data channels.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use dashmap::DashMap;
use webrtc::{
    data_channel::RTCDataChannel,
    ice_transport::ice_candidate::RTCIceCandidateInit,
    peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection},
};

use crate::connect_lib::{
    gen::{generate_human_id, return_id, set_status},
    peer_factory::create_peer,
    signaling::{connect_signaling, SignalMsg},
};

use tauri::{
    Emitter,
    Manager,
};

// Main logic
#[tauri::command]
pub async fn gs_run(app: tauri::AppHandle) -> Result<String, String> {
    let my_id = generate_human_id();
    
    // Clone the ID so we can pass it into the background thread
    let id_for_task = my_id.clone();

    // Spawn the long-running task into the background
    tauri::async_runtime::spawn(async move {
        let (signal_tx, mut signal_rx) = match connect_signaling(&id_for_task).await {
            Ok(channels) => channels,
            Err(e) => {
                let _ = app.emit("client-error", serde_json::json!({ "message": e }));
                return; // Stop the background task
            }
        };

        // Move your maps and counters inside the spawned task
        let peer_map: Arc<DashMap<String, Arc<RTCPeerConnection>>> = Arc::new(DashMap::new());
        let connections: Arc<Mutex<Vec<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(Vec::new()));
        let node_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        // Main signaling loop runs safely in the background
        while let Some(msg) = signal_rx.recv().await {
            match msg {
                SignalMsg::Offer { from: node_id, sdp, .. } => {
                    let peer = create_peer().await;
                    peer_map.insert(node_id.clone(), peer.clone());

                    let signal_tx   = signal_tx.clone();
                    let gs_id       = id_for_task.clone();
                    let node_id_cl  = node_id.clone();
                    let connections = connections.clone();
                    let node_count  = node_count.clone();
                    let app_for_handler = app.clone();

                    tauri::async_runtime::spawn(async move {
                        handle_incoming_connection(
                            peer, sdp, node_id_cl, gs_id,
                            signal_tx, connections, node_count,
                            app_for_handler,
                        )
                        .await;
                    });
                }
                SignalMsg::Ice { from: node_id, candidate, sdp_mid, sdp_mline_index, .. } => {
                    if let Some(peer) = peer_map.get(&node_id) {
                        let _ = peer.add_ice_candidate(RTCIceCandidateInit {
                            candidate, sdp_mid, sdp_mline_index, ..Default::default()
                        }).await;
                    }
                }
                SignalMsg::Error { message } => {
                    eprintln!("  Signaling error: {}", message);
                }
                _ => {}
            }
        }
    });

    // Immediately return the ID to JavaScript
    Ok(my_id)
}

async fn handle_incoming_connection(
    peer:        Arc<RTCPeerConnection>,
    offer_sdp:   String,
    node_id:     String,
    gs_id:       String,
    signal_tx:   tokio::sync::mpsc::UnboundedSender<SignalMsg>,
    connections: Arc<Mutex<Vec<Arc<RTCDataChannel>>>>,
    node_count:  Arc<Mutex<usize>>,
    app:         tauri::AppHandle,
) {
    // Apply the Client's offer
    let offer = RTCSessionDescription::offer(offer_sdp).unwrap();
    peer.set_remote_description(offer).await.unwrap();

    peer.on_data_channel(Box::new(move |channel| {
        let connections = connections.clone();
        let node_count  = node_count.clone();
        let ch_for_msg  = channel.clone();
        let app_for_ch  = app.clone();

        Box::pin(async move {
            // JS: conn.on("open") — GS just waits; nothing to send first
            channel.on_open(Box::new(|| Box::pin(async {})));
            let msg_connections = connections.clone();
            let msg_node_count  = node_count.clone();
            let msg_ch          = ch_for_msg.clone();
            let app_for_msg     = app_for_ch.clone();

            // JS: conn.on("data", data => { if data.type === "role-announcement" … })
            channel.on_message(Box::new(move |msg| {
                let connections = msg_connections.clone();
                let node_count  = msg_node_count.clone();
                let ch          = msg_ch.clone();
                let app         = app_for_msg.clone();

                Box::pin(async move {
                    let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
                    let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) else {
                        return;
                    };

                    if data["type"] == "role-announcement" && data["role"] == "node" {
                        // Assign "n1", "n2", … — mirrors JS nodeIndex / nodeName
                        let node_name = {
                            let mut count = node_count.lock().unwrap();
                            *count += 1;
                            format!("n{}", count)
                        };

                        // JS: conn.send({ type: "assignment", name: nodeName })
                        let assignment = serde_json::json!({
                            "type": "assignment",
                            "name": node_name,
                        });
                        let _ = ch.send(&Bytes::from(assignment.to_string())).await;

                        // JS: connections.push(conn)
                        connections.lock().unwrap().push(ch.clone());

                        let n = connections.lock().unwrap().len();
                        println!("  Status : Connected — {} Node(s)", n);

                        // Notify the frontend so it can render the new peer entry
                        let _ = app.emit("new-node", serde_json::json!({
                            "id":   node_name,
                            "role": "node",
                        }));
                    } else if data.get("type").is_none() {
                        // JS: conn.close() for unrecognised sources
                        let _ = ch.close().await;
                    }
                })
            }));

            // JS: conn.on("close", () => { connections.filter(…) … })
            let connections_cl = connections.clone();

            channel.on_close(Box::new(move || {
                let connections = connections_cl.clone();
                Box::pin(async move {
                    // Remove closed channel from the vec
                    // (identity comparison via Arc pointer)
                    // JS: connections = connections.filter(c => c !== conn)
                    // Rust: retain all channels that are still open
                    connections
                        .lock()
                        .unwrap()
                        .retain(|c| !matches!(c.ready_state(),
                            webrtc::data_channel::data_channel_state::RTCDataChannelState::Closed
                        ));
                    let n = connections.lock().unwrap().len();
                    if n > 0 {
                        println!("  Status : Connected — {} Node(s)", n);
                    } else {
                        println!("  Status : GS Online — Waiting for Nodes");
                    }
                })
            }));
        })
    }));

    // ── Trickle-ICE: forward our candidates to the Node via signaling ─────────
    // JS: peer.on("icecandidate", …) — hidden inside PeerJS
    let signal_tx_ice = signal_tx.clone();
    let gs_id_ice     = gs_id.clone();
    let node_id_ice   = node_id.clone();
    peer.on_ice_candidate(Box::new(move |candidate| {
        let tx      = signal_tx_ice.clone();
        let from    = gs_id_ice.clone();
        let to      = node_id_ice.clone();
        Box::pin(async move {
            if let Some(c) = candidate {
                if let Ok(c_json) = c.to_json() {
                    let _ = tx.send(SignalMsg::Ice {
                        from,
                        to,
                        candidate:       c_json.candidate,
                        sdp_mid:         c_json.sdp_mid,
                        sdp_mline_index: c_json.sdp_mline_index,
                    });
                }
            }
        })
    }));

    // Create and send the SDP answer back to the Node
    let answer = peer.create_answer(None).await.unwrap();
    peer.set_local_description(answer.clone()).await.unwrap();

    let _ = signal_tx.send(SignalMsg::Answer {
        from: gs_id,
        to:   node_id,
        sdp:  answer.sdp,
    });
}