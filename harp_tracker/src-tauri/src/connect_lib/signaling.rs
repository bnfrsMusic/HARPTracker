// Peers exchange offers/answers and ICE candidates over a WebSocket

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tauri::async_runtime::spawn;

pub const SIGNAL_SERVER: &str = "ws://127.0.0.1:9000";

// Every message carries a `type` so the server can route it.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]

pub enum SignalMsg {
    /// Send id once on connect
    Register { id: String },

    /// C to GS WebRTC offer
    Offer {
        from: String,
        to: String,
        sdp: String,
    },

    /// GS to C WebRTC answer
    Answer {
        from: String,
        to: String,
        sdp: String,
    },

    /// Trickle ICE candidate
    Ice {
        from:              String,
        to:                String,
        candidate:         String,
        sdp_mid:           Option<String>,
        sdp_mline_index:   Option<u16>,
    },

    /// Server to code registration
    Registered { id: String },

    /// Server to code routing error
    Error { message: String },
}

// Connect to the signaling server and register our ID
pub async fn connect_signaling(
    my_id: &str,
) -> Result<(mpsc::UnboundedSender<SignalMsg>, mpsc::UnboundedReceiver<SignalMsg>), String> {
    
    let (ws, _) = connect_async(SIGNAL_SERVER)
        .await
        .map_err(|e| format!("Cannot reach signal server: {}", e))?;

    let (mut ws_tx, mut ws_rx) = ws.split();

    // Register Id
    let reg_json = serde_json::to_string(&SignalMsg::Register {
        id: my_id.to_owned(),
    }).map_err(|e| e.to_string())?;
    
    ws_tx.send(Message::Text(reg_json)).await.map_err(|e| e.to_string())?;

    // Create out_tx / out_rx (code to server)
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<SignalMsg>();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Create in_tx / in_rx (server to code)
    let (in_tx, in_rx) = mpsc::unbounded_channel::<SignalMsg>();
    tauri::async_runtime::spawn(async move {
        while let Some(Ok(Message::Text(txt))) = ws_rx.next().await {
            if let Ok(msg) = serde_json::from_str::<SignalMsg>(&txt) {
                let _ = in_tx.send(msg);
            }
        }
    });

    // Return successfully
    Ok((out_tx, in_rx))
}

// Holds the live signaling channels in Tauri app state so can be passed to frontend
pub struct SignalingState {
    pub tx: Mutex<Option<mpsc::UnboundedSender<SignalMsg>>>,
    pub rx: Mutex<Option<mpsc::UnboundedReceiver<SignalMsg>>>,
}

impl SignalingState {
    pub fn new() -> Self {
        Self {
            tx: Mutex::new(None),
            rx: Mutex::new(None),
        }
    }
}