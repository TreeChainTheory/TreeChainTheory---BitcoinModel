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
//     to_address: &str,
//     fee_rate: u64,
// ) -> Transaction {
//     let mut tx = Transaction::create_new_transaction(wallet, utxo_set, pool, value, to_address)
//         .expect("Failed to create transaction");

//     for i in 0..tx.vin.len() {
//         // Get fields without holding a mutable borrow
//         let (txid, vout) = {
//             let input = &tx.vin[i];
//             (input.txid.clone(), input.vout)
//         };

//         // Look up UTXO
//         let utxo = utxo_set.get_utxo(&txid, vout).expect("UTXO not found");

//         // Compute sighash with correct prevout value
//         let sighash = tx.compute_sighash(i, &utxo.out.script_pubkey, utxo.out.value, SIGHASH_ALL);

//         // Sign the sighash
//         let sig = wallet.sign_data(&sighash);
//         let sig_bytes = hex::decode(&sig).expect("Invalid signature hex");
//         let pubkey_bytes = hex::decode(&wallet.public_key).expect("Invalid pubkey hex");

//         let mut script_sig = vec![];
//         script_sig.push(sig_bytes.len() as u8);
//         script_sig.extend_from_slice(&sig_bytes);
//         script_sig.push(pubkey_bytes.len() as u8);
//         script_sig.extend_from_slice(&pubkey_bytes);

//         // Update script_sig
//         tx.vin[i].script_sig = hex::encode(script_sig);
//     }

//     // Recalculate txid and hash after signing
//     tx.txid = tx.compute_non_witness_txid();
//     tx.hash = tx.compute_hash();

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

// fn add_utxos_for_wallet(utxo_set: &mut UtxoSet, wallet: &Wallet, value: u64, count: usize) {
//     let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
//     for i in 0..count {
//         let utxo = Utxo::new(
//             TxOutput {
//                 value,
//                 script_pubkey: sender_script.clone(),
//             },
//             100 + i as u32,
//             false,
//         );
//         let txid = generate_valid_txid(&format!("utxotxid{}", i));
//         utxo_set.add_utxo(txid, 0, utxo);
//     }
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
//         &to_address,
//         USER_TXN_FREERATE,
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
//     let pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );

//     let min_fee_rate = USER_TXN_FREERATE as f64;
//     let result = pool.validate_transaction(&tx, &utxo_set, min_fee_rate);
//     assert!(result.is_ok(), "Validation failed: {:?}", result.err());
//     let (fee, vsize, fee_rate, depends) = result.unwrap();
//     assert!(fee > 0);
//     assert!(vsize > 0);
//     assert!(fee_rate >= min_fee_rate);
//     assert!(depends.is_empty());
// }

// #[test]
// fn test_validate_transaction_failures() {
//     let pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000, 100000, 100000, 100000, 100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let mut tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );

//     // Too large
//     let large_tx = Transaction::new(
//         1,
//         0,
//         vec![],
//         vec![TxOutput {
//             value: 0,
//             script_pubkey: "0".repeat(MAX_TX_SIZE * 2),
//         }],
//         None,
//     );
//     let result = pool.validate_transaction(&large_tx, &utxo_set, 1.0);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("too large"));

//     // Locktime not reached
//     let future_time = (SystemTime::now()
//         .duration_since(UNIX_EPOCH)
//         .unwrap()
//         .as_secs()
//         + 3600) as u32;
//     tx.locktime = future_time;
//     let result = pool.validate_transaction(&tx, &utxo_set, 1.0);
//     assert!(result.is_err());
//     let err = result.unwrap_err();
//     assert!(err.contains("not yet reached"), "Error: {}", err);

//     // UTXO not found
//     let invalid_tx = Transaction::new(
//         1,
//         0,
//         vec![TxInput {
//             txid: generate_valid_txid("invalid"),
//             vout: 0,
//             script_sig: "".to_string(),
//             sequence: 0,
//         }],
//         vec![],
//         None,
//     );
//     let result = pool.validate_transaction(&invalid_tx, &utxo_set, 1.0);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("UTXO not found"));

//     // Invalid signature
//     let mut utxo_set_clone2 = utxo_set.clone();
//     let mut invalid_sig_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     invalid_sig_tx.vin[0].script_sig = "invalid".to_string();
//     let result = pool.validate_transaction(&invalid_sig_tx, &utxo_set, 1.0);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("Invalid signature"));

//     // Output > input
//     let mut utxo_set_clone3 = utxo_set.clone();
//     let invalid_value_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone3,
//         &pool,
//         600000, // Increased to ensure output > input
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     let result = pool.validate_transaction(&invalid_value_tx, &utxo_set, 1.0);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("exceeds input"));

//     // Low fee rate
//     let mut utxo_set_clone4 = utxo_set.clone();
//     let low_fee_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone4,
//         &pool,
//         99999,
//         &to_address,
//         USER_TXN_FREERATE - 1, // Use lower fee rate
//     );
//     let result = pool.validate_transaction(&low_fee_tx, &utxo_set, USER_TXN_FREERATE as f64);
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("below minimum"));
// }

// #[test]
// fn test_add_transaction_success() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );

//     let result = pool.add_transaction(tx.clone(), &utxo_set);
//     assert!(result.is_ok(), "Add transaction failed: {:?}", result.err());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&tx.txid).is_some());
//     assert_eq!(pool.utxo_set.utxos.len(), tx.vout.len());
//     assert!(pool.total_size > 0);
// }

// #[test]
// fn test_add_transaction_with_dependency() {
//     let mut pool = TransactionPool::new();
//     let wallet1 = Wallet::new();
//     let wallet2 = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet1, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let tx1 = create_signed_tx(
//         &wallet1,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &wallet2.address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();

//     let mut child_utxo_set = utxo_set.clone();
//     for (vout, output) in tx1.vout.iter().enumerate() {
//         let utxo = Utxo::new(output.clone(), 100, false);
//         child_utxo_set.add_utxo(tx1.txid.clone(), vout as u32, utxo);
//     }
//     let mut child_utxo_set_clone = child_utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet2,
//         &mut child_utxo_set_clone,
//         &pool,
//         20000,
//         &Wallet::new().address,
//         USER_TXN_FREERATE,
//     );
//     let result = pool.add_transaction(tx2.clone(), &child_utxo_set);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 2);
//     assert!(pool.get_transaction(&tx2.txid).is_some());
// }

// #[test]
// fn test_evict_low_fee_transactions() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000, 100000, 100000]);
//     let mut utxo_set_clone1 = utxo_set.clone();
//     let to_address = Wallet::new().address;

//     let tx1 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone1,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();

//     let mut utxo_set_clone2 = utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         60000,
//         &to_address,
//         USER_TXN_FREERATE + 1,
//     );
//     pool.add_transaction(tx2.clone(), &utxo_set).unwrap();

//     let vsize1 = pool.calculate_vsize(&tx1);
//     let vsize2 = pool.calculate_vsize(&tx2);
//     let result = pool.evict_low_fee_transactions(vsize1);
//     assert!(result.is_ok());
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&tx2.txid).is_some());
// }

// #[test]
// fn test_select_transactions() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000, 100000, 100000]);
//     let mut utxo_set_clone1 = utxo_set.clone();
//     let to_address = Wallet::new().address;

//     let tx1 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone1,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();

//     let mut utxo_set_clone2 = utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         60000,
//         &to_address,
//         USER_TXN_FREERATE + 1,
//     );
//     pool.add_transaction(tx2.clone(), &utxo_set).unwrap();

//     let max_block_size = pool.calculate_vsize(&tx1) + pool.calculate_vsize(&tx2);
//     let selected = pool.select_transactions(max_block_size, 0);
//     assert_eq!(selected.len(), 2);
//     assert!(selected.iter().any(|t| t.txid == tx1.txid));
//     assert!(selected.iter().any(|t| t.txid == tx2.txid));
// }

// #[test]
// fn test_tx_suitable_for_align() {
//     let pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );

//     let last_char = tx.txid.chars().last().unwrap();
//     let last_digit = u32::from_str_radix(&last_char.to_string(), 16).unwrap();
//     let expected_align = ((last_digit % CHILDREN as u32) + 1) as u8;

//     assert!(pool.tx_suitable_for_align(&tx, expected_align));
//     assert!(!pool.tx_suitable_for_align(&tx, (expected_align % CHILDREN + 1) as u8));
// }

// #[test]
// fn test_replace_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx_old = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     let old_txid = tx_old.vin[0].txid.clone(); // Save the UTXO txid used by tx_old
//     pool.add_transaction(tx_old.clone(), &utxo_set).unwrap();

//     // Create new transaction using the same UTXO as tx_old
//     let mut utxo_set_clone2 = utxo_set.clone();
//     let tx_new = Transaction::create_new_transaction(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         40000,
//         &to_address,
//     )
//     .expect("Failed to create new transaction");

//     let mut tx_new = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         40000,
//         &to_address,
//         USER_TXN_FREERATE + 1,
//     );
//     tx_new.vin[0].txid = old_txid; // Ensure tx_new spends the same UTXO
//     tx_new.txid = tx_new.compute_non_witness_txid();
//     tx_new.hash = tx_new.compute_hash();

//     let result = pool.replace_transaction(tx_new.clone(), &utxo_set);
//     assert!(
//         result.is_ok(),
//         "Replace transaction failed: {:?}",
//         result.err()
//     );
//     assert!(!pool.pool.contains_key(&tx_old.txid));
//     assert!(pool.pool.contains_key(&tx_new.txid));
// }
// #[test]
// fn test_parse_script_sig() {
//     let wallet = Wallet::new();
//     let data = b"test data";
//     let sig_hex = wallet.sign_data(data);
//     let pubkey_hex = wallet.public_key.clone();

//     let sig_bytes = hex::decode(&sig_hex).expect("Invalid sig hex");
//     let pubkey_bytes = hex::decode(&pubkey_hex).expect("Invalid pubkey hex");
//     let mut script_bytes = vec![];
//     script_bytes.push(sig_bytes.len() as u8);
//     script_bytes.extend_from_slice(&sig_bytes);
//     script_bytes.push(pubkey_bytes.len() as u8);
//     script_bytes.extend_from_slice(&pubkey_bytes);
//     let script_sig = hex::encode(script_bytes);

//     let result = TransactionPool::parse_script_sig(&script_sig);
//     assert!(result.is_ok());
//     let (parsed_sig, parsed_pubkey) = result.unwrap();
//     assert_eq!(parsed_sig, sig_hex);
//     assert_eq!(parsed_pubkey, pubkey_hex);

//     let invalid = "invalid".to_string();
//     let result = TransactionPool::parse_script_sig(&invalid);
//     assert!(result.is_err());
// }

// #[test]
// fn test_verify_signature() {
//     let wallet = Wallet::new();
//     let data = b"test data";
//     let sighash = Sha256::digest(data).to_vec();
//     let sig_hex = wallet.sign_data(&sighash);
//     let pubkey_hex = wallet.public_key.clone();

//     assert!(TransactionPool::verify_signature(
//         &sighash,
//         &sig_hex,
//         &pubkey_hex
//     ));

//     let invalid_sig = "invalid".to_string();
//     assert!(!TransactionPool::verify_signature(
//         &sighash,
//         &invalid_sig,
//         &pubkey_hex
//     ));
// }

// #[test]
// fn test_clear() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx, &utxo_set).unwrap();

//     pool.clear();
//     assert!(pool.is_empty());
//     assert!(pool.utxo_set.utxos.is_empty());
//     assert_eq!(pool.total_size, 0);
// }

// #[test]
// fn test_remove_confirmed_transactions() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000, 100000]);
//     let mut utxo_set_clone1 = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx1 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone1,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();

//     let mut utxo_set_clone2 = utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone2,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx2.clone(), &utxo_set).unwrap();

//     pool.remove_confirmed_transactions(&[tx1.clone()]);
//     assert_eq!(pool.len(), 1);
//     assert!(pool.get_transaction(&tx2.txid).is_some());
// }

// #[test]
// fn test_get_transaction() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
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
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
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
//     let mut utxo_set_clone = utxo_set.clone();
//     let tx1 = create_signed_tx(
//         &wallet1,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &wallet2.address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx1.clone(), &utxo_set).unwrap();

//     let mut child_utxo_set = utxo_set.clone();
//     for (vout, output) in tx1.vout.iter().enumerate() {
//         let utxo = Utxo::new(output.clone(), 100, false);
//         child_utxo_set.add_utxo(tx1.txid.clone(), vout as u32, utxo);
//     }
//     let mut child_utxo_set_clone = child_utxo_set.clone();
//     let tx2 = create_signed_tx(
//         &wallet2,
//         &mut child_utxo_set_clone,
//         &pool,
//         20000,
//         &Wallet::new().address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx2.clone(), &child_utxo_set).unwrap();

//     let spending = pool.get_transactions_spending_utxo(&tx1.txid, 0);
//     assert_eq!(spending.len(), 1);
//     assert_eq!(spending[0].txid, tx2.txid);
// }

// #[test]
// fn test_reorg() {
//     let mut pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     pool.add_transaction(tx.clone(), &utxo_set).unwrap();

//     let mut utxo_set_reorg = utxo_set.clone();
//     utxo_set_reorg.remove_utxo(&generate_valid_txid("utxo0"), 0);

//     pool.reorg(&utxo_set_reorg);
//     assert_eq!(pool.len(), 0);
// }

// #[test]
// fn test_segwit_validation() {
//     let pool = TransactionPool::new();
//     let wallet = Wallet::new();
//     let utxo_set = sample_utxo_set(&wallet, vec![100000]);
//     let mut utxo_set_clone = utxo_set.clone();
//     let to_address = Wallet::new().address;
//     let p2pkh_tx = create_signed_tx(
//         &wallet,
//         &mut utxo_set_clone,
//         &pool,
//         50000,
//         &to_address,
//         USER_TXN_FREERATE,
//     );
//     let segwit_tx = p2pkh_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, None)
//         .unwrap();

//     let result = pool.validate_transaction(&segwit_tx, &utxo_set, USER_TXN_FREERATE as f64);
//     assert!(result.is_ok());
// }
