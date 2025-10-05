pub mod chain_util;
pub mod config;
mod miner;
pub mod treechain;
pub mod wallet;

use crate::wallet::transaction_pool;
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::UtxoSet;
use config::CHILDREN;
use treechain::treechain::{PQP, TreeChain};
use wallet::wallet::Wallet;

use actix_web::{App, HttpServer, web};
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use tokio::sync::Mutex;

use crate::miner::p2p_server::{self, P2PServer};

#[actix_web::main]
async fn main() {
    dotenv().ok();

    fn get_align() -> u8 {
        let val: u8 = env::var("ALIGN")
            .unwrap_or_else(|_| "1".into())
            .parse()
            .unwrap_or(1);

        if val == 0 || val > CHILDREN {
            panic!(
                "Invalid ALIGN value: {} (must be between 1 and {})",
                val, CHILDREN
            );
        }

        val
    }

    let wallet = Arc::new(Mutex::new(Wallet::new()));

    let http_port = env::var("HTTP_PORT").unwrap_or_else(|_| "3001".into());
    let p2p_port = env::var("P2P_PORT").unwrap_or_else(|_| "5001".into());
    let peers = env::var("PEERS").unwrap_or_else(|_| "".into());
    let miner_address = wallet.lock().await.clone().address;
    let align = get_align();

    println!("Starting HTTP server on http://localhost:{}", http_port);
    println!("Starting P2P server on http://localhost:{}", p2p_port);

    let treechain = Arc::new(Mutex::new(TreeChain::new()));
    let pqp = Arc::new(Mutex::new(PQP::new()));
    let utxo_set = Arc::new(Mutex::new(UtxoSet::new()));
    let transaction_pool = Arc::new(Mutex::new(TransactionPool::new()));
    let mining_flag = Arc::new(Mutex::new(false));
    let current_mining_position = Arc::new(SyncMutex::new(None::<String>));
    let abort_mining = Arc::new(SyncMutex::new(false));
    let chain_length = Arc::new(SyncMutex::new(1 as u64));

    let p2p_server = P2PServer::start_p2p_server(
        p2p_port.clone(),
        peers.clone(),
        miner_address.clone(),
        Arc::clone(&treechain),
        Arc::clone(&pqp),
        Arc::clone(&utxo_set),
        Arc::clone(&transaction_pool),
        Arc::clone(&current_mining_position),
        Arc::clone(&abort_mining),
        Arc::clone(&chain_length),
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
                Arc::clone(&wallet),
                align,
                Arc::clone(&chain_length),
                Arc::clone(&utxo_set),
                Arc::clone(&transaction_pool),
            )))
            .configure(miner::routes::init_routes)
    })
    .bind(format!("0.0.0.0:{}", http_port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
