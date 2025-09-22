// use crate::chain_util::ChainUtil;
// use crate::config::{CHILDREN, USER_TXN_FREERATE};
// use crate::wallet::transaction::{SIGHASH_ALL, Transaction, TxInput, TxOutput};
// use crate::wallet::transaction_pool::{
//     MAX_MEMPOOL_SIZE, MAX_TX_SIZE, MempoolEntry, TransactionPool,
// };
// use crate::wallet::utxo::{Utxo, UtxoSet};
// use crate::wallet::wallet::Wallet;
// use hex;
// use k256::ecdsa::{Signature, VerifyingKey};
// use sha2::{Digest, Sha256};
// use std::collections::{HashMap, HashSet};
// use std::time::{Duration, SystemTime, UNIX_EPOCH};

// // Helper function to generate a valid 64-character hex txid
// fn generate_valid_txid(seed: &str) -> String {
//     let mut hasher = Sha256::new();
//     hasher.update(seed);
//     let hash = hasher.finalize();
//     hex::encode(hash) // 32 bytes -> 64 hex chars
// }

// fn create_signed_tx(
//     wallet: &Wallet,
//     utxo_set: &mut UtxoSet,
//     pool: &TransactionPool,
//     value: u64,
//     fee: u64,
//     to_address: &str,
// ) -> Transaction {
//     let mut tx = Transaction::create_new_transaction(wallet, utxo_set, value, fee, to_address)
//         .expect("Failed to create transaction");
//     tx = tx.clone().sign_transaction(wallet, &mut tx, utxo_set);

//     tx
// }

// fn sample_utxo_set(wallet: &Wallet, values: Vec<u64>) -> UtxoSet {
//     let mut utxo_set = UtxoSet::new();
//     let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
//     for (i, value) in values.iter().enumerate() {
//         let utxo = Utxo::new(
//             TxOutput {
//                 value: *value,
//                 script_pubkey: sender_script.clone(),
//             },
//             100 + i as u32,
//             false,
//         );
//         let txid = generate_valid_txid(&format!("utxo{}", i));
//         utxo_set.add_utxo(txid, 0, utxo);
//     }
//     utxo_set
// }

// #[test]
// fn test_new_transaction_pool() {
//     let pool = TransactionPool::new();
//     assert!(pool.pool.is_empty());
//     assert!(pool.utxo_set.utxos.is_empty());
//     assert_eq!(pool.total_size, 0);
// }

// #[test]
// fn test_calculate_vsize() {
//     let pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let mut utxo_set_clone = sample_utxo_set(&wallet, vec![100000]);
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );

//     let vsize = pool.calculate_vsize(&tx);
//     assert!(vsize > 0);
//     assert!(
//         vsize >= 100 && vsize <= 300,
//         "vsize {} not in expected range 100-300",
//         vsize
//     );
// }

// #[test]
// fn test_validate_transaction_success() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set to simulate availability
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );

//     let min_fee_rate = USER_TXN_FREERATE as f64;
//     let result = pool.validate_transaction(&tx, min_fee_rate, &utxo_set);
//     assert!(result.is_ok(), "Validation failed: {:?}", result.err());
//     let (fee, vsize, fee_rate, depends) = result.unwrap();
//     assert_eq!(fee, 3000, "Fee should match provided fee");
//     assert!(vsize > 0);
//     assert!(
//         fee_rate >= min_fee_rate,
//         "Fee rate {} should be >= {}",
//         fee_rate,
//         min_fee_rate
//     );
//     assert!(depends.is_empty());
// }

// #[test]
// fn test_validate_transaction_failures() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000, 100000, 100000, 100000, 100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let mut tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );

//     // Too large
//     let mut large_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set.clone(),
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     let large_script = "00".repeat(100_001);
//     large_tx.vin[0].script_sig = large_script;
//     let result = pool.validate_transaction(&large_tx, USER_TXN_FREERATE as f64, &utxo_set_clone);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("Transaction too large"));

//     // Invalid locktime
//     let current_time = SystemTime::now()
//         .duration_since(UNIX_EPOCH)
//         .unwrap()
//         .as_secs() as u32;
//     tx.locktime = current_time + 3600; // 1 hour in future
//     let result = pool.validate_transaction(&tx, USER_TXN_FREERATE as f64, &utxo_set_clone);
//     assert!(result.is_err(), "Expected locktime error, got {:?}", result);
//     // assert!(
//     //     result.clone().unwrap_err().contains("locktime"),
//     //     "Error message: {}",
//     //     result.unwrap_err()
//     // );

//     // Missing UTXO
//     let mut empty_pool = TransactionPool::new();
//     let empty_utxo_set = UtxoSet::new();
//     let result = empty_pool.validate_transaction(&tx, USER_TXN_FREERATE as f64, &empty_utxo_set);
//     assert!(result.is_err());
//     println!("result err: {:?}", result);
//     assert!(result.unwrap_err().contains("UTXO not found"));

//     // Invalid signature
//     let mut invalid_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set.clone(),
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     invalid_tx.vin[0].script_sig = "00000000".repeat(8).to_string(); //invalid hex
//     let result = pool.validate_transaction(&invalid_tx, USER_TXN_FREERATE as f64, &utxo_set_clone);
//     println!("result: {:?}", result);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("Invalid signature"));

//     // Insufficient fee
//     let low_fee_tx = create_signed_tx(&wallet, &mut utxo_set.clone(), &pool, 99999, 1, &to_address);
//     let result = pool.validate_transaction(&low_fee_tx, (USER_TXN_FREERATE * 2) as f64, &utxo_set);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("Fee rate"));
// }

// #[test]
// fn test_add_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     println!("current_utxo set: {:?}", utxo_set);

//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     println!("UTXO set after creating tx: {:?}", utxo_set_clone);
//     let result = pool.add_transaction(tx.clone(), &utxo_set_clone);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&tx.txid).is_some());
//     println!("UTXO set after adding tx: {:?}", pool.utxo_set);
//     assert_eq!(pool.utxo_set.utxos.len(), 2); // Recipient and change outputs
// }

// #[test]
// fn test_select_transactions() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     // Create full UTXO set with two UTXOs (1,000,000 satoshis each) for the pool
//     let utxo_set = sample_utxo_set(&wallet, vec![1000000, 1000000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     println!(
//         "\nInitial UTXO set: {:?}",
//         pool.utxo_set.utxos.keys().collect::<Vec<_>>()
//     );

//     let to_address = Wallet::new().address;

//     // Extract individual UTXOs from the sample set (assuming sample_utxo_set creates them in consistent order)
//     let mut utxo_iter = utxo_set.utxos.iter();
//     let utxo1_opt = utxo_iter.next(); // First UTXO (e.g., c12d70aa...:0)
//     let utxo2_opt = utxo_iter.next(); // Second UTXO (e.g., f7a4e07c...:0)

//     if utxo1_opt.is_none() || utxo2_opt.is_none() {
//         panic!("sample_utxo_set must have exactly 2 UTXOs");
//     }

//     let (txid1, vout1) = utxo1_opt.unwrap().0;
//     let utxo1 = utxo1_opt.unwrap().1.clone();
//     let (txid2, vout2) = utxo2_opt.unwrap().0;
//     let utxo2 = utxo2_opt.unwrap().1.clone();

//     // Create custom UTXO set for tx1 (only first UTXO)
//     let mut utxo_set_clone1 = UtxoSet::new();
//     utxo_set_clone1.add_utxo(txid1.clone(), *vout1, utxo1);

//     // Create first transaction with lower fee rate (~11.67 sat/vB)
//     let tx1 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone1,
//         &pool,
//         50000, // Value
//         3000,  // Fee
//         &to_address,
//     );
//     let (_, tx1_vsize, tx1_weight) = tx1.get_size_vsize_weight();
//     let tx1_fee_rate = 3000.0 / tx1_vsize as f64;
//     println!(
//         "\nTransaction 1: txid={}, vsize={}, weight={}, fee={}, fee_rate={:.2} sat/vB",
//         tx1.txid, tx1_vsize, tx1_weight, 3000, tx1_fee_rate
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();
//     println!(
//         "\nUTXO set after adding tx1: {:?}",
//         pool.utxo_set.utxos.keys().collect::<Vec<_>>()
//     );

//     // Create custom UTXO set for tx2 (only second UTXO)
//     let mut utxo_set_clone2 = UtxoSet::new();
//     utxo_set_clone2.add_utxo(txid2.clone(), *vout2, utxo2);

//     // Create second transaction with higher fee rate (~19.46 sat/vB)
//     let tx2 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         50000, // Value
//         5000,  // Fee
//         &to_address,
//     );
//     let (_, tx2_vsize, tx2_weight) = tx2.get_size_vsize_weight();
//     let tx2_fee_rate = 5000.0 / tx2_vsize as f64;
//     println!(
//         "\nTransaction 2: txid={}, vsize={}, weight={}, fee={}, fee_rate={:.2} sat/vB",
//         tx2.txid, tx2_vsize, tx2_weight, 5000, tx2_fee_rate
//     );
//     pool.add_transaction(tx2.clone(), &utxo_set).unwrap();
//     println!(
//         "\nUTXO set after adding tx2: {:?}",
//         pool.utxo_set.utxos.keys().collect::<Vec<_>>()
//     );

//     // Scenario 1: Select both transactions with sufficient weight limit
//     let max_block_weight = 2090; // Enough for both txs (each ~1028 weight)
//     let selected = pool.select_transactions(max_block_weight, 0);
//     println!(
//         "\nScenario 1 (max_weight={}): Selected transactions: {:?}",
//         max_block_weight,
//         selected.iter().map(|tx| &tx.txid).collect::<Vec<_>>()
//     );

//     assert_eq!(
//         selected.len(),
//         2,
//         "Scenario 1: Expected 2 transactions, got {}",
//         selected.len()
//     );
//     assert_eq!(
//         selected[0].txid, tx2.txid,
//         "Scenario 1: First selected transaction should be tx2 (higher fee rate: {:.2} vs {:.2})",
//         tx2_fee_rate, tx1_fee_rate
//     );
//     assert_eq!(
//         selected[1].txid, tx1.txid,
//         "Scenario 1: Second selected transaction should be tx1"
//     );

//     // Verify total weight
//     let total_weight: u64 = selected.iter().map(|tx| tx.get_size_vsize_weight().2).sum();
//     assert!(
//         total_weight <= max_block_weight as u64,
//         "Scenario 1: Total weight {} exceeds max weight {}",
//         total_weight,
//         max_block_weight
//     );

//     // Scenario 2: Select only one transaction with restrictive weight limit
//     let max_block_weight = 1028; // Enough for one tx (~1028 weight)
//     let selected = pool.select_transactions(max_block_weight, 0);
//     println!(
//         "\nScenario 2 (max_weight={}): Selected transactions: {:?}",
//         max_block_weight,
//         selected.iter().map(|tx| &tx.txid).collect::<Vec<_>>()
//     );

//     assert_eq!(
//         selected.len(),
//         1,
//         "Scenario 2: Expected 1 transaction, got {}",
//         selected.len()
//     );
//     assert_eq!(
//         selected[0].txid, tx2.txid,
//         "Scenario 2: Selected transaction should be tx2 (higher fee rate: {:.2} vs {:.2})",
//         tx2_fee_rate, tx1_fee_rate
//     );

//     // Verify total weight
//     let total_weight: u64 = selected.iter().map(|tx| tx.get_size_vsize_weight().2).sum();
//     assert!(
//         total_weight <= max_block_weight as u64,
//         "Scenario 2: Total weight {} exceeds max weight {}",
//         total_weight,
//         max_block_weight
//     );

//     // Verify UTXO set contains expected outputs
//     assert_eq!(
//         pool.utxo_set.utxos.len(),
//         4, // Two outputs from tx1 (recipient + change) + two from tx2
//         "UTXO set should contain outputs from both transactions"
//     );
// }
// #[test]
// fn test_get_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     pool.add_transaction(tx.clone(), &utxo_set).unwrap();

//     let got_tx = pool.get_transaction(&tx.txid).unwrap();
//     assert_eq!(got_tx.txid, tx.txid);
//     assert!(
//         pool.get_transaction(&generate_valid_txid("invalid"))
//             .is_none()
//     );
// }

// #[test]
// fn test_len_and_is_empty() {
//     let mut pool = TransactionPool::new();
//     assert!(pool.is_empty());
//     assert_eq!(pool.len(), 0);

//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     pool.add_transaction(tx, &utxo_set).unwrap();

//     assert!(!pool.is_empty());
//     assert_eq!(pool.len(), 1);
// }

// #[test]
// fn test_get_transactions_spending_utxo() {
//     let mut pool = TransactionPool::new();
//     let wallet1 = Wallet::new();
//     let wallet2 = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet1, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let tx1 = create_signed_tx(
//         &wallet1,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &wallet2.address,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set_clone).unwrap();

//     let mut child_utxo_set = utxo_set.clone();
//     for (vout, output) in tx1.vout.iter().enumerate() {
//         let utxo = Utxo::new(output.clone(), 100, false);
//         child_utxo_set.add_utxo(tx1.txid.clone(), vout as u32, utxo.clone());
//         pool.utxo_set.add_utxo(tx1.txid.clone(), vout as u32, utxo);
//     }
//     let mut child_utxo_set_clone = child_utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet2,
//         &mut child_utxo_set_clone,
//         &pool,
//         20000,
//         3000,
//         &Wallet::new().address,
//     );
//     pool.add_transaction(tx2.clone(), &child_utxo_set_clone)
//         .unwrap();

//     let spending = pool.get_transactions_spending_utxo(&tx1.txid, 0);
//     assert_eq!(spending.len(), 1);
//     assert_eq!(spending[0].txid, tx2.txid);
// }

// #[test]
// fn test_reorg() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     pool.add_transaction(tx.clone(), &utxo_set).unwrap();

//     // Simulate reorg by clearing pool's UTXO set
//     pool.utxo_set = UtxoSet::new();
//     pool.reorg(&utxo_set);
//     assert_eq!(pool.len(), 0);
// }

// #[test]
// fn test_segwit_validation() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let p2pkh_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         3000,
//         &to_address,
//     );
//     let segwit_tx = p2pkh_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, None)
//         .unwrap();

//     let result = pool.validate_transaction(&segwit_tx, USER_TXN_FREERATE as f64, &utxo_set);
//     assert!(result.is_ok());
// }

// #[test]
// fn test_multisig_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let wallet2 = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let pubkeys = vec![wallet.public_key.clone(), wallet2.public_key.clone()];
//     let tx = Transaction::create_new_multisig_txn(
//         &wallet,
//         &utxo_set_clone,
//         2,
//         pubkeys.clone(),
//         50000,
//         3000,
//     )
//     .expect("Failed to create multisig transaction");

//     // Sign the transaction
//     let mut signed_tx = tx.clone();
//     for i in 0..signed_tx.vin.len() {
//         let (txid, vout) = {
//             let input = &signed_tx.vin[i];
//             (input.txid.clone(), input.vout)
//         };
//         let utxo = utxo_set.get_utxo(&txid, vout).expect("UTXO not found");
//         let sighash =
//             signed_tx.compute_sighash(i, &utxo.out.script_pubkey, utxo.out.value, SIGHASH_ALL);
//         let sig = wallet.sign_data(&sighash);
//         let sig_bytes = hex::decode(&sig).expect("Invalid signature hex");
//         let pubkey_bytes = hex::decode(&wallet.public_key).expect("Invalid pubkey hex");
//         let mut script_sig = vec![];
//         script_sig.push(sig_bytes.len() as u8);
//         script_sig.extend_from_slice(&sig_bytes);
//         script_sig.push(pubkey_bytes.len() as u8);
//         script_sig.extend_from_slice(&pubkey_bytes);
//         signed_tx.vin[i].script_sig = hex::encode(script_sig);
//     }
//     signed_tx.txid = signed_tx.compute_non_witness_txid();
//     signed_tx.hash = signed_tx.compute_hash();

//     let result = pool.add_transaction(signed_tx.clone(), &utxo_set);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&signed_tx.txid).is_some());
//     assert_eq!(pool.utxo_set.utxos.len(), 2); // Multisig and change outputs
// }

// #[test]
// fn test_timelocked_cltv_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = Transaction::create_new_timelocked_cltv_txn(
//         &wallet,
//         &utxo_set_clone,
//         50000,
//         3000,
//         500000,
//         &to_address,
//     )
//     .expect("Failed to create CLTV transaction");

//     // Sign the transaction
//     let mut signed_tx = tx.clone();
//     for i in 0..signed_tx.vin.len() {
//         let (txid, vout) = {
//             let input = &signed_tx.vin[i];
//             (input.txid.clone(), input.vout)
//         };
//         let utxo = utxo_set.get_utxo(&txid, vout).expect("UTXO not found");
//         let sighash =
//             signed_tx.compute_sighash(i, &utxo.out.script_pubkey, utxo.out.value, SIGHASH_ALL);
//         let sig = wallet.sign_data(&sighash);
//         let sig_bytes = hex::decode(&sig).expect("Invalid signature hex");
//         let pubkey_bytes = hex::decode(&wallet.public_key).expect("Invalid pubkey hex");
//         let mut script_sig = vec![];
//         script_sig.push(sig_bytes.len() as u8);
//         script_sig.extend_from_slice(&sig_bytes);
//         script_sig.push(pubkey_bytes.len() as u8);
//         script_sig.extend_from_slice(&pubkey_bytes);
//         signed_tx.vin[i].script_sig = hex::encode(script_sig);
//     }
//     signed_tx.txid = signed_tx.compute_non_witness_txid();
//     signed_tx.hash = signed_tx.compute_hash();

//     let result = pool.add_transaction(signed_tx.clone(), &utxo_set);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&signed_tx.txid).is_some());
//     assert_eq!(pool.utxo_set.utxos.len(), 2); // CLTV and change outputs
// }

// #[test]
// fn test_timelocked_csv_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     // Add UTXOs to pool's UTXO set
//     for ((txid, vout), utxo) in utxo_set.utxos.iter() {
//         pool.utxo_set.add_utxo(txid.clone(), *vout, utxo.clone());
//     }
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = Transaction::create_new_timelocked_csv_txn(
//         &wallet,
//         &utxo_set_clone,
//         50000,
//         3000,
//         100,
//         &to_address,
//     )
//     .expect("Failed to create CSV transaction");

//     // Sign the transaction
//     let mut signed_tx = tx.clone();
//     for i in 0..signed_tx.vin.len() {
//         let (txid, vout) = {
//             let input = &signed_tx.vin[i];
//             (input.txid.clone(), input.vout)
//         };
//         let utxo = utxo_set.get_utxo(&txid, vout).expect("UTXO not found");
//         let sighash =
//             signed_tx.compute_sighash(i, &utxo.out.script_pubkey, utxo.out.value, SIGHASH_ALL);
//         let sig = wallet.sign_data(&sighash);
//         let sig_bytes = hex::decode(&sig).expect("Invalid signature hex");
//         let pubkey_bytes = hex::decode(&wallet.public_key).expect("Invalid pubkey hex");
//         let mut script_sig = vec![];
//         script_sig.push(sig_bytes.len() as u8);
//         script_sig.extend_from_slice(&sig_bytes);
//         script_sig.push(pubkey_bytes.len() as u8);
//         script_sig.extend_from_slice(&pubkey_bytes);
//         signed_tx.vin[i].script_sig = hex::encode(script_sig);
//     }
//     signed_tx.txid = signed_tx.compute_non_witness_txid();
//     signed_tx.hash = signed_tx.compute_hash();

//     let result = pool.add_transaction(signed_tx.clone(), &utxo_set);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&signed_tx.txid).is_some());
//     assert_eq!(pool.utxo_set.utxos.len(), 2); // CSV and change outputs
// }
