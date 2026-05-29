// ── node.rs ───────────────────────────────────────────────────────────────────
// Translation of the Node branch:
//   btnNode click → initPeer('node') → initiateNodeConnection()
//                → setupConnectionListeners(conn, "gs")

use bytes::Bytes;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::connect_lib::{
    gen::{generate_human_id, return_id, set_status},
    peer_factory::create_peer,
    signaling::{connect_signaling, SignalMsg},
};

use tauri::Emitter;

/// JS: btnNode click → initPeer('node') + initiateNodeConnection()
#[tauri::command]
pub async fn client_run(app: tauri::AppHandle) -> Result<String, String> {
    let my_id = generate_human_id();

    let id_for_task = my_id.clone();

    tauri::async_runtime::spawn(async move {
        let gs_id = {
            use tokio::io::AsyncBufReadExt;
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
            let mut line   = String::new();
            reader.read_line(&mut line).await.unwrap();
            line.trim().to_owned()
        };

        if gs_id.is_empty() {
            eprintln!("  No GS ID provided — exiting.");
            return;
        }

        println!("\n  Status  : Linking to GS {}…", gs_id);

        // ── Connect to signaling server ───────────────────────────────────────────
        // JS: new Peer(humanId) — which connects to PeerServer internally
        let (signal_tx, mut signal_rx) = match connect_signaling(&id_for_task).await {
        Ok(channels) => channels,
        Err(e) => {
            // Emit the error to your JavaScript UI
                app.emit("client-error", serde_json::json!({ "message": e })).unwrap();
                return; // Stop running the command, but leave the app alive!
            }
        };

        // ── Create our WebRTC peer connection ─────────────────────────────────────
        // JS: peer.connect(gsId)
        let peer = create_peer().await;

        // ── Open a data channel toward the GS ────────────────────────────────────
        // In WebRTC the *caller* (Node) creates the data channel; the *answerer*
        // (GS) receives it via on_data_channel.  This mirrors PeerJS's conn object.
        let channel = peer
            .create_data_channel("main", None)
            .await
            .unwrap();

        // Clone before moving into the closure
        let ch_open = channel.clone();

        // JS: conn.on("open", () => conn.send({ type:"role-announcement", role:"node" }))
        channel.on_open(Box::new(move || {
            let ch = ch_open.clone();
            Box::pin(async move {
                let announcement = serde_json::json!({
                    "type": "role-announcement",
                    "role": "node",
                });
                let _ = ch
                    .send(&Bytes::from(announcement.to_string()))
                    .await;
            })
        }));

        // JS: setupConnectionListeners(conn, "gs")
        //     → conn.on("data") — handle the "assignment" message from GS
        channel.on_message(Box::new(|msg| {
            Box::pin(async move {
                let text = String::from_utf8(msg.data.to_vec()).unwrap_or_default();
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    if data["type"] == "assignment" {
                        if let Some(name) = data["name"].as_str() {
                            // JS: setStatus(`Linked to GS as ${data.name}`, "connected")
                            println!("  Status  : Linked to GS as {}", name);
                        }
                    }
                }
            })
        }));
        

        // JS: conn.on("close") → setStatus("Disconnected from GS", "disconnected")
        channel.on_close(Box::new(|| {
            Box::pin(async {
                println!("  Status  : Disconnected from GS");
            })
        }));

        // ── Trickle-ICE: forward our candidates to the GS via signaling ───────────
        let signal_tx_ice = signal_tx.clone();
        let node_id_ice   = id_for_task.clone();
        let gs_id_ice     = gs_id.clone();
        peer.on_ice_candidate(Box::new(move |candidate| {
            let tx   = signal_tx_ice.clone();
            let from = node_id_ice.clone();
            let to   = gs_id_ice.clone();
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

        // ── Create and send SDP offer to the GS ──────────────────────────────────
        // JS: peer.connect(gsId) internally calls createOffer → setLocalDescription
        //     → send to peer server → GS receives "connection" event
        let offer = peer.create_offer(None).await.unwrap();
        peer.set_local_description(offer.clone()).await.unwrap();

        signal_tx
            .send(SignalMsg::Offer {
                from: id_for_task.clone(),
                to:   gs_id.clone(),
                sdp:  offer.sdp,
            })
            .unwrap();

        // ── Signaling loop: handle answer + ICE from GS ───────────────────────────
        while let Some(msg) = signal_rx.recv().await {
            match msg {
                // GS replied with an SDP answer
                SignalMsg::Answer { sdp, .. } => {
                    let answer = RTCSessionDescription::answer(sdp).unwrap();
                    peer.set_remote_description(answer).await.unwrap();
                }

                // GS sent an ICE candidate
                SignalMsg::Ice { candidate, sdp_mid, sdp_mline_index, .. } => {
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
                }

                _ => {}
            }
        }
    });

    Ok(my_id)
}