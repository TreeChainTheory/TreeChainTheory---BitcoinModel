use crate::chain_util::ChainUtil;
use crate::config::SIGHASH_ALL;
use crate::miner::p2p_server;
use crate::p2p_server::P2PServer;
use crate::treechain;
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::transaction_pool::{ScriptInfo, TransactionPool};
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
        Arc<Mutex<Wallet>>,
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
        wallet_arc,
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
            Arc::clone(&d.6),
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

    let wallet = {
        let guard = wallet_arc.lock().await;
        guard.clone()
    };
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
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
    body: web::Json<CreateSimpleTxnRequest>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (_, _, _, p2p_server, _, _, wallet, _, _, utxo_set, txn_pool) = data.as_ref();
    let wallet_guard = wallet.lock().await;

    let to_address = body.to_address.clone();
    let value = body.value;
    let fee: u64 = body.fee.unwrap_or(1_000_000);

    // Validate inputs
    if value == 0 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Value must be positive"
        }));
    }

    if ChainUtil::pubkey_hash_from_address(&to_address).is_err() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Invalid to_address"
        }));
    }

    let utxo_set_guard = utxo_set.lock().await;

    let treechain_guard = data.0.lock().await; // Get TreeChain for current height

    let (total_balance, unlocked_balance, locked_balance) =
        wallet_guard.get_balance(&wallet_guard.address, &utxo_set_guard, &treechain_guard);
    let available_balance = unlocked_balance + locked_balance; // Both can be spent if conditions met

    if available_balance < value + fee {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": format!(
                "Insufficient funds: need {}, available {} (unlocked: {}, locked: {})",
                value + fee, available_balance, unlocked_balance, locked_balance
            )
        }));
    }

    // Create transaction (handles timelocked UTXOs automatically)
    let mut tx = match Transaction::create_new_transaction(
        &wallet_guard,
        &utxo_set_guard,
        value,
        fee,
        &to_address,
        &treechain_guard,
    ) {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to create transaction: {}", e)
            }));
        }
    };

    // Sign the transaction (assumes sign_transaction handles P2SH/redeem)
    tx = tx
        .clone()
        .sign_transaction(&wallet_guard, &mut tx, &utxo_set_guard);
    // Add to mempool
    let mut txn_pool_guard = txn_pool.lock().await;
    match txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard, &treechain_guard) {
        Ok(_) => {
            drop(treechain_guard);
            p2p_server.broadcast_transaction(tx.clone()).await;
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "txid": tx.txid,
                "from_address": wallet_guard.address,
                "to_address": to_address,
                "value": value,
                "fee": fee,
                "total_input": value + fee,
                "wallet_balance": {
                    "total": total_balance,
                    "unlocked": unlocked_balance,
                    "locked": locked_balance
                },
                "transaction": tx
            }))
        }
        Err(e) => {
            drop(treechain_guard);
            HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to add to mempool: {}", e),
                "transaction": tx
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let wallet = data.6.clone();
    let mut wallet_guard = wallet.lock().await;
    let utxo_set_guard = data.9.lock().await;
    let treechain_guard = data.0.lock().await;

    let address = &wallet_guard.address;
    let pubkey_hash = &wallet_guard.public_key_hash;
    let pubkey = &wallet_guard.public_key;

    let (total_balance, unlocked_balance, locked_balance) =
        wallet_guard.get_balance(address, &utxo_set_guard, &treechain_guard);
    let utxos = Wallet::get_utxos_for_address(address, &utxo_set_guard, &treechain_guard); // Update to include timelocked if needed

    let mut total_received_estimate = 0;
    for (_, _, u) in &utxos {
        total_received_estimate += u.out.value;
    }

    let address_pubkey_hash = ChainUtil::pubkey_hash_from_address(address).unwrap_or_default();
    let derived_pubkey_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey);

    let address_verification = if address_pubkey_hash == derived_pubkey_hash {
        "valid".to_string()
    } else {
        "invalid".to_string()
    };

    let response = serde_json::json!({
        "wallet": {
            "address": address,
            "public_key": pubkey,
            "public_key_hash": pubkey_hash,
            "balance_satoshis": {
                "total": total_balance,
                "unlocked": unlocked_balance,
                "locked": locked_balance
            },
            "balance_btc": format!("{:.8}", total_balance as f64 / 100_000_000.0)
        },
        "utxos": {
            "count": utxos.len(),
            "total_value_satoshis": total_received_estimate,
            "dust_utxos": utxos.iter().filter(|(_, _, u)| u.out.value < 546).count()
        },
        "transaction_stats": {
            "total_received_satoshis": total_received_estimate,
            "total_received_btc": format!("{:.8}", total_received_estimate as f64 / 100_000_000.0),
            "total_sent_satoshis": 0, // Placeholder
            "total_sent_btc": "0.00000000",
            "net_balance_satoshis": total_balance
        },
        "security": {
            "address_verification": address_verification,
            "derived_pubkey_hash": derived_pubkey_hash,
            "expected_pubkey_hash": address_pubkey_hash,
            "wallet_format": "simple"
        },
        "metadata": {
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "node_version": "0.1.0",
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let treechain_guard = data.0.lock().await;
    let p2p_server = data.3.clone();
    let wallet_arc = data.6.clone();
    let utxo_set = data.9.clone();
    let wallet = wallet_arc.lock().await;
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
    let mut txn = Transaction::create_new_multisig_txn(
        &wallet,
        &utxo_set_guard,
        m,
        pubkeys,
        value,
        fee,
        &treechain_guard,
    )
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
        Arc<Mutex<Wallet>>,
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
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let treechain = data.0.clone();
    let treechain_guard = treechain.lock().await;
    let p2p_server = data.3.clone();
    let wallet_arc = data.6.clone();
    let wallet = wallet_arc.lock().await;
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

    if pubkeys.iter().any(|p| p == &wallet.public_key) {
        let sighash = spending_tx.compute_sighash(0, &redeem, input_value, SIGHASH_ALL);
        let sig = wallet.sign_data(&sighash);

        if !sigs.contains(&Some(sig.clone())) {
            sigs.push(Some(sig));
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

        match txn_pool_guard.add_transaction(spending_tx.clone(), &utxo_set_guard, &treechain_guard)
        {
            Ok(()) => {
                drop(treechain_guard);
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
                drop(treechain_guard);
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
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let wallet_arc = data.6.clone();
    let wallet = wallet_arc.lock().await;
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
struct CreateTimelockedBody {
    is_cltv: bool,
    lock: u32,
    value: u64,
    fee: u64,
    to_address: String,
}
//   sequence: if req.is_cltv { 0 } else { req.lock },
#[post("/create_timelocked_txn")]
async fn create_timelocked_txn(
    req: web::Json<CreateTimelockedBody>,
    data: web::Data<(
        Arc<Mutex<TreeChain>>,
        Arc<Mutex<PQP>>,
        Arc<Mutex<bool>>,
        Arc<P2PServer>,
        Arc<SyncMutex<Option<String>>>,
        Arc<SyncMutex<bool>>,
        Arc<Mutex<Wallet>>,
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let (
        _treechain,
        _pqp,
        _mining_flag,
        _p2p_server,
        _current_mining_position,
        _abort_mining,
        wallet_arc,
        _align,
        _chain_length,
        utxo_set,
        _txn_pool,
    ) = {
        let d = data.as_ref();
        (
            Arc::clone(&d.0),
            Arc::clone(&d.1),
            Arc::clone(&d.2),
            Arc::clone(&d.3),
            Arc::clone(&d.4),
            Arc::clone(&d.5),
            Arc::clone(&d.6),
            d.7,
            Arc::clone(&d.8),
            Arc::clone(&d.9),
            Arc::clone(&d.10),
        )
    };

    let wallet = {
        let guard = wallet_arc.lock().await;
        guard.clone()
    };

    let utxo_set_guard = utxo_set.lock().await;
    let treechain_guard = _treechain.lock().await;

    // Find a suitable UTXO (simplified: use first available with enough value)
    let available_utxos =
        Wallet::get_utxos_for_address(&wallet.address, &utxo_set_guard, &treechain_guard);
    let total_available: u64 = available_utxos.iter().map(|(_, _, u)| u.out.value).sum();
    if total_available < req.value + req.fee {
        drop(utxo_set_guard);
        drop(treechain_guard);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": format!("Insufficient balance: available {} < required {}", total_available, req.value + req.fee)
        }));
    }

    // For simplicity, assume we select one UTXO with enough value (in production, select optimally)
    let selected_utxo = available_utxos
        .iter()
        .find(|(_, _, u)| u.out.value >= req.value + req.fee)
        .expect("UTXO selection logic should ensure this");

    let input = TxInput {
        txid: selected_utxo.0.clone(),
        vout: selected_utxo.1,
        script_sig: "".to_string(), // Will be filled during signing
        sequence: if req.is_cltv { 0 } else { req.lock }, // Max sequence for funding tx (CSV enforced on spending input)
    };

    // Create redeem script for timelock (proper: push lock + opcode + drop + P2PKH)
    let to_pubkey_hash =
        ChainUtil::pubkey_hash_from_address(&req.to_address).expect("Invalid to_address");
    let p2pkh_script = format!("76a914{}88ac", to_pubkey_hash);

    // Lock as 4 LE bytes hex (e.g., 110 = "0000006e")
    let lock_bytes = req.lock.to_le_bytes();
    let lock_hex = format!(
        "{:02x}{:02x}{:02x}{:02x}",
        lock_bytes[0], lock_bytes[1], lock_bytes[2], lock_bytes[3]
    );
    let opcode = if req.is_cltv { "b1" } else { "b2" }; // b1=OP_CLTV, b2=OP_CSV

    let redeem_script = format!("04{}{}75{}", lock_hex, opcode, p2pkh_script);

    let redeem_bytes = match hex::decode(&redeem_script) {
        Ok(bytes) => bytes,
        Err(e) => {
            drop(utxo_set_guard);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to decode redeem script '{}': {}", redeem_script, e)
            }));
        }
    };

    let redeem_hash = ChainUtil::hash160(&redeem_bytes);
    let script_pubkey = format!("a914{}87", redeem_hash); // P2SH

    let mut outputs = vec![TxOutput {
        value: req.value,
        script_pubkey,
    }];

    // Add change output if needed
    let input_value = selected_utxo.2.out.value;
    if input_value > req.value + req.fee {
        let change_value = input_value - req.value - req.fee;
        let change_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
        outputs.push(TxOutput {
            value: change_value,
            script_pubkey: change_script,
        });
    }

    let locktime = if req.is_cltv { req.lock } else { 0 }; // CLTV uses tx.locktime; CSV does not
    let mut tx = Transaction::new(
        2, // Version 2 for timelocks
        locktime,
        vec![input],
        outputs,
        None,
    );
    tx = tx
        .clone()
        .sign_transaction(&wallet, &mut tx, &utxo_set_guard);
    drop(utxo_set_guard);
    drop(treechain_guard);
    let redeem_script_hex = hex::encode(redeem_bytes);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": format!("Timelocked txn created to address {}. Broadcast when conditions met.", req.to_address),
        "transaction": tx,
        "script_hash": redeem_hash,
        "redeem_script": redeem_script_hex
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
        Arc<Mutex<Wallet>>, // Updated for mutability
        u8,
        Arc<SyncMutex<u64>>,
        Arc<Mutex<UtxoSet>>,
        Arc<Mutex<TransactionPool>>,
    )>,
) -> impl Responder {
    let p2p_server = data.3.clone();
    let utxo_set = data.9.clone();
    let txn_pool = data.10.clone();
    let treechain = data.0.clone();
    let tx = body.transaction.clone();

    let utxo_set_guard = utxo_set.lock().await;
    let treechain_guard = treechain.lock().await;
    let current_height = treechain_guard.get_max_queue_index(); // Assume this method exists
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;

    // Check if this is a normal/CSV txn (locktime=0, all sequences=max) or CLTV funding (locktime>0)
    let is_normal_or_csv =
        tx.locktime == 0 && tx.vin.iter().all(|input| input.sequence == 0xffffffff);
    if !is_normal_or_csv {
        // Assume CLTV funding: enforce maturity on txn locktime
        let is_time_based = tx.locktime >= 500_000_000;
        let current = if is_time_based {
            current_time
        } else {
            current_height as u32
        };
        if current < tx.locktime {
            drop(treechain_guard);
            drop(utxo_set_guard);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": format!("CLTV funding txn immature: current {} < locktime {}", current, tx.locktime)
            }));
        }
        println!(
            "✅ CLTV funding txn mature: locktime {} <= current {}",
            tx.locktime, current
        );
    } else {
        println!("✅ Normal/CSV txn: no maturity check needed");
    }

    // Existing input-side timelock checks (for spending P2SH UTXOs)
    for (i, input) in tx.vin.iter().enumerate() {
        let utxo = match utxo_set_guard.get_utxo(&input.txid, input.vout) {
            Some(u) => u,
            None => {
                drop(treechain_guard);
                drop(utxo_set_guard);
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "message": format!("UTXO not found: {}:{}", input.txid, input.vout)
                }));
            }
        };

        let script_pubkey = &utxo.out.script_pubkey;
        if script_pubkey.starts_with("a914") && script_pubkey.ends_with("87") {
            let redeem_opt = Transaction::extract_redeem_from_scriptsig(&input.script_sig);
            let redeem = match redeem_opt {
                Some(r) => {
                    println!("redeem opt :{}", r.clone());
                    r
                }
                None => {
                    drop(treechain_guard);
                    drop(utxo_set_guard);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "status": "error",
                        "message": format!("No redeem in script_sig for {}:{}", input.txid, input.vout)
                    }));
                }
            };

            let redeem_bytes = match hex::decode(&redeem) {
                Ok(b) => b,
                Err(_) => {
                    drop(treechain_guard);
                    drop(utxo_set_guard);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "status": "error",
                        "message": "Invalid redeem hex"
                    }));
                }
            };

            let ops = TransactionPool::parse_script_ops(&redeem_bytes);
            if let Ok(info) = TransactionPool::get_script_info(&ops) {
                match info {
                    ScriptInfo::Cltv { lock, .. } => {
                        println!("Script info :{:?}", info);
                        let effective_lock = lock as u32;
                        if tx.locktime < effective_lock {
                            drop(treechain_guard);
                            drop(utxo_set_guard);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "status": "error",
                                "message": format!("CLTV tx.locktime {} < redeem lock {}", tx.locktime, effective_lock)
                            }));
                        }
                        // Additional maturity check: ensure current time/height >= tx.locktime
                        let is_time_based = effective_lock >= 500_000_000;
                        let current = if is_time_based {
                            current_time
                        } else {
                            current_height as u32
                        };
                        if current < tx.locktime {
                            drop(treechain_guard);
                            drop(utxo_set_guard);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "status": "error",
                                "message": format!("CLTV maturity not met: current {} < tx.locktime {}", current, tx.locktime)
                            }));
                        }
                    }
                    ScriptInfo::Csv { lock, .. } => {
                        println!("Script info :{:?}", info);
                        // Check input sequence >= lock
                        if input.sequence < lock as u32 {
                            drop(treechain_guard);
                            drop(utxo_set_guard);
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "status": "error",
                                "message": format!("CSV input.sequence {} < lock {}", input.sequence, lock)
                            }));
                        }
                        let blocks_since = current_height.saturating_sub(utxo.queue_index as u64);
                        if blocks_since < lock as u64 {
                            drop(treechain_guard);

                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "status": "error",
                                "message": format!("CSV not met: {} blocks < lock {} (queue_index: {}, current: {})", blocks_since, lock, utxo.queue_index, current_height)
                            }));
                        }
                    }
                    _ => {
                        println!("Script info :{:?}", info);
                    }
                }
            }
        }
    }

    // If checks pass, add to mempool and broadcast
    let mut txn_pool_guard = txn_pool.lock().await;
    match txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard, &treechain_guard) {
        Ok(_) => {
            drop(treechain_guard);
            drop(utxo_set_guard);
            drop(txn_pool_guard);
            p2p_server.broadcast_transaction(tx.clone()).await;
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Conditions met, added to mempool and broadcasted",
                "txid": tx.txid
            }))
        }
        Err(e) => {
            drop(treechain_guard);
            drop(utxo_set_guard);
            drop(txn_pool_guard);
            HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": e
            }))
        }
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
