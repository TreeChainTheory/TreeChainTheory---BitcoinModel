// //1st terminal cargo run
// //2nd terminal HTTP_PORT=3002 P2P_PORT=5002 PEERS=127.0.0.1:5001 cargo run
// //3rd terminal HTTP_PORT=3003 P2P_PORT=5003 PEERS=127.0.0.1:5001,127.0.0.1:5002 cargo run
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, TreeChain};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

const MESSAGE_TYPE_CONNECTION_INFO: &str = "CONNECTION_INFO";
pub struct P2PServer {
    pub treechain: Arc<Mutex<TreeChain>>,
    pub pqp: Arc<Mutex<PQP>>,
    pub peers: Vec<String>,
    pub sockets: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>,
}

impl P2PServer {
    pub fn new(treechain: Arc<Mutex<TreeChain>>, pqp: Arc<Mutex<PQP>>, peers: Vec<String>) -> Self {
        P2PServer {
            treechain,
            pqp,
            peers,
            sockets: Arc::new(Mutex::new(Vec::new())),
        }
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
        let socket = Arc::new(Mutex::new(socket));
        self.sockets.lock().await.push(socket.clone());

        // Send initial message in a separate scope to drop the MutexGuard
        {
            let treechain = self.treechain.lock().await;
            let chain_length = treechain.blocks.len();
            let last_block = treechain
                .blocks
                .get_index(chain_length - 1)
                .map(|(_, v)| v.clone());
            let message = serde_json::json!({
                "type":MESSAGE_TYPE_CONNECTION_INFO,
                "chain_length": chain_length,
                "last_block": last_block
            });

            let message_bytes = match serde_json::to_vec(&message) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Failed to serialize message for {}: {}", peer_addr, e);
                    return;
                }
            };
            let len = message_bytes.len() as u32;
            let mut locked_socket = socket.lock().await;
            if let Err(e) = locked_socket.write_all(&len.to_be_bytes()).await {
                eprintln!("Failed to send message length to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_socket.write_all(&message_bytes).await {
                eprintln!("Failed to send treechain data to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_socket.flush().await {
                eprintln!("Failed to flush socket to {}: {}", peer_addr, e);
                return;
            }
            println!(
                "Sent chain_length: {}, last_block to {}",
                chain_length, peer_addr
            );
        } // locked_socket is dropped here

        let self_clone = Arc::clone(&self);
        tokio::spawn(async move {
            self_clone.message_handler(socket, peer_addr).await;
        });
    }

    async fn message_handler(&self, socket: Arc<Mutex<TcpStream>>, peer_addr: String) {
        let mut buffer = vec![0u8; 4096];
        let mut received_data = Vec::new();

        loop {
            let locked_socket = socket.lock().await;
            match locked_socket.readable().await {
                Ok(_) => match locked_socket.try_read(&mut buffer) {
                    Ok(0) => {
                        println!("Connection closed by peer: {}", peer_addr);
                        let socket_clone = socket.clone();
                        self.remove_socket(socket_clone).await;
                        break;
                    }
                    Ok(n) => {
                        println!("Received {} bytes from {}", n, peer_addr);
                        received_data.extend_from_slice(&buffer[..n]);
                        while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                            received_data = remaining;
                            match serde_json::from_value::<Value>(msg) {
                                Ok(data) => {
                                    if let Some(msg_type) =
                                        data.get("type").and_then(|v| v.as_str())
                                    {
                                        match msg_type {
                                            MESSAGE_TYPE_CONNECTION_INFO => {
                                                let chain_length = data.get("chain_length");
                                                let last_block = data.get("last_block");
                                                println!(
                                                    "CONNECTION_INFO from {}: chain_length={:?}, last_block={:?}",
                                                    peer_addr, chain_length, last_block
                                                );
                                            }
                                            _ => {
                                                println!(
                                                    "Other type from {}: {:?}",
                                                    peer_addr, data
                                                );
                                            }
                                        }
                                    } else {
                                        println!(
                                            "No 'type' field in message from {}: {:?}",
                                            peer_addr, data
                                        );
                                    }
                                }
                                Err(e) => eprintln!("Invalid JSON from {}: {}", peer_addr, e),
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => {
                        eprintln!("Read error from {}: {}", peer_addr, e);
                        let socket_clone = socket.clone();
                        self.remove_socket(socket_clone).await;
                        break;
                    }
                },
                Err(e) => {
                    eprintln!("Socket error from {}: {}", peer_addr, e);
                    let socket_clone = socket.clone();
                    self.remove_socket(socket_clone).await;
                    break;
                }
            }
        }
    }

    async fn remove_socket(&self, socket: Arc<Mutex<TcpStream>>) {
        let mut sockets = self.sockets.lock().await;
        sockets.retain(|s| !Arc::ptr_eq(s, &socket));
        println!("Removed disconnected socket");
    }

    fn parse_message(data: &[u8]) -> Option<(Value, Vec<u8>)> {
        if data.len() < 4 {
            println!("Incomplete message: only {} bytes received", data.len());
            return None;
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            println!(
                "Incomplete message: need {} bytes, have {}",
                4 + len,
                data.len()
            );
            return None;
        }
        let msg_data = &data[4..4 + len];
        match serde_json::from_slice::<Value>(msg_data) {
            Ok(msg) => Some((msg, data[4 + len..].to_vec())),
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
                None
            }
        }
    }
}

pub fn start_p2p_server(port: String, peers: String) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        println!("peers: {}", peers);
        runtime.block_on(async move {
            let treechain = Arc::new(Mutex::new(TreeChain::new()));
            let pqp = Arc::new(Mutex::new(PQP::new()));
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
