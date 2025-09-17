// ports_server.rs

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

struct PeerData {
    chain_length: u64,
    last_timestamp: u64,
}

struct PortsServer {
    peers: Arc<Mutex<HashMap<String, PeerData>>>,
    writers: Arc<Mutex<HashMap<String, Arc<Mutex<OwnedWriteHalf>>>>>,
}

impl PortsServer {
    fn new() -> Self {
        PortsServer {
            peers: Arc::new(Mutex::new(HashMap::new())),
            writers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn send_message(writer: Arc<Mutex<OwnedWriteHalf>>, message: &Value, tag: &str) {
        let message_bytes = match serde_json::to_vec(message) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to serialize {} message: {}", tag, e);
                return;
            }
        };
        let len = message_bytes.len() as u32;
        let mut locked_writer = writer.lock().await;
        if let Err(e) = locked_writer.write_all(&len.to_be_bytes()).await {
            eprintln!("Failed to write length for {} message: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.write_all(&message_bytes).await {
            eprintln!("Failed to write {} message: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.flush().await {
            eprintln!("Failed to flush {} message: {}", tag, e);
            return;
        }
        println!("Successfully sent {} message ({} bytes)", tag, len);
    }

    fn parse_message(data: &[u8]) -> Option<(Value, Vec<u8>)> {
        if data.len() < 4 {
            println!(
                "Received data too short for message length: {} bytes",
                data.len()
            );
            return None;
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            println!(
                "Received data incomplete: {} bytes, need {}",
                data.len(),
                4 + len
            );
            return None;
        }
        let msg_data = &data[4..4 + len];
        match serde_json::from_slice(msg_data) {
            Ok(v) => {
                println!("Parsed message successfully: {:?}", v);
                Some((v, data[4 + len..].to_vec()))
            }
            Err(e) => {
                eprintln!("Failed to parse message: {}", e);
                None
            }
        }
    }

    async fn broadcast_peer_list(&self) {
        let peers_lock = self.peers.lock().await;
        let peers_json = peers_lock
            .iter()
            .map(|(addr, data)| {
                serde_json::json!({
                    "addr": addr,
                    "chain_length": data.chain_length,
                    "last_timestamp": data.last_timestamp,
                })
            })
            .collect::<Vec<_>>();
        let message = serde_json::json!({"type": "PEER_LIST", "peers": peers_json});
        let writers = self
            .writers
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        println!(
            "Broadcasting PEER_LIST to {} writers with {} peers",
            writers.len(),
            peers_json.len()
        );
        for w in writers {
            Self::send_message(w, &message, "PEER_LIST").await;
        }
    }

    async fn send_peer_list(&self, writer: &Arc<Mutex<OwnedWriteHalf>>) {
        println!("Called send_peer_list");
        let peers_lock = self.peers.lock().await;
        let peers_json = peers_lock
            .iter()
            .map(|(addr, data)| {
                serde_json::json!({
                    "addr": addr,
                    "chain_length": data.chain_length,
                    "last_timestamp": data.last_timestamp,
                })
            })
            .collect::<Vec<_>>();
        let message = serde_json::json!({"type": "PEER_LIST", "peers": peers_json});
        println!(
            "Sending PEER_LIST with {} peers: {}",
            peers_json.len(),
            message
        );
        Self::send_message(writer.clone(), &message, "PEER_LIST").await;
    }

    async fn handle_connection(self: Arc<Self>, stream: TcpStream) {
        let (mut reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(writer));
        let mut addr: Option<String> = None;
        let mut buffer = vec![0u8; 4096];
        let mut received_data = Vec::new();
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Connection closed for {:?}", addr);
                    break;
                }
                Ok(n) => {
                    received_data.extend_from_slice(&buffer[..n]);
                    while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                        received_data = remaining;
                        let data: Value = serde_json::from_value(msg).unwrap_or_default();
                        let msg_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        println!("Received message type: {}", msg_type);
                        match msg_type {
                            "REGISTER" => {
                                println!("Received REGISTER message: {:?}", data);
                                let new_addr = data
                                    .get("addr")
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                                    .unwrap_or_default();
                                let chain_length = data
                                    .get("chain_length")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let last_timestamp = data
                                    .get("last_timestamp")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                if !new_addr.is_empty() {
                                    let mut peers = self.peers.lock().await;
                                    let mut writers = self.writers.lock().await;
                                    println!(
                                        "Registering peer {}: chain_length={}, last_timestamp={}",
                                        new_addr, chain_length, last_timestamp
                                    );
                                    peers.insert(
                                        new_addr.clone(),
                                        PeerData {
                                            chain_length,
                                            last_timestamp,
                                        },
                                    );
                                    writers.insert(new_addr.clone(), writer.clone());
                                    addr = Some(new_addr.clone());
                                    drop(peers);
                                    drop(writers);
                                    println!("Calling broadcast_peer_list after REGISTER");
                                    self.broadcast_peer_list().await;
                                } else {
                                    println!("Invalid or empty address in REGISTER message");
                                }
                            }
                            "UPDATE" => {
                                if let Some(a) = &addr {
                                    let chain_length = data
                                        .get("chain_length")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    let last_timestamp = data
                                        .get("last_timestamp")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    let mut peers = self.peers.lock().await;
                                    if let Some(p) = peers.get_mut(a) {
                                        p.chain_length = chain_length;
                                        p.last_timestamp = last_timestamp;
                                        println!(
                                            "Updated peer {}: chain_length={}, last_timestamp={}",
                                            a, chain_length, last_timestamp
                                        );
                                    }
                                    drop(peers);
                                    // println!("Broadcasting PEER_LIST after UPDATE");
                                    // self.broadcast_peer_list().await;
                                }
                            }
                            "GET_PEER_LIST" => {
                                println!("Received GET_PEER_LIST from {:?}", addr);
                                self.send_peer_list(&writer).await;
                            }
                            _ => {
                                println!("Unknown message type: {}", msg_type);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from socket for {:?}: {}", addr, e);
                    break;
                }
            }
        }
        if let Some(a) = addr {
            let mut peers = self.peers.lock().await;
            let mut writers = self.writers.lock().await;
            peers.remove(&a);
            writers.remove(&a);
            println!("Removed peer {} from registry", a);
            drop(peers);
            drop(writers);
            self.broadcast_peer_list().await;
        }
    }
}

#[tokio::main]
async fn main() {
    let server = Arc::new(PortsServer::new());
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("Ports server listening on 127.0.0.1:8080");
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        println!("New connection from {}", addr);
        let server_clone = server.clone();
        tokio::spawn(async move {
            server_clone.handle_connection(stream).await;
        });
    }
}
