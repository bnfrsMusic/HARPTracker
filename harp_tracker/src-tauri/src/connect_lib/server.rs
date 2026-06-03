// Each peer sends its type, register, and id on connect.
// The server routes them to whichever WebSocket registered that ID.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, OnceLock},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

type PeerTx = mpsc::UnboundedSender<Message>;
pub type PeerMap = Arc<Mutex<HashMap<String, PeerTx>>>;

pub const SIGNAL_PORT: u16 = 9000;

static PEER_DIRECTORY: OnceLock<PeerMap> = OnceLock::new();

pub fn peer_directory() -> Option<&'static PeerMap> {
    PEER_DIRECTORY.get()
}

/// Check whether a peer id is currently registered (works across app processes).
pub async fn is_peer_online(peer_id: &str) -> bool {
    let id = peer_id.trim();
    if id.is_empty() {
        return false;
    }
    let Some(peers) = PEER_DIRECTORY.get() else {
        return false;
    };
    peers.lock().await.contains_key(id)
}

pub async fn list_online_peers() -> Vec<String> {
    let Some(peers) = PEER_DIRECTORY.get() else {
        return vec![];
    };
    peers.lock().await.keys().cloned().collect()
}

#[tokio::main]
async fn main() {
    start_signaling_server().await;
}

pub async fn start_signaling_server() {
    if PEER_DIRECTORY.get().is_some() {
        println!("Signaling server already running in this process");
        return;
    }

    let addr: SocketAddr = format!("0.0.0.0:{}", SIGNAL_PORT).parse().unwrap();
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            println!(
                "Could not bind signaling server on ws://{} ({}). \
                 Another HARP Tracker instance may already own this port — \
                 peers in this process will use that shared server.",
                addr, e
            );
            return;
        }
    };

    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
    let _ = PEER_DIRECTORY.set(peers.clone());

    let lan = crate::connect_lib::network::list_lan_ipv4();
    println!("Signal server listening on ws://127.0.0.1:{} (all interfaces)", SIGNAL_PORT);
    for ip in &lan {
        println!("  Remote clients on your LAN should use: ws://{}:{}", ip, SIGNAL_PORT);
    }
    if lan.is_empty() {
        println!("  (No LAN IPv4 detected — remote devices may need manual network setup)");
    }
    println!("  Allow TCP port {} through the host firewall for remote connections", SIGNAL_PORT);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let peers = peers.clone();
        tokio::spawn(handle_connection(stream, peer_addr, peers));
    }
}

async fn handle_connection(stream: TcpStream, addr: SocketAddr, peers: PeerMap) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[{}] WebSocket handshake failed: {}", addr, e);
            return;
        }
    };
    let (mut ws_tx, mut ws_rx) = ws.split();

    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = conn_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut my_id: Option<String> = None;

    while let Some(Ok(Message::Text(txt))) = ws_rx.next().await {
        let Ok(json) = serde_json::from_str::<Value>(&txt) else {
            continue;
        };

        match json["type"].as_str() {
            Some("register") => {
                if let Some(id) = json["id"].as_str() {
                    let id = id.trim().to_string();
                    if id.is_empty() {
                        continue;
                    }
                    peers.lock().await.insert(id.clone(), conn_tx.clone());
                    my_id = Some(id.clone());
                    println!("[{}] registered as '{}'", addr, id);

                    let ack = serde_json::json!({ "type": "registered", "id": id });
                    let _ = conn_tx.send(Message::Text(ack.to_string()));
                }
            }

            Some("lookup") => {
                if let Some(id) = json["id"].as_str() {
                    let id = id.trim();
                    let online = peers.lock().await.contains_key(id);
                    let res = serde_json::json!({
                        "type": "lookup_result",
                        "id": id,
                        "online": online,
                    });
                    let _ = conn_tx.send(Message::Text(res.to_string()));
                }
            }

            Some("list") => {
                let online: Vec<_> = peers.lock().await.keys().cloned().collect();
                let res = serde_json::json!({
                    "type": "list_result",
                    "peers": online,
                });
                let _ = conn_tx.send(Message::Text(res.to_string()));
            }

            Some("offer") | Some("answer") | Some("ice") => {
                if let Some(to) = json["to"].as_str() {
                    let to = to.trim();
                    let map = peers.lock().await;
                    if let Some(dest_tx) = map.get(to) {
                        let _ = dest_tx.send(Message::Text(txt));
                    } else {
                        let online: Vec<_> = map.keys().cloned().collect();
                        eprintln!(
                            "Signaling route failed: peer '{}' not found (online: {:?})",
                            to, online
                        );
                        let err = serde_json::json!({
                            "type":    "error",
                            "message": format!("Peer '{}' not found", to),
                        });
                        let _ = conn_tx.send(Message::Text(err.to_string()));
                    }
                }
            }

            _ => {}
        }
    }

    if let Some(id) = my_id {
        peers.lock().await.remove(&id);
        println!("[{}] '{}' disconnected", addr, id);
    }
}
