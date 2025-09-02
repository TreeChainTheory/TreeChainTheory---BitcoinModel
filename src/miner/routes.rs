use crate::p2p_server::P2PServer;
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, TreeChain};
use actix_web::{HttpResponse, Responder, get, web};
use num_bigint::BigUint;
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

            // Brief lock for queue index calculation
            let calc;
            {
                let mut chain = treechain.lock().await;
                let mut pqp_guard = pqp.lock().await;
                calc = chain.calculate_queue_index(&mut pqp_guard, align);
            } // Release locks

            if let Some((queue_index, calculated_align, prev_pqp)) = calc {
                *current_mining_target.lock().unwrap() = Some(queue_index);

                // Brief lock for template preparation
                let block_template_opt;
                {
                    let chain = treechain.lock().await;
                    let mut pqp_guard = pqp.lock().await;
                    block_template_opt = chain.prepare_block_template(
                        &mut pqp_guard,
                        align,
                        tx.clone(),
                        miner_address.clone(),
                        signature.clone(),
                    );
                } // Release locks

                if let Some(mut block_template) = block_template_opt {
                    // Offload nonce search to blocking thread
                    let abort_clone = Arc::clone(&abort_mining);
                    let mined_result = tokio::task::spawn_blocking(move || {
                        let target = Block::calculate_target(block_template.bits.clone())
                            .unwrap_or_else(|| BigUint::from(0u32));
                        let mut nonce = 0u32;
                        let mut iteration = 0;
                        loop {
                            if iteration % 1000 == 0 && *abort_clone.lock().unwrap() {
                                return None;
                            }

                            block_template.nonce = nonce;
                            Block::calculate_hash_and_pqp_commitment(&mut block_template);

                            let hash_bytes = hex::decode(&block_template.hash).unwrap_or_default();
                            let hash_int = BigUint::from_bytes_be(&hash_bytes);

                            if hash_int < target {
                                return Some(block_template);
                            }

                            nonce = nonce.wrapping_add(1);
                            iteration += 1;
                        }
                    })
                    .await
                    .unwrap();

                    *current_mining_target.lock().unwrap() = None;

                    if let Some(mined_block) = mined_result {
                        // Brief lock to add and broadcast
                        {
                            let mut chain = treechain.lock().await;
                            let mut pqp_guard = pqp.lock().await;

                            let pqp_entry = TreeChain::parent_queue_entry_from_block(&mined_block);
                            pqp_guard.add_entry(pqp_entry.clone());

                            let exist: bool = pqp_guard
                                .pool
                                .iter()
                                .rev()
                                .take(crate::config::CHILDREN as usize)
                                .any(|e| e.block_hash == pqp_entry.block_hash);

                            if exist {
                                if chain.verify_and_add_block(&mined_block) {
                                    println!("Successfully mined block: {}", mined_block.hash);
                                    let writers = p2p_server.writers.lock().await;
                                    for writer in writers.iter() {
                                        p2p_server
                                            .clone()
                                            .send_minedblock(writer.clone(), mined_block.clone())
                                            .await;
                                    }
                                } else {
                                    println!(
                                        "Failed to add block {} (duplicate or invalid)",
                                        mined_block.hash
                                    );
                                    pqp_guard.remove_pqp_entry(pqp_entry);
                                }
                            } else {
                                println!("Failed to add PQP entry for mined block, retrying...");
                            }
                        }
                    } else {
                        println!("Mining aborted, retrying...");
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                } else {
                    println!("Failed to prepare block template, retrying...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            } else {
                println!("Failed to calculate queue index, retrying...");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    });

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": "Mining started"
    }))
}

// The rest of the file (stop_mining, get_blocks, init_routes) remains unchanged.
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
