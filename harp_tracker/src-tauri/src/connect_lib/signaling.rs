// Replaces the hidden PeerServer that PeerJS uses under the hood.
// Peers exchange SDP offers/answers and ICE candidates over a WebSocket

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tauri::async_runtime::spawn;

pub const SIGNAL_SERVER: &str = "ws://127.0.0.1:9000";

// Every message carries a `type` so the server can route it.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]

pub enum SignalMsg {
    /// Sent once on connect: "here is my human-readable ID"
    Register { id: String },

    /// Node → GS: WebRTC offer SDP
    Offer {
        from: String,
        to: String,
        sdp: String,
    },

    /// GS → Node: WebRTC answer SDP
    Answer {
        from: String,
        to: String,
        sdp: String,
    },

    /// trickle-ICE candidate
    Ice {
        from:              String,
        to:                String,
        candidate:         String,
        sdp_mid:           Option<String>,
        sdp_mline_index:   Option<u16>,
    },

    /// Server → client: registration confirmed
    Registered { id: String },

    /// Server → client: routing error
    Error { message: String },
}

// Connect to the signaling server and register our ID
pub async fn connect_signaling(
    my_id: &str,
) -> Result<(mpsc::UnboundedSender<SignalMsg>, mpsc::UnboundedReceiver<SignalMsg>), String> {
    
    // 1. Connect and handle errors gracefully
    let (ws, _) = connect_async(SIGNAL_SERVER)
        .await
        .map_err(|e| format!("Cannot reach signal server: {}", e))?;

    let (mut ws_tx, mut ws_rx) = ws.split();

    // 2. Register immediately
    let reg_json = serde_json::to_string(&SignalMsg::Register {
        id: my_id.to_owned(),
    }).map_err(|e| e.to_string())?;
    
    ws_tx.send(Message::Text(reg_json)).await.map_err(|e| e.to_string())?;

    // 3. CREATE out_tx / out_rx (Code to server)
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<SignalMsg>();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // 4. CREATE in_tx / in_rx (Server to code)
    let (in_tx, in_rx) = mpsc::unbounded_channel::<SignalMsg>();
    tauri::async_runtime::spawn(async move {
        while let Some(Ok(Message::Text(txt))) = ws_rx.next().await {
            if let Ok(msg) = serde_json::from_str::<SignalMsg>(&txt) {
                let _ = in_tx.send(msg);
            }
        }
    });

    // 5. Return them successfully!
    Ok((out_tx, in_rx))
}