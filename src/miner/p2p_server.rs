// //1st terminal cargo run
// //2nd terminal HTTP_PORT=3002 P2P_PORT=5002 PEERS=127.0.0.1:5001 cargo run
// //3rd terminal HTTP_PORT=3003 P2P_PORT=5003 PEERS=127.0.0.1:5001,127.0.0.1:5002 cargo run
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, TreeChain};
use TreeChainTheorey::treechain;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

const MESSAGE_TYPE_CONNECTION_INFO: &str = "CONNECTION_INFO";
const MESSAGE_TYPE_GETBLOCKS: &str = "GETBLOCKS";

pub struct P2PServer {
    pub treechain: Arc<Mutex<TreeChain>>,
    pub pqp: Arc<Mutex<PQP>>,
    pub peers: Vec<String>,
    // Store only writers here (readers are managed by message_handler tasks)
    pub writers: Arc<Mutex<Vec<Arc<Mutex<OwnedWriteHalf>>>>>,
}

impl P2PServer {
    pub fn new(treechain: Arc<Mutex<TreeChain>>, pqp: Arc<Mutex<PQP>>, peers: Vec<String>) -> Self {
        P2PServer {
            treechain,
            pqp,
            peers,
            writers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn build_block_locator(&self) -> Vec<String> {
        println!("Trying to lock treechain");
        let treechain = self.treechain.lock().await;
        println!("Locked treechain");
        let mut hashes: Vec<String> = treechain.blocks.keys().cloned().collect();
        hashes.reverse();
        hashes.truncate(10);
        drop(treechain);
        println!("Dropped treechain lock");
        hashes
    }

    async fn send_getblocks(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("get blocks called");
        let block_locator = self.build_block_locator().await;
        println!("block_locator in send_getblocks: {:?}", block_locator);
        let stopping_hash = "0".repeat(64);

        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GETBLOCKS,
            "protocol_version": 70002,
            "block_locator": block_locator,
            "stopping_hash": stopping_hash
        });

        let message_bytes = match serde_json::to_vec(&message) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to serialize GETBLOCKS message: {}", e);
                return;
            }
        };

        let len = message_bytes.len() as u32;
        println!("msg bytes len: {}", len);

        let mut locked_writer = writer.lock().await;
        println!("writer locked");

        if let Err(e) = locked_writer.write_all(&len.to_be_bytes()).await {
            eprintln!("Failed to send length for GETBLOCKS: {}", e);
            return;
        }
        if let Err(e) = locked_writer.write_all(&message_bytes).await {
            eprintln!("Failed to send GETBLOCKS message: {}", e);
            return;
        }
        if let Err(e) = locked_writer.flush().await {
            eprintln!("Failed to flush GETBLOCKS message: {}", e);
            return;
        }

        println!(
            "Sent GETBLOCKS message with {} hashes",
            message["block_locator"].as_array().unwrap().len()
        );
    }

    pub async fn listen(self: Arc<Self>, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

        println!("Listening for peer to peer connections on {}", addr);
        self.clone().connect_to_peers().await;

        loop {
            let (stream, peer_addr) = listener.accept().await.unwrap();
            let self_clone = self.clone();
            tokio::spawn(async move {
                self_clone
                    .connect_socket(stream, peer_addr.to_string())
                    .await;
            });
        }
    }

    async fn connect_to_peers(self: Arc<Self>) {
        let peers = self.peers.clone();
        println!("peers in connect to peers: {:?}", peers);
        for peer in peers {
            println!("connect to peer: {:?}", peer);
            let result = timeout(Duration::from_secs(5), TcpStream::connect(&peer)).await;
            match result {
                Ok(Ok(stream)) => {
                    println!("Connected to peer {}", peer);
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        self_clone.connect_socket(stream, peer).await;
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to connect to peer {}: {}", peer, e);
                }
                Err(_) => {
                    eprintln!("Timeout while connecting to peer: {}", peer);
                }
            }
        }
    }

    async fn connect_socket(self: Arc<Self>, socket: TcpStream, peer_addr: String) {
        println!("New connection from {}", peer_addr);

        let (reader, writer) = socket.into_split();
        let writer = Arc::new(Mutex::new(writer));
        self.writers.lock().await.push(writer.clone());

        // Send initial CONNECTION_INFO
        {
            let treechain = self.treechain.lock().await;
            let chain_length = treechain.blocks.len();
            let last_block = treechain
                .blocks
                .get_index(chain_length - 1)
                .map(|(_, v)| v.clone());
            let message = serde_json::json!({
                "type": MESSAGE_TYPE_CONNECTION_INFO,
                "chain_length": chain_length,
                "last_block": last_block
            });

            let message_bytes = serde_json::to_vec(&message).unwrap();
            let len = message_bytes.len() as u32;

            let mut locked_writer = writer.lock().await;
            if let Err(e) = locked_writer.write_all(&len.to_be_bytes()).await {
                eprintln!("Failed to send message length to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_writer.write_all(&message_bytes).await {
                eprintln!("Failed to send treechain data to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_writer.flush().await {
                eprintln!("Failed to flush socket to {}: {}", peer_addr, e);
                return;
            }
            println!(
                "Sent chain_length: {}, last_block to {}",
                chain_length, peer_addr
            );
        }

        // Spawn message handler for the read half
        let self_clone = Arc::clone(&self);
        tokio::spawn(async move {
            self_clone
                .message_handler(reader, writer.clone(), peer_addr)
                .await;
        });
    }

    async fn message_handler(
        &self,
        mut reader: OwnedReadHalf,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        peer_addr: String,
    ) {
        let mut buffer = vec![0u8; 4096];
        let mut received_data = Vec::new();

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Connection closed by peer: {}", peer_addr);
                    self.remove_writer(writer.clone()).await;
                    break;
                }
                Ok(n) => {
                    println!("Received {} bytes from {}", n, peer_addr);
                    received_data.extend_from_slice(&buffer[..n]);
                    while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                        received_data = remaining;
                        match serde_json::from_value::<Value>(msg) {
                            Ok(data) => {
                                if let Some(msg_type) = data.get("type").and_then(|v| v.as_str()) {
                                    match msg_type {
                                        MESSAGE_TYPE_CONNECTION_INFO => {
                                            let chain_length_remote = data
                                                .get("chain_length")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0);

                                            let chain_local_len = {
                                                let treechain = self.treechain.lock().await;
                                                treechain.blocks.len()
                                            };

                                            println!(
                                                "CONNECTION_INFO from {}: remote_len={}, local_len={}",
                                                peer_addr, chain_length_remote, chain_local_len
                                            );

                                            if chain_length_remote > chain_local_len as u64 {
                                                println!(
                                                    "Peer has longer chain, sending GETBLOCKS"
                                                );
                                                self.send_getblocks(writer.clone()).await;
                                            } else {
                                                println!(
                                                    "Our chain is equal or longer, no GETBLOCKS needed"
                                                );
                                            }
                                        }
                                        MESSAGE_TYPE_GETBLOCKS => {
                                            let block_locator = data
                                                .get("block_locator")
                                                .and_then(|v| v.as_array())
                                                .map(|arr| {
                                                    arr.iter()
                                                        .filter_map(|v| {
                                                            v.as_str().map(String::from)
                                                        })
                                                        .collect::<Vec<String>>()
                                                })
                                                .unwrap_or_default();
                                            println!(
                                                "Received GETBLOCKS from {} with block locator: {:?}",
                                                peer_addr, block_locator
                                            );
                                        }
                                        _ => {
                                            println!("Other type from {}: {:?}", peer_addr, data);
                                        }
                                    }
                                }
                            }
                            Err(e) => eprintln!("Invalid JSON from {}: {}", peer_addr, e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Read error from {}: {}", peer_addr, e);
                    self.remove_writer(writer.clone()).await;
                    break;
                }
            }
        }
    }

    async fn remove_writer(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let mut writers = self.writers.lock().await;
        writers.retain(|w| !Arc::ptr_eq(w, &writer));
        println!("Removed disconnected writer");
    }

    fn parse_message(data: &[u8]) -> Option<(Value, Vec<u8>)> {
        if data.len() < 4 {
            return None;
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            return None;
        }
        let msg_data = &data[4..4 + len];
        match serde_json::from_slice::<Value>(msg_data) {
            Ok(msg) => Some((msg, data[4 + len..].to_vec())),
            Err(_) => None,
        }
    }
}

pub fn start_p2p_server(
    port: String,
    peers: String,
    treechain: Arc<Mutex<TreeChain>>,
    pqp: Arc<Mutex<PQP>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        println!("peers: {}", peers);
        runtime.block_on(async move {
            let peer_list: Vec<String> = peers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let server = P2PServer::new(treechain, pqp, peer_list);
            Arc::new(server)
                .listen(port.parse::<u16>().expect("Invalid port number"))
                .await;
        })
    })
}
