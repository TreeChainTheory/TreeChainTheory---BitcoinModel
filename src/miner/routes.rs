use crate::p2p_server::P2PServer;
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, TreeChain};
use actix_web::{HttpResponse, Responder, get, web};
use std::sync::{Arc, Mutex as SyncMutex};
use tokio::sync::Mutex;

#[get("/start_mining")]
async fn start_mining(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<u32>>>,
        Arc<SyncMutex<bool>>,
        String,
    )>,
) -> impl Responder {
    let (
        treechain,
        pqp,
        mining_flag,
        p2p_server,
        current_mining_target,
        abort_mining,
        miner_address,
    ) = {
        let d = data.as_ref();
        (
            Arc::clone(&d.0),
            Arc::clone(&d.1),
            Arc::clone(&d.2),
            Arc::clone(&d.3),
            Arc::clone(&d.4),
            Arc::clone(&d.5),
            d.6.clone(),
        )
    };

    // Lock the mining flag once to check if already running
    {
        let mut mining = mining_flag.lock().await;
        if *mining {
            return HttpResponse::Ok().json(serde_json::json!({
                "status": "error",
                "message": "Mining is already in progress"
            }));
        }
        *mining = true;
    }

    // Clone Arcs for background task
    let treechain = Arc::clone(&treechain);
    let pqp = Arc::clone(&pqp);
    let mining_flag = Arc::clone(&mining_flag);

    // Static values for mining parameters
    let align = 1;
    let tx = vec!["tx1".to_string(), "tx2".to_string()];
    let signature = "DUMMY_SIGNATURE_64_BYTES".to_string().repeat(3);

    tokio::spawn(async move {
        loop {
            // Check if we should continue mining
            if !*mining_flag.lock().await {
                println!("Mining stopped");
                break;
            }

            *abort_mining.lock().unwrap() = false;

            let mut chain = treechain.lock().await;
            let mut pqp_guard = pqp.lock().await;

            let calc = chain.calculate_queue_index(&mut pqp_guard, align);

            if let Some((queue_index, calculated_align, prev_pqp)) = calc {
                *current_mining_target.lock().unwrap() = Some(queue_index);
                match chain.mine_block_demo_2(
                    &mut pqp_guard,
                    align,
                    tx.clone(),
                    miner_address.clone(),
                    signature.clone(),
                    &abort_mining,
                ) {
                    Some(block) => {
                        println!("Successfully mined block: {}", block.hash);
                        let writers = p2p_server.writers.lock().await;
                        for writer in writers.iter() {
                            p2p_server
                                .clone()
                                .send_minedblock(writer.clone(), block.clone())
                                .await;
                        }
                    }
                    None => {
                        println!("Failed to mine block, retrying...");
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            } else {
                println!("Failed to calculate queue index, retrying...");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            // match chain.mine_block_demo(
            //     &mut pqp_guard,
            //     align,
            //     tx.clone(),
            //     miner_address.clone(),
            //     signature.clone(),
            // ) {
            //     Some(block) => {
            //         println!("Successfully mined block: {}", block.hash);
            //         let writers = p2p_server.writers.lock().await;
            //         for writer in writers.iter() {
            //             p2p_server
            //                 .clone()
            //                 .send_minedblock(writer.clone(), block.clone())
            //                 .await;
            //         }
            //     }
            //     None => {
            //         println!("Failed to mine block, retrying...");
            //         tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            //     }
            // }
            *current_mining_target.lock().unwrap() = None;
        }
    });

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": "Mining started"
    }))
}

#[get("/stop_mining")]
async fn stop_mining(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<u32>>>,
        Arc<SyncMutex<bool>>,
        String,
    )>,
) -> impl Responder {
    let (_, _, mining_flag, _, _, _, _) = data.as_ref();
    let mut mining = mining_flag.lock().await;

    if !*mining {
        return HttpResponse::Ok().json(serde_json::json!({
            "status": "error",
            "message": "Mining is not in progress"
        }));
    }

    *mining = false;
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": "Mining stopped"
    }))
}

#[get("/get_blocks")]
async fn get_blocks(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<u32>>>,
        Arc<SyncMutex<bool>>,
        String,
    )>,
) -> impl Responder {
    let (treechain, _, _, _, _, _, _) = data.as_ref();
    let chain = treechain.lock().await;

    let blocks: Vec<Block> = chain
        .blocks
        .values()
        .filter(|block| !block.position.is_empty()) // Exclude placeholder blocks
        .cloned()
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "blocks": blocks
    }))
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(start_mining);
    cfg.service(stop_mining);
    cfg.service(get_blocks);
}
