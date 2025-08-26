mod miner;
mod treechain;
use treechain::block::Block;
use treechain::treechain::{PQP, TreeChain};
mod config;

use actix_web::{App, HttpServer, web};
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;

#[actix_web::main]
async fn main() {
    // let mut genisis_block = Block::genesis();
    // Block::calculate_hash_and_pqp_commitment(&mut genisis_block);
    // println!("Genisis Block: {:#?}", genisis_block);
    // println!("Genisis Block Hash: {}", genisis_block.hash);

    dotenv().ok();

    let http_port = env::var("HTTP_PORT").unwrap_or_else(|_| "3001".into());
    let p2p_port = env::var("P2P_PORT").unwrap_or_else(|_| "5001".into());
    let peers = env::var("PEERS").unwrap_or_else(|_| "".into());

    println!("Starting HTTP server on http://localhost:{}", http_port);
    println!("Starting P2P server on http://localhost:{}", p2p_port);

    let treechain = Arc::new(Mutex::new(TreeChain::new()));
    let pqp = Arc::new(Mutex::new(PQP::new()));
    let mining_flag = Arc::new(Mutex::new(false));

    let p2p_port_clone = p2p_port.clone();
    let peers_clone = peers.clone();

    let treechain_clone = Arc::clone(&treechain);
    let pqp_clone = Arc::clone(&pqp);

    thread::spawn(move || {
        miner::p2p_server::start_p2p_server(
            p2p_port_clone,
            peers_clone,
            treechain_clone,
            pqp_clone,
        );
    });

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new((
                Arc::clone(&treechain),
                Arc::clone(&pqp),
                Arc::clone(&mining_flag),
            )))
            .configure(miner::routes::init_routes)
    })
    .bind(format!("127.0.0.1:{}", http_port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
