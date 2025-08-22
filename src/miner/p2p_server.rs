use crate::treechain::treechain::{PQP, TreeChain};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, timeout};

//1st terminal cargo run
//2nd terminal HTTP_PORT=3002 P2P_PORT=5002 PEERS=127.0.0.1:5001 cargo run
//3rd terminal HTTP_PORT=3003 P2P_PORT=5003 PEERS=127.0.0.1:5001,127.0.0.1:5002 cargo run
pub struct P2PServer {
    pub treechain: Arc<Mutex<TreeChain>>,
    pub pqp: Arc<Mutex<PQP>>,
    pub peers: Vec<String>,
    pub sockets: Vec<TcpStream>,
}

impl P2PServer {
    pub fn new(treechain: Arc<Mutex<TreeChain>>, pqp: Arc<Mutex<PQP>>, peers: Vec<String>) -> Self {
        P2PServer {
            treechain,
            pqp,
            peers,
            sockets: Vec::new(),
        }
    }

    pub async fn listen(&mut self, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

        println!("Listening for peer to peer connections on {}", addr);
        self.connect_to_peers().await;
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            self.connect_socket(stream).await;
        }
    }

    async fn connect_to_peers(&mut self) {
        let peers = self.peers.clone();
        println!("peers in connect to peers: {:?}", peers);
        for peer in peers {
            println!("connect to peer: {:?}", peer);
            let result = timeout(Duration::from_secs(5), TcpStream::connect(&peer)).await;
            match result {
                Ok(Ok(stream)) => {
                    println!("Connected to peers {} ", peer);
                    self.connect_socket(stream).await;
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

    async fn connect_socket(&mut self, socket: TcpStream) {
        println!("New connection from {}", socket.peer_addr().unwrap());
        self.sockets.push(socket);
    }
}

pub fn start_p2p_server(port: String, peers: String) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        println!("peers: {}", peers);
        runtime.block_on(async move {
            let treechain = Arc::new(Mutex::new(TreeChain::new()));
            let pqp = Arc::new(Mutex::new(PQP::new()));
            let peer_list: Vec<String> = peers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let mut server = P2PServer::new(treechain.clone(), pqp.clone(), peer_list);
            let port_num = port.parse::<u16>().expect("Invalid port number");
            server.listen(port_num).await;
        })
    })
}
