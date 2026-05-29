// Each peer sends its type, register, and idon connect.
// The server routes them to whichever WebSocket registered that ID.
 
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
 
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
 
type PeerTx = mpsc::UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<String, PeerTx>>>;
 
#[tokio::main]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Signal server listening on ws://{}", addr);
 
    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
 
    while let Ok((stream, peer_addr)) = listener.accept().await {
        let peers = peers.clone();
        tokio::spawn(handle_connection(stream, peer_addr, peers));
    }
}
 
async fn handle_connection(stream: TcpStream, addr: SocketAddr, peers: PeerMap) {
    let ws = accept_async(stream).await.unwrap();
    let (mut ws_tx, mut ws_rx) = ws.split();
 
    // We need to send from the router task, so put ws_tx behind a channel
    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<Message>();
 
    tokio::spawn(async move {
        while let Some(msg) = conn_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });
 
    let mut my_id: Option<String> = None;
 
    // Reading loop
    while let Some(Ok(Message::Text(txt))) = ws_rx.next().await {
        let Ok(json) = serde_json::from_str::<Value>(&txt) else {
            continue;
        };
 
        match json["type"].as_str() {
            // Client registers its human-readable ID
            Some("register") => {
                if let Some(id) = json["id"].as_str() {
                    let id = id.to_owned();
                    peers.lock().await.insert(id.clone(), conn_tx.clone());
                    my_id = Some(id.clone());
                    println!("[{}] registered as '{}'", addr, id);
 
                    let ack = serde_json::json!({ "type": "registered", "id": id });
                    let _ = conn_tx.send(Message::Text(ack.to_string()));
                }
            }
 
            // Any message with a `to` field gets routed
            Some("offer") | Some("answer") | Some("ice") => {
                if let Some(to) = json["to"].as_str() {
                    let map = peers.lock().await;
                    if let Some(dest_tx) = map.get(to) {
                        let _ = dest_tx.send(Message::Text(txt));
                    } else {
                        // Destination not found, so send error
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
 
    // Cleanup on disconnect
    if let Some(id) = my_id {
        peers.lock().await.remove(&id);
        println!("[{}] '{}' disconnected", addr, id);
    }
}

pub async fn start_signaling_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9000").await.unwrap();
}