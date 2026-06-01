// Peers exchange offers/answers and ICE candidates over a WebSocket

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

pub const SIGNAL_SERVER: &str = "ws://127.0.0.1:9000";

const CONNECT_RETRIES: u32 = 40;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);
const REGISTER_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalMsg {
    Register { id: String },
    Offer {
        from: String,
        to: String,
        sdp: String,
    },
    Answer {
        from: String,
        to: String,
        sdp: String,
    },
    Ice {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    Registered { id: String },
    LookupResult { id: String, online: bool },
    ListResult { peers: Vec<String> },
    Error { message: String },
}

/// Live signaling session. Call [`SignalingConnection::close`] to unregister from the server.
pub struct SignalingConnection {
    pub tx: mpsc::UnboundedSender<SignalMsg>,
    pub rx: mpsc::UnboundedReceiver<SignalMsg>,
    pub cancel: CancellationToken,
}

impl SignalingConnection {
    pub fn close(&self) {
        self.cancel.cancel();
    }
}

async fn connect_ws() -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    let mut last_err = String::from("unknown error");
    for attempt in 0..CONNECT_RETRIES {
        match connect_async(SIGNAL_SERVER).await {
            Ok((ws, _)) => return Ok(ws),
            Err(e) => {
                last_err = format!("Cannot reach signal server: {}", e);
                if attempt + 1 < CONNECT_RETRIES {
                    sleep(CONNECT_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err)
}

/// One-shot lookup (does not register this connection).
pub async fn lookup_peer_online(peer_id: &str) -> Result<bool, String> {
    let peer_id = peer_id.trim();
    if peer_id.is_empty() {
        return Ok(false);
    }

    let ws = connect_ws().await?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    let lookup = serde_json::json!({ "type": "lookup", "id": peer_id });
    ws_tx
        .send(Message::Text(lookup.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    let result = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(msg) = ws_rx.next().await {
            let Ok(Message::Text(txt)) = msg else {
                break;
            };
            if let Ok(parsed) = serde_json::from_str::<SignalMsg>(&txt) {
                if let SignalMsg::LookupResult { id, online } = parsed {
                    if id.trim() == peer_id {
                        return Ok::<bool, String>(online);
                    }
                }
            }
        }
        Ok::<bool, String>(false)
    })
    .await
    .map_err(|_| "Timed out checking peer online status".to_string())??;

    Ok(result)
}

/// Query all registered peer ids from the shared signaling server.
pub async fn list_peers_remote() -> Result<Vec<String>, String> {
    let ws = connect_ws().await?;
    let (mut ws_tx, mut ws_rx) = ws.split();
    ws_tx
        .send(Message::Text(r#"{"type":"list"}"#.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(msg) = ws_rx.next().await {
            let Ok(Message::Text(txt)) = msg else {
                break;
            };
            if let Ok(SignalMsg::ListResult { peers }) = serde_json::from_str(&txt) {
                return Ok::<Vec<String>, String>(peers);
            }
        }
        Ok::<Vec<String>, String>(vec![])
    })
    .await
    .map_err(|_| "Timed out listing signaling peers".to_string())?
}

/// Wait until `peer_id` appears on the shared signaling server.
pub async fn wait_for_peer_online(peer_id: &str, timeout: Duration) -> Result<(), String> {
    let peer_id = peer_id.trim();
    if peer_id.is_empty() {
        return Err("Peer id cannot be empty".into());
    }

    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if lookup_peer_online(peer_id).await? {
            return Ok(());
        }
        sleep(Duration::from_millis(400)).await;
    }

    let online = list_peers_remote().await.unwrap_or_default();
    Err(format!(
        "Peer '{}' never appeared on the signaling server (currently online: {:?}). \
         Start the Ground Station first and keep that app window open.",
        peer_id, online
    ))
}

/// Connect to the signaling server, register `my_id`, and wait until the server acknowledges registration.
pub async fn connect_signaling(my_id: &str) -> Result<SignalingConnection, String> {
    let my_id = my_id.trim().to_string();
    if my_id.is_empty() {
        return Err("Peer id cannot be empty".into());
    }

    let ws = connect_ws().await?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    let reg_json = serde_json::to_string(&SignalMsg::Register { id: my_id.clone() })
        .map_err(|e| e.to_string())?;
    ws_tx
        .send(Message::Text(reg_json))
        .await
        .map_err(|e| e.to_string())?;

    tokio::time::timeout(REGISTER_TIMEOUT, async {
        while let Some(msg) = ws_rx.next().await {
            let Ok(Message::Text(txt)) = msg else {
                return Err("Signaling connection closed before registration completed".into());
            };
            let Ok(parsed) = serde_json::from_str::<SignalMsg>(&txt) else {
                continue;
            };
            match parsed {
                SignalMsg::Registered { id } if id.trim() == my_id => return Ok(()),
                SignalMsg::Error { message } => return Err(message),
                _ => continue,
            }
        }
        Err("Signaling connection closed before registration completed".into())
    })
    .await
    .map_err(|_| "Timed out waiting for signaling registration".to_string())??;

    if !lookup_peer_online(&my_id).await? {
        return Err(format!(
            "Registered as '{}' but peer is not visible on the signaling server",
            my_id
        ));
    }

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<SignalMsg>();
    let (in_tx, in_rx) = mpsc::unbounded_channel::<SignalMsg>();
    let cancel = CancellationToken::new();
    let cancel_reader = cancel.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                () = cancel_reader.cancelled() => break,
                msg = ws_rx.next() => {
                    match msg {
                        Some(Ok(Message::Text(txt))) => {
                            if let Ok(parsed) = serde_json::from_str::<SignalMsg>(&txt) {
                                let _ = in_tx.send(parsed);
                            }
                        }
                        _ => break,
                    }
                }
            }
        }
    });

    Ok(SignalingConnection {
        tx: out_tx,
        rx: in_rx,
        cancel,
    })
}
