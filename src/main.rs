mod miner;

pub mod chain_util;
pub mod config;
pub mod treechain;
pub mod wallet;

use config::CHILDREN;
use treechain::block::Block;
use treechain::treechain::{PQP, TreeChain};

use actix_web::{App, HttpServer, web};
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use tokio::sync::Mutex;

use crate::miner::p2p_server::{self, start_p2p_server};

#[actix_web::main]
async fn main() {
    // let mut genisis_block = Block::genesis();
    // Block::calculate_hash_and_pqp_commitment(&mut genisis_block);
    // println!("Genisis Block: {:#?}", genisis_block);
    // println!("Genisis Block Hash: {}", genisis_block.hash);

    dotenv().ok();

    fn get_align() -> u8 {
        let val: u8 = env::var("ALIGN")
            .unwrap_or_else(|_| "1".into()) // default to "1"
            .parse()
            .unwrap_or(1); // fallback to 1 if parsing fails

        if val == 0 || val > CHILDREN {
            panic!(
                "Invalid ALIGN value: {} (must be between 1 and {})",
                val, CHILDREN
            );
        }

        val
    }

    let http_port = env::var("HTTP_PORT").unwrap_or_else(|_| "3001".into());
    let p2p_port = env::var("P2P_PORT").unwrap_or_else(|_| "5001".into());
    let peers = env::var("PEERS").unwrap_or_else(|_| "".into());
    let miner_address = env::var("MINER_ADDRESS").unwrap_or_else(|_| "Miner 123".to_string());
    let align = get_align();

    println!("Starting HTTP server on http://localhost:{}", http_port);
    println!("Starting P2P server on http://localhost:{}", p2p_port);

    let treechain = Arc::new(Mutex::new(TreeChain::new()));
    let pqp = Arc::new(Mutex::new(PQP::new()));
    let mining_flag = Arc::new(Mutex::new(false));
    let current_mining_position = Arc::new(SyncMutex::new(None::<String>));
    let abort_mining = Arc::new(SyncMutex::new(false));

    let p2p_port_clone = p2p_port.clone();
    let peers_clone = peers.clone();

    let p2p_server = start_p2p_server(
        p2p_port_clone,
        peers_clone,
        Arc::clone(&treechain),
        Arc::clone(&pqp),
        Arc::clone(&current_mining_position),
        Arc::clone(&abort_mining),
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new((
                Arc::clone(&treechain),
                Arc::clone(&pqp),
                Arc::clone(&mining_flag),
                Arc::clone(&p2p_server),
                Arc::clone(&current_mining_position),
                Arc::clone(&abort_mining),
                miner_address.clone(),
                align,
            )))
            .configure(miner::routes::init_routes)
    })
    .bind(format!("127.0.0.1:{}", http_port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
