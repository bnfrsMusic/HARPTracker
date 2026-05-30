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
    gen::{generate_human_id},
    peer_factory::create_peer,
    signaling::{connect_signaling, SignalMsg}
};

use tauri::{
    Emitter,
    Manager,
    State
};

pub struct GsState {
    pub peer_map:    Arc<DashMap<String, Arc<RTCPeerConnection>>>,
    pub connections: Arc<Mutex<Vec<Arc<RTCDataChannel>>>>,
}

// Main logic for registering GS peer
#[tauri::command]
pub async fn gs_run(
    app: tauri::AppHandle, 
    state: State<'_, Arc<Mutex<Option<GsState>>>>
) -> Result<String, String> {
    let my_id = generate_human_id();
    
    //Cloned variables used for long-running tasks
    let id_for_task = my_id.clone();
    let the_state = state.inner().clone();

    // Spawned task for peer creation and connection handling in the background
    tauri::async_runtime::spawn(async move {
        // Connects to server
        let (signal_tx, mut signal_rx) = match connect_signaling(&id_for_task).await {
            Ok(channels) => channels,
            Err(e) => {
                let _ = app.emit("client-error", serde_json::json!({ "message": e }));
                return;
            }
        };
        // Lists the number of online peers
        let peer_map: Arc<DashMap<String, Arc<RTCPeerConnection>>> = Arc::new(DashMap::new());
        // Lists the number of peers that are connected
        let connections: Arc<Mutex<Vec<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(Vec::new()));
        let node_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        // Main signaling loop
        // The msg hold the offer information
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
                    // Add the client peer as a potential ICE candidate if found in the peer map
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

            // Store state to be reached in connections.js
            let mut s = the_state.lock().unwrap();
            *s = Some(GsState {
                peer_map: peer_map.clone(),
                connections: connections.clone(),
            });
        }
    });

    // Return the ID to JavaScript
    Ok(my_id)
}

// For handling incoming offers from Client peers
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
            // Ground Station will wait without anything to send on open
            channel.on_open(Box::new(|| Box::pin(async {})));
            let msg_connections = connections.clone();
            let msg_node_count  = node_count.clone();
            let msg_ch          = ch_for_msg.clone();
            let app_for_msg     = app_for_ch.clone();

            // Ground Station will check if the message received matched its desired role (Client)
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

                    if data["type"] == "role-announcement" && data["role"] == "client" {
                        // Assign "client1", "client2", ...
                        let node_name = {
                            let mut count = node_count.lock().unwrap();
                            *count += 1;
                            format!("client{}", count)
                        };

                        // Describes Client peers that are ready to be connected
                        let assignment = serde_json::json!({
                            "type": "assignment",
                            "name": node_name,
                        });
                        let _ = ch.send(&Bytes::from(assignment.to_string())).await;

                        // Adds assignment to conenctions
                        connections.lock().unwrap().push(ch.clone());

                        let n = connections.lock().unwrap().len();
                        println!("  Status : Connected — {} Node(s)", n);

                        // Notify the frontend so it can render the new peer entry
                        let _ = app.emit("new-client", serde_json::json!({
                            "id":   node_name,
                            "role": "Client",
                        }));

                    } else if data.get("type").is_none() {
                        // Channel closes for unrecognized entries
                        let _ = ch.close().await;
                    }
                })
            }));

            let connections_cl = connections.clone();
            
            // Process for Ground Station when channel closes
            channel.on_close(Box::new(move || {
                let connections = connections_cl.clone();
                Box::pin(async move {
                    // Retains all channels that are still open
                    connections.lock().unwrap()
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

    // Forward the candidates to the Node via signaling
    let signal_tx_ice = signal_tx.clone();
    let gs_id_ice     = gs_id.clone();
    let node_id_ice   = node_id.clone();

    // Ground Station will have Client peers forwarded to them as trickle ICE candidates
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

    // Ground Station sends a created answer to the node
    let answer = peer.create_answer(None).await.unwrap();
    peer.set_local_description(answer.clone()).await.unwrap();

    let _ = signal_tx.send(SignalMsg::Answer {
        from: gs_id,
        to:   node_id,
        sdp:  answer.sdp,
    });
}

#[tauri::command]
pub async fn gs_disconnect(
    state: State<'_, Arc<Mutex<Option<GsState>>>>,
) -> Result<(), String> {

    // Mutex guard enabled
    let gs_opt = {
        let mut s = state.lock().unwrap();
        s.take()
    };

    // Must extract to a Vec before transitoning to await
    if let Some(gs) = gs_opt {        
        let channels: Vec<_> = {
            let conns = gs.connections.lock().unwrap();
            conns.clone()
        };
        
        for ch in channels {
            let _ = ch.close().await;
        }
        
        let peers: Vec<_> = gs.peer_map.iter().map(|entry| entry.value().clone()).collect();
        
        for peer in peers {
            let _ = peer.close().await;
        }
    }
    
    Ok(())
}