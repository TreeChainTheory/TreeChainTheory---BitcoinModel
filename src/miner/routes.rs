use crate::chain_util::ChainUtil;
use crate::config::SIGHASH_ALL;
use crate::p2p_server::P2PServer;
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
use crate::wallet::transaction::Transaction;
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::UtxoSet;
use crate::wallet::wallet::Wallet;
use actix_web::{HttpResponse, Responder, get, web};
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

    // let tx = vec!["tx1".to_string(), "tx2".to_string()];
    let signature = "DUMMY_SIGNATURE_64_BYTES".to_string().repeat(3); //this has to be changed 

    //this should be updated to txn_pool.get_txns_by_align
    let tx1 = Transaction::create_sample_transaction_with_different_output();
    let tx2 = Transaction::create_sample_transaction();
    let tx = vec![tx1, tx2];

    let tag = "WOW";

    tokio::spawn(async move {
        loop {
            if !*mining_flag.lock().await {
                println!("Mining stopped");
                break;
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
                        signature.clone(),
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

                        let pqp_entry = TreeChain::parent_queue_entry_from_block(&mined_block);
                        pqp_guard.add_entry_to_pqp(pqp_entry.clone(), &tree);

                        let exist: bool = pqp_guard
                            .pool
                            .iter()
                            .rev()
                            .take(crate::config::CHILDREN as usize)
                            .any(|e| e.block_hash == pqp_entry.block_hash);

                        if exist {
                            if tree.verify_and_add_block(&mined_block) {
                                println!("Successfully mined block: {}", mined_block.hash);
                                drop(tree);
                                drop(pqp_guard);
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

#[get("/create_txn")]
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
    let (_, _, _, _, _, _, wallet, _, _, utxo_set, txn_pool) = data.as_ref();

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
        &wallet,
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
            for (i, (txid, vout, script_pubkey, value)) in input_data.iter().enumerate() {
                let sighash = tx.compute_sighash(i, script_pubkey, *value, SIGHASH_ALL);

                let sig_hex = wallet.sign_data(&sighash);
                tx.vin[i].script_sig = format!(
                    "{}{}{}",
                    format!("{:02x}", sig_hex.len() + 1), // sig length + sighash type
                    sig_hex,
                    format!(
                        "{}{}",
                        format!("{:02x}", wallet.public_key.len()), // pubkey length
                        wallet.public_key
                    )
                );
            }

            // Compute final txid after signing
            tx.txid = tx.compute_non_witness_txid();
            tx.hash = tx.compute_hash();

            // Lock transaction pool for adding
            let mut txn_pool_guard = txn_pool.lock().await;

            // Add transaction to mempool
            match txn_pool_guard.add_transaction(tx.clone(), &utxo_set_guard) {
                Ok(()) => {
                    println!(
                        "✅ Created and added simple txn to pool: {} ({} satoshis to {})",
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
}
