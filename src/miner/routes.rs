use crate::chain_util::ChainUtil;
use crate::config::SIGHASH_ALL;
use crate::miner::p2p_server;
use crate::p2p_server::P2PServer;
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::UtxoSet;
use crate::wallet::wallet::Wallet;
use actix_web::{HttpResponse, Responder, get, post, web};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex as SyncMutex};
use tokio::sync::Mutex;

#[get("/start_mining")]
async fn start_mining(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (
        treechain,
        pqp,
        mining_flag,
        p2p_server,
        current_mining_position,
        abort_mining,
        wallet,
        align,
        chain_length,
        utxo_set,
        txn_pool,
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
            d.7,
            Arc::clone(&d.8),
            Arc::clone(&d.9),
            Arc::clone(&d.10),
        )
    };

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

    let treechain = Arc::clone(&treechain);
    let pqp = Arc::clone(&pqp);
    let mining_flag = Arc::clone(&mining_flag);

    // //this should be updated to txn_pool.get_txns_by_align
    // let tx1 = Transaction::create_sample_transaction_with_different_output();
    // let tx2 = Transaction::create_sample_transaction();

    let tag = "WOW";

    tokio::spawn(async move {
        loop {
            if !*mining_flag.lock().await {
                println!("Mining stopped");
                break;
            }

            let tx: Vec<Transaction>;
            {
                let txn_pool = txn_pool.lock().await;
                tx = txn_pool.select_transactions(1024 * 10, align.clone());
                println!("🤔🤔 selected txns: {:?}", tx);
                drop(txn_pool);
            }

            *abort_mining.lock().unwrap() = false;
            let mut parent_pos: String;
            let calc;
            {
                let mut tree = treechain.lock().await;
                let mut pqp_guard = pqp.lock().await;
                let parent_pos_entry = pqp_guard.current_parent();
                let parent_pos_block =
                    tree.get_block(&parent_pos_entry.unwrap().block_hash.clone());
                parent_pos = if let Some(b) = parent_pos_block {
                    b.position.clone()
                } else {
                    "".to_string()
                };

                calc = tree.calculate_qi(&mut pqp_guard, align);
                if calc.is_none() {
                    println!("Failed to calculate queue index it returned None");
                }
            }

            if let Some((queue_index, prev_pqp, parent_hash)) = calc {
                *current_mining_position.lock().unwrap() = Some(parent_pos.clone());

                let miner_address = wallet.address.clone();
                let mut sig_data = Vec::new();
                sig_data.extend_from_slice(&queue_index.to_le_bytes());
                sig_data.extend(hex::decode(&parent_hash).unwrap_or_default());
                sig_data.extend(hex::decode(&miner_address).unwrap_or_default());
                sig_data.extend(hex::decode(&prev_pqp).unwrap_or_default());

                // Sign the data
                let signature = wallet.sign_data(&sig_data);

                // CONCATENATE signature with wallet.public_key
                let combined_signature = format!("{}{}", signature, wallet.public_key);

                let block_template_opt;
                {
                    let tree = treechain.lock().await;
                    let mut pqp_guard = pqp.lock().await;
                    let txn_pool = txn_pool.lock().await;
                    let utxo_set = utxo_set.lock().await;
                    block_template_opt = tree.prepare_block_template(
                        &mut pqp_guard,
                        align,
                        tx.clone(),
                        wallet.address.clone(),
                        combined_signature.clone(),
                        tag.to_string(),
                        &utxo_set,
                        &txn_pool,
                    );
                }

                if let Some(mut block_template) = block_template_opt {
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
                    *current_mining_position.lock().unwrap() = None;

                    if let Some(mined_block) = mined_result {
                        let mut tree = treechain.lock().await;
                        let mut pqp_guard = pqp.lock().await;
                        let mut utxo_set = utxo_set.lock().await;
                        let mut txn_pool = txn_pool.lock().await;

                        let pqp_entry = TreeChain::parent_queue_entry_from_block(&mined_block);
                        pqp_guard.add_entry_to_pqp(pqp_entry.clone(), &tree);

                        let exist: bool = pqp_guard
                            .pool
                            .iter()
                            .rev()
                            .take(crate::config::CHILDREN as usize)
                            .any(|e| e.block_hash == pqp_entry.block_hash);

                        if exist {
                            if tree.verify_and_add_block_to_tree(
                                &mined_block,
                                &mut utxo_set,
                                &mut txn_pool,
                            ) {
                                println!("Successfully mined block: {}", mined_block.hash);
                                drop(tree);
                                drop(pqp_guard);
                                drop(utxo_set);
                                drop(txn_pool);
                                p2p_server
                                    .clone()
                                    .send_minedblock(mined_block.clone())
                                    .await;
                                {
                                    let mut chain_length = chain_length.lock().unwrap();
                                    *chain_length += 1;
                                    drop(chain_length);
                                }
                                p2p_server.clone().update_registry_chain_length().await;
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

#[get("/stop_mining")]
async fn stop_mining(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (_, _, mining_flag, _, _, _, _, _, _, _, _) = data.as_ref();
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
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (treechain, _, _, _, _, _, _, _, _, _, _) = data.as_ref();
    let chain = treechain.lock().await;

    let blocks: Vec<Block> = chain
        .blocks
        .values()
        .filter(|block| !block.position.is_empty())
        .cloned()
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "blocks": blocks
    }))
}

#[get("/get_pqp")]
async fn get_pqp(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (_, pqp, _, _, _, _, _, _, _, _, _) = data.as_ref();
    let pqp = pqp.lock().await;
    let pool: Vec<ParentQueueEntry> = pqp.pool.clone();
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "blocks": pool,
    }))
}

#[get("/verify_tree")]
async fn verify_tree(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (treechain, pqp, _, _, _, _, _, _, _, _, _) = data.as_ref();
    let chain = treechain.lock().await;
    let pqp_guard = pqp.lock().await;

    let is_valid = chain.is_valid_tree(&pqp_guard);

    HttpResponse::Ok().json(serde_json::json!({
        "status": if is_valid { "success" } else { "error" },
        "message": if is_valid { "Tree is valid" } else { "Tree is invalid" }
    }))
}

#[get("/verify_pqp")]
async fn verify_pqp(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (treechain, pqp, _, _, _, _, _, _, _, _, _) = data.as_ref();
    let chain = treechain.lock().await;
    let pqp_guard = pqp.lock().await;

    let is_valid = chain.is_valid_pqp(&pqp_guard);

    HttpResponse::Ok().json(serde_json::json!({
        "status": if is_valid { "success" } else { "error" },
        "message": if is_valid { "PQP is valid" } else { "PQP is invalid" }
    }))
}

#[derive(Deserialize, Debug)]
struct CreateSimpleTxnRequest {
    /// Value to send in satoshis
    value: u64,

    /// Recipient address
    to_address: String,

    /// Transaction fee in satoshis (optional, defaults to 1000)
    #[serde(default)]
    fee: Option<u64>,
}

#[post("/create_txn")]
async fn create_txn(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
    payload: web::Json<CreateSimpleTxnRequest>,
) -> impl Responder {
    let (_, _, _, p2p_server, _, _, wallet, _, _, utxo_set, txn_pool) = data.as_ref();

    // Validate required fields
    if payload.value == 0 {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "success": false,
                "error": "Missing 'value' field (satoshis to send)"
            }));
    }

    if payload.to_address.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "success": false,
                "error": "Missing 'to_address' field (recipient address)"
            }));
    }

    let fee = payload.fee.unwrap_or(1000); //will be changed

    // Validate recipient address
    if ChainUtil::pubkey_hash_from_address(&payload.to_address).is_err() {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "success": false,
                "error": format!("Invalid recipient address: {}", payload.to_address)
            }));
    }

    // Lock UTXO set for reading
    let utxo_set_guard = utxo_set.lock().await;

    // Check wallet balance first
    let wallet_balance = Wallet::get_balance(&wallet.address, &utxo_set_guard);
    if wallet_balance < payload.value + fee {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "success": false,
                "error": format!(
                    "Insufficient funds: need {}, available: {} satoshis",
                    payload.value + fee, wallet_balance
                )
            }));
    }

    // Create the transaction
    match Transaction::create_new_transaction(
        &wallet.clone(),
        &utxo_set_guard,
        payload.value,
        fee,
        &payload.to_address,
    ) {
        Ok(mut tx) => {
            let input_data: Vec<(String, u32, String, u64)> = tx
                .vin
                .iter()
                .map(|input| {
                    let utxo = utxo_set_guard
                        .get_utxo(&input.txid, input.vout)
                        .expect("UTXO should exist as it was just selected");
                    (
                        input.txid.clone(),
                        input.vout,
                        utxo.out.script_pubkey.clone(),
                        utxo.out.value,
                    )
                })
                .collect();

            // Now sign using the collected data
            tx.clone()
                .sign_transaction(&wallet, &mut tx, &utxo_set_guard);

            // Lock transaction pool for adding
            let mut txn_pool_guard = txn_pool.lock().await;

            // Add transaction to mempool
            match txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard) {
                Ok(()) => {
                    println!(
                        "✅ Created and added normal txn to pool: {} ({} satoshis to {})",
                        tx.txid, payload.value, payload.to_address
                    );

                    // Return transaction details as JSON
                    let response = serde_json::json!({
                        "success": true,
                        "txid": tx.txid,
                        "from_address": wallet.clone().address,
                        "to_address": payload.to_address,
                        "value": payload.value,
                        "fee": fee,
                        "total_input": payload.value + fee,
                        "wallet_balance": wallet_balance,
                        "transaction": tx
                    });
                    println!("sening broadcast txn");
                    p2p_server.clone().broadcast_transaction(tx.clone()).await;

                    HttpResponse::Ok()
                        .content_type("application/json")
                        .json(response)
                }
                Err(e) => {
                    println!("❌ Failed to add transaction to pool: {}", e);
                    HttpResponse::BadRequest()
                        .content_type("application/json")
                        .json(serde_json::json!({
                            "success": false,
                            "error": format!("Failed to add to mempool: {}", e),
                            "transaction": tx
                        }))
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create transaction: {}", e);
            HttpResponse::BadRequest()
                .content_type("application/json")
                .json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to create transaction: {}", e)
                }))
        }
    }
}

#[get("/transaction_pool")]
async fn transaction_pool(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (_, _, _, _, _, _, _, _, _, _, txn_pool) = data.as_ref();

    // Lock transaction pool for reading
    let txn_pool_guard = txn_pool.lock().await;

    let pool_size = txn_pool_guard.len();
    let total_size_bytes = txn_pool_guard.total_size;

    if pool_size == 0 {
        let empty_response = serde_json::json!({
            "success": true,
            "pool_size": 0,
            "total_size_bytes": 0,
            "total_size_mb": 0.0,
            "transactions": [],
            "summary": {
                "empty": true,
                "message": "Transaction pool is empty"
            }
        });

        return HttpResponse::Ok()
            .content_type("application/json")
            .json(empty_response);
    }

    // Collect all mempool entries
    let transactions: Vec<serde_json::Value> = txn_pool_guard
        .pool
        .values()
        .map(|entry| {
            serde_json::json!({
                "txid": entry.tx.txid,
                "hash": entry.tx.hash,
                "version": entry.tx.version,
                "locktime": entry.tx.locktime,
                "input_count": entry.tx.vin.len(),
                "output_count": entry.tx.vout.len(),
                "total_input_value": {
                    // Calculate total input value (simplified - you'd need UTXO set for accurate values)
                    "satoshis": entry.fee + entry.tx.vout.iter().map(|out| out.value).sum::<u64>()
                },
                "total_output_value": {
                    "satoshis": entry.tx.vout.iter().map(|out| out.value).sum::<u64>()
                },
                "fee": {
                    "satoshis": entry.fee,
                    "sat_per_vbyte": entry.fee_rate
                },
                "size": {
                    "vbytes": entry.vsize,
                    "bytes": entry.vsize * 4 / 3 // Approximate actual bytes
                },
                "weight": {
                    "weight_units": entry.tx.get_size_vsize_weight().2
                },
                "fee_rate": {
                    "sat_per_vbyte": entry.fee_rate
                },
                "added_time": {
                    "timestamp_ms": entry.added_time,
                    "iso": chrono::DateTime::from_timestamp(entry.added_time as i64 / 1000, 
                        std::time::Duration::from_millis((entry.added_time % 1000) as u64).subsec_nanos())
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| "invalid timestamp".to_string())
                },
                "age_seconds": {
                    "value": (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64 - entry.added_time) / 1000
                },
                "dependencies": {
                    "parent_count": entry.depends.len(),
                    "parents": entry.depends.iter().collect::<Vec<_>>(),
                    "child_count": entry.children.len(),
                    "children": entry.children.iter().collect::<Vec<_>>()
                },
                "has_witness": entry.tx.witnesses.is_some(),
                // Only include full transaction details if requested (to keep response size manageable)
                "full_transaction": null // Set to true in query params to include full tx
            })
        })
        .collect();

    // Calculate pool statistics
    let total_fees: u64 = txn_pool_guard.pool.values().map(|entry| entry.fee).sum();

    let avg_fee_rate: f64 = if pool_size > 0 {
        txn_pool_guard
            .pool
            .values()
            .map(|entry| entry.fee_rate)
            .sum::<f64>()
            / pool_size as f64
    } else {
        0.0
    };

    let total_value: u64 = txn_pool_guard
        .pool
        .values()
        .map(|entry| entry.fee + entry.tx.vout.iter().map(|out| out.value).sum::<u64>())
        .sum();

    let response = serde_json::json!({
        "success": true,
        "pool_size": pool_size,
        "total_size_bytes": total_size_bytes,
        "total_size_mb": (total_size_bytes as f64) / (1024.0 * 1024.0),
        "total_fees_satoshis": total_fees,
        "total_value_satoshis": total_value,
        "average_fee_rate_sat_per_vb": avg_fee_rate,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "transactions": transactions,
        "summary": {
            "pool_stats": {
                "transaction_count": pool_size,
                "total_size_mb": format!("{:.2}", (total_size_bytes as f64) / (1024.0 * 1024.0)),
                "total_fees_btc": format!("{:.8}", total_fees as f64 / 100_000_000.0),
                "avg_fee_rate_sat_per_vb": format!("{:.2}", avg_fee_rate)
            }
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}

#[get("/wallet")]
async fn wallet_details(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (_, _, _, _, _, _, wallet, _, _, utxo_set, _) = data.as_ref();

    // Lock UTXO set for reading
    let utxo_set_guard = utxo_set.lock().await;

    // Get wallet balance and UTXOs
    let balance = Wallet::get_balance(&wallet.address, &utxo_set_guard);
    let utxos = Wallet::get_utxos_for_address(&wallet.address, &utxo_set_guard);

    // Calculate total UTXO count and value distribution
    let utxo_count = utxos.len();
    let utxo_details: Vec<serde_json::Value> = utxos
        .iter()
        .map(|(txid, vout, utxo)| {
            serde_json::json!({
                "txid": txid.clone(),
                "vout": *vout,
                "value_satoshis": utxo.out.value,
                "value_btc": format!("{:.8}", utxo.out.value as f64 / 100_000_000.0),
                "script_pubkey": utxo.out.script_pubkey.clone(),
                "is_coinbase": utxo.f_coinbase,
                "confirmations": 1, // Simplified - you'd calculate based on block depth
                "block_queue_index": utxo.queue_index
            })
        })
        .collect();

    // Get private key as hex (WARNING: This is sensitive information!)
    let private_key_bytes = wallet.key_pair.to_bytes();
    let private_key_hex = hex::encode(&private_key_bytes);

    // Get public key details
    let public_key_bytes = hex::decode(&wallet.public_key).unwrap_or_default();
    let public_key_compressed = public_key_bytes.len() == 33;
    let public_key_uncompressed = public_key_bytes.len() == 65;

    // Verify address derivation (for security audit)
    let derived_pubkey_hash = ChainUtil::pubkey_hash_from_pubkey(&wallet.public_key);
    let address_pubkey_hash =
        ChainUtil::pubkey_hash_from_address(&wallet.address).unwrap_or_default();

    let address_verification = if derived_pubkey_hash == address_pubkey_hash {
        "valid"
    } else {
        "invalid - address doesn't match public key!"
    };

    // Calculate network stats (simplified)
    let total_sent_estimate = 0u64; // You'd track this in a real wallet
    let total_received_estimate = balance; // Simplified

    let response = serde_json::json!({
        "success": true,
        "wallet": {
            "address": wallet.address.clone(),
            "public_key": {
                "hex": wallet.public_key.clone(),
                "length_bytes": public_key_bytes.len(),
                "format": if public_key_compressed { "compressed" } else if public_key_uncompressed { "uncompressed" } else { "unknown" },
                "bytes": public_key_bytes
            },
            "public_key_hash": wallet.public_key_hash.clone(),
            "private_key": {
                "hex": private_key_hex,
                "length_bytes": private_key_bytes.len(),
                "warning": "⚠️  This is sensitive information! Never share your private key.",
                "bytes": private_key_bytes.as_slice().to_vec()
            },
            "network": {
                "mainnet": true, // Assuming mainnet for this implementation
                "testnet": false
            }
        },
        "balance": {
            "satoshis": balance,
            "btc": format!("{:.8}", balance as f64 / 100_000_000.0),
            "confirmed": balance,
            "unconfirmed": 0
        },
        "utxos": {
            "count": utxo_count,
            "total_value_satoshis": utxos.iter().map(|(_, _, u)| u.out.value).sum::<u64>(),
            "total_value_btc": format!("{:.8}", utxos.iter().map(|(_, _, u)| u.out.value).sum::<u64>() as f64 / 100_000_000.0),
            "details": utxo_details,
            "largest_utxo": utxos.iter().map(|(_, _, u)| u.out.value).max().unwrap_or(0),
            "smallest_utxo": utxos.iter().map(|(_, _, u)| u.out.value).min().unwrap_or(0),
            "dust_utxos": utxos.iter().filter(|(_, _, u)| u.out.value < 546).count() // Dust threshold
        },
        "transaction_stats": {
            "total_received_satoshis": total_received_estimate,
            "total_received_btc": format!("{:.8}", total_received_estimate as f64 / 100_000_000.0),
            "total_sent_satoshis": total_sent_estimate,
            "total_sent_btc": format!("{:.8}", total_sent_estimate as f64 / 100_000_000.0),
            "net_balance_satoshis": total_received_estimate.saturating_sub(total_sent_estimate)
        },
        "security": {
            "address_verification": address_verification,
            "derived_pubkey_hash": derived_pubkey_hash,
            "expected_pubkey_hash": address_pubkey_hash,
            "wallet_format": "hd" // or "simple" - you'd extend this for HD wallets
        },
        "metadata": {
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "node_version": "0.1.0", // You'd get this from your app config
            "utxo_set_height": utxo_set_guard.utxos.len()
        },
        "warnings": if address_verification != "valid" {
            vec!["Address verification failed - check wallet integrity!"]
        } else {
            vec![]
        }
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}

#[get("/utxo_set")]
async fn utxo_set_details(
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let utxo_set = data.9.lock().await;

    // Convert UTXO set to a JSON-serializable format
    let utxos: Vec<serde_json::Value> = utxo_set
        .utxos
        .iter()
        .map(|((txid, vout), utxo)| {
            serde_json::json!({
                "txid": txid,
                "vout": vout,
                "value": utxo.out.value,
                "script_pubkey": utxo.out.script_pubkey,
                "queue_index": utxo.queue_index,
                "is_coinbase": utxo.f_coinbase
            })
        })
        .collect();

    let response = serde_json::json!({
        "success": true,
        "utxo_count": utxos.len(),
        "utxos": utxos,
        "total_value": utxo_set.utxos.values().map(|u| u.out.value).sum::<u64>(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}

//multisig txn creation and handling

#[derive(Deserialize)]
struct MultisigBody {
    pubkeys: Vec<String>,
    m: u8,
    value: u64,
    fee: u64,
}
#[post("/create_multisig_txn")]
async fn create_multisig_txn(
    body: web::Json<MultisigBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let p2p_server = data.3.clone();
    let wallet = data.6.clone();
    let utxo_set = data.9.clone();

    let pubkeys = body.pubkeys.clone();
    let m = body.m;
    let value = body.value;
    let fee = body.fee;

    if m as usize > pubkeys.clone().len() || m == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Invalid m or pubkeys"
        }));
    }

    let utxo_set_guard = utxo_set.lock().await;
    let mut txn =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set_guard, m, pubkeys, value, fee)
            .expect("Failed to create multisig transaction");
    txn = txn
        .clone()
        .sign_transaction(&wallet, &mut txn, &utxo_set_guard);
    p2p_server.broadcast_transaction(txn.clone()).await;

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "transaction": txn,
    }))
}

#[derive(Deserialize)]
struct CreateSpendingMultisigBody {
    txid: String,
    vout: u32,
    pubkeys: Vec<String>,
    m: u8,
    to_address: String,
    value: u64,
    fee: u64,
    change_address: Option<String>,
}

#[post("/create_spending_multisig_tx")]
async fn create_spending_multisig_tx(
    body: web::Json<CreateSpendingMultisigBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let utxo_set = data.9.clone();

    let txid = body.txid.clone();
    let vout = body.vout;
    let pubkeys = body.pubkeys.clone();
    let m = body.m;
    let to_address = body.to_address.clone();
    let value = body.value;
    let fee = body.fee;
    let change_address = body.change_address.clone();

    if m as usize > pubkeys.len() || m == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Invalid m or pubkeys"
        }));
    }

    let utxo_set_guard = utxo_set.lock().await;
    let utxo = match utxo_set_guard.get_utxo(&txid, vout) {
        Some(u) => u,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": "UTXO not found"
            }));
        }
    };
    let input_value = utxo.out.value;

    if input_value < value + fee {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Insufficient funds"
        }));
    }

    let change = input_value - value - fee;

    let redeem = Transaction::create_multisig_redeem_script(m, &pubkeys);
    let expected_script = Transaction::create_p2sh_script(&redeem);
    if utxo.out.script_pubkey != expected_script {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Script mismatch"
        }));
    }

    let mut vout_vec = vec![TxOutput {
        value,
        script_pubkey: Transaction::create_p2pkh_script(
            &ChainUtil::pubkey_hash_from_address(&to_address).unwrap(),
        ),
    }];

    if change > 0 {
        if let Some(ch_addr) = change_address {
            vout_vec.push(TxOutput {
                value: change,
                script_pubkey: Transaction::create_p2pkh_script(
                    &ChainUtil::pubkey_hash_from_address(&ch_addr).unwrap(),
                ),
            });
        } else {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": "Change address required for remainder"
            }));
        }
    }

    let vin = vec![TxInput {
        txid,
        vout,
        script_sig: String::new(),
        sequence: 0xffffffff,
    }];

    let mut txn = Transaction::new(1, 0, vin, vout_vec, None);
    txn.txid = txn.compute_non_witness_txid();
    txn.hash = txn.compute_hash();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "unsigned_transaction": txn,
    }))
}

#[derive(Deserialize)]
struct SpendMultisigBody {
    spending_tx: Transaction,
    pubkeys: Vec<String>,
    m: u8,
    sigs: Vec<Option<String>>,
}

#[post("/spend_multisig_txn")]
async fn spend_multisig_txn(
    body: web::Json<SpendMultisigBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let p2p_server = data.3.clone();
    let wallet = data.6.clone();
    let utxo_set = data.9.clone();
    let txn_pool = data.10.clone();

    let mut spending_tx = body.spending_tx.clone();
    let pubkeys = body.pubkeys.clone();
    let m = body.m;
    let mut sigs = body.sigs.clone();

    if m as usize > pubkeys.len() || m == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Invalid m or pubkeys"
        }));
    }
    if sigs.len() < m as usize {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Signatures length is less than m"
        }));
    }

    // Assume single input for simplicity
    if spending_tx.vin.len() != 1 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Only single-input transactions supported"
        }));
    }

    let input = &spending_tx.vin[0];
    let txid = input.txid.clone();
    let vout = input.vout;

    let utxo_set_guard = utxo_set.lock().await;
    let utxo = match utxo_set_guard.get_utxo(&txid, vout) {
        Some(u) => u,
        None => {
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .json(serde_json::json!({
                    "success": false,
                    "error": format!("UTXO not found: {}:{}", txid, vout)
                }));
        }
    };
    let input_value = utxo.out.value;

    let redeem = Transaction::create_multisig_redeem_script(m, &pubkeys);
    let expected_script = Transaction::create_p2sh_script(&redeem);
    if utxo.out.script_pubkey != expected_script {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "status": "error",
                "message": "Script mismatch"
            }));
    }

    let total_output: u64 = spending_tx.vout.iter().map(|out| out.value).sum();
    let fee = input_value.saturating_sub(total_output);
    if input_value < total_output {
        return HttpResponse::BadRequest()
            .content_type("application/json")
            .json(serde_json::json!({
                "success": false,
                "error": format!(
                    "Insufficient funds: input {} satoshis, outputs {} satoshis",
                    input_value, total_output
                )
            }));
    }

    // Find if current wallet can sign
    if let Some(my_index) = pubkeys.iter().position(|p| p == &wallet.public_key) {
        if sigs[my_index].is_none() {
            let sighash = spending_tx.compute_sighash(0, &redeem, input_value, SIGHASH_ALL);
            let sig = wallet.sign_data(&sighash);
            sigs[my_index] = Some(sig);
        }
    }

    let num_sigs = sigs.iter().filter(|s| s.is_some()).count();

    if num_sigs >= m as usize {
        // Build script_sig
        let mut script_sig_vec: Vec<u8> = vec![0]; // OP_0

        for s in &sigs {
            if let Some(sig_str) = s {
                let s_b = hex::decode(sig_str).unwrap();
                script_sig_vec.push(s_b.len() as u8);
                script_sig_vec.extend_from_slice(&s_b);
            }
        }

        let redeem_b = hex::decode(&redeem).unwrap();
        script_sig_vec.push(redeem_b.len() as u8);
        script_sig_vec.extend_from_slice(&redeem_b);

        spending_tx.vin[0].script_sig = hex::encode(script_sig_vec);
        spending_tx.txid = spending_tx.compute_non_witness_txid();
        spending_tx.hash = spending_tx.compute_hash();

        // Lock transaction pool for adding
        let mut txn_pool_guard = txn_pool.lock().await;

        match txn_pool_guard.add_transaction(spending_tx.clone(), &utxo_set_guard) {
            Ok(()) => {
                println!(
                    "✅ Added multisig spending txn to pool: {} (input {}:{})",
                    spending_tx.txid, txid, vout
                );

                // Broadcast the transaction
                p2p_server.broadcast_transaction(spending_tx.clone()).await;

                // Return transaction details as JSON
                let response = serde_json::json!({
                    "success": true,
                    "txid": spending_tx.txid,
                    "from_multisig": format!("{}:{}", txid, vout),
                    "total_output": total_output,
                    "fee": fee,
                    "total_input": input_value,
                    "num_signatures": num_sigs,
                    "required_signatures": m,
                    "transaction": spending_tx
                });

                HttpResponse::Ok()
                    .content_type("application/json")
                    .json(response)
            }
            Err(e) => {
                println!(
                    "❌ Failed to add multisig spending transaction to pool: {}",
                    e
                );
                HttpResponse::BadRequest()
                    .content_type("application/json")
                    .json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to add to mempool: {}", e),
                        "transaction": spending_tx
                    }))
            }
        }
    } else {
        let response = serde_json::json!({
            "success": false,
            "error": "Insufficient signatures",
            "updated_sigs": sigs,
            "num_signatures": num_sigs,
            "required_signatures": m,
            "transaction": spending_tx
        });
        HttpResponse::Ok()
            .content_type("application/json")
            .json(response)
    }
}
#[derive(Deserialize)]
struct SignMultisigBody {
    txid: String,
    vout: u32,
    spending_tx: Transaction,
    pubkeys: Vec<String>,
    m: u8,
}

#[post("/sign_multisig")]
async fn sign_multisig(
    body: web::Json<SignMultisigBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let wallet = data.6.clone();
    let utxo_set = data.9.lock().await;

    let txid = body.txid.clone();
    let vout = body.vout;
    let spending_tx = body.spending_tx.clone();
    let pubkeys = body.pubkeys.clone();
    let m = body.m;

    if m as usize > pubkeys.len() || m == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Invalid m or pubkeys"
        }));
    }

    match wallet.sign_multisig(&utxo_set, &txid, vout, &spending_tx, &pubkeys, m) {
        Ok(signature) => HttpResponse::Ok().json(serde_json::json!({
            "status": "success",
            "signature": signature,
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": e,
        })),
    }
}

//cration and handling the timelocked txn
#[derive(Deserialize)]
struct CreateTimelockedTxnBody {
    txn_type: String, // "cltv" or "csv"
    lock: u32,
    value: u64,
    fee: u64,
    to_address: String,
}

#[post("/create_timelocked_txn")]
async fn create_timelocked_txn(
    body: web::Json<CreateTimelockedTxnBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let p2p_server = data.3.clone();
    let wallet = data.6.clone();
    let utxo_set = data.9.clone();
    let txn_pool = data.10.clone();

    let txn_type = body.txn_type.clone();
    let lock = body.lock;
    let value = body.value;
    let fee = body.fee;
    let to_address = body.to_address.clone();

    // Validate required fields
    if value == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Value must be positive"
        }));
    }

    if to_address.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "To address is required"
        }));
    }

    if txn_type != "cltv" && txn_type != "csv" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "Invalid txn_type, must be 'cltv' or 'csv'"
        }));
    }

    let utxo_set_guard = utxo_set.lock().await;

    let mut tx = if txn_type == "cltv" {
        Transaction::create_new_timelocked_cltv_txn(
            &wallet,
            &utxo_set_guard,
            value,
            fee,
            lock,
            &to_address,
        )
    } else {
        Transaction::create_new_timelocked_csv_txn(
            &wallet,
            &utxo_set_guard,
            value,
            fee,
            lock,
            &to_address,
        )
    }
    .expect("Failed to create timelocked transaction");

    tx = tx
        .clone()
        .sign_transaction(&wallet, &mut tx, &utxo_set_guard);

    let mut txn_pool_guard = txn_pool.lock().await;

    let add_result = txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard);

    let status_message = match add_result {
        Ok(_) => {
            p2p_server.broadcast_transaction(tx.clone()).await;
            "Transaction added to mempool and broadcasted"
        }
        Err(e) if e.contains("locktime not yet reached") => {
            "Transaction created but locktime in future - broadcast later when condition met"
        }
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": e
            }));
        }
    };

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": status_message,
        "transaction": tx
    }))
}

#[derive(Deserialize)]
struct BroadcastTxnBody {
    transaction: Transaction,
}

#[post("/broadcast_txn")]
async fn broadcast_txn(
    body: web::Json<BroadcastTxnBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Wallet,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let p2p_server = data.3.clone();
    let utxo_set = data.9.clone();
    let txn_pool = data.10.clone();

    let tx = body.transaction.clone();

    let utxo_set_guard = utxo_set.lock().await;
    let mut txn_pool_guard = txn_pool.lock().await;

    match txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard) {
        Ok(_) => {
            p2p_server.broadcast_transaction(tx.clone()).await;
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Transaction added to mempool and broadcasted",
                "txid": tx.txid
            }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": e
        })),
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(start_mining);
    cfg.service(stop_mining);
    cfg.service(get_blocks);
    cfg.service(get_pqp);
    cfg.service(verify_tree);
    cfg.service(verify_pqp);
    cfg.service(create_txn);
    cfg.service(transaction_pool);
    cfg.service(wallet_details);
    cfg.service(utxo_set_details);
    cfg.service(create_multisig_txn);
    cfg.service(create_spending_multisig_tx);
    cfg.service(sign_multisig);
    cfg.service(spend_multisig_txn);
    cfg.service(create_timelocked_txn);
    cfg.service(broadcast_txn);
}
