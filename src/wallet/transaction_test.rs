// use crate::chain_util::ChainUtil;
// use crate::config::SIGHASH_ALL;
// use crate::config::USER_TXN_FREERATE as DEFAULT_MIN_FEE_RATE;
// use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
// use crate::wallet::transaction_pool::TransactionPool;
// use crate::wallet::utxo::{Utxo, UtxoSet};
// use crate::wallet::wallet::Wallet;
// use hex;

// fn sample_input(txid: &str, vout: u32, script_sig: &str) -> TxInput {
//     TxInput {
//         txid: txid.to_string(),
//         vout,
//         script_sig: hex::encode(script_sig),
//         sequence: 0xffffffff,
//     }
// }

// fn sample_output(value: u64, pubkey_hash: &str, is_p2wpkh: bool) -> TxOutput {
//     TxOutput {
//         value,
//         script_pubkey: if is_p2wpkh {
//             format!("0014{}", pubkey_hash)
//         } else {
//             format!("76a914{}88ac", pubkey_hash)
//         },
//     }
// }

// #[test]
// fn test_new_transaction() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
//     let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);

//     assert_eq!(tx.version, 1);
//     assert_eq!(tx.locktime, 0);
//     assert_eq!(tx.vin, inputs);
//     assert_eq!(tx.vout, outputs);
//     assert!(!tx.txid.is_empty(), "TXID should be computed");
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");
//     assert_eq!(
//         tx.vout[0].script_pubkey,
//         format!("76a914{}88ac", wallet.public_key_hash)
//     );
// }

// #[test]
// fn test_new_coinbase() {
//     let wallet = Wallet::new();
//     let tx = Transaction::new_coinbase(
//         1,
//         wallet.address.clone(),
//         123,
//         5000000000,
//         100000,
//         "nonce",
//         "tag",
//     );

//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(tx.vout.len(), 1);
//     assert_eq!(tx.vin[0].txid, "00".repeat(32));
//     assert_eq!(tx.vin[0].vout, u32::MAX);
//     assert_eq!(tx.vout[0].value, 5000100000);
//     assert_eq!(tx.locktime, 0);
//     assert!(!tx.txid.is_empty(), "TXID should be computed");
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");
//     assert_eq!(
//         tx.vout[0].script_pubkey,
//         format!("76a914{}88ac", wallet.public_key_hash)
//     );

//     let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
//     assert_eq!(utxo.out.value, 5000100000);
//     assert_eq!(utxo.queue_index, 123456);
//     assert!(utxo.f_coinbase);
// }

// #[test]
// fn test_create_normal_txn() {
//     let wallet1 = Wallet::new();
//     let wallet2 = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let recipients = vec![
//         (1000000, wallet1.address.clone()),
//         (2000000, wallet2.address.clone()),
//     ];
//     let tx = Transaction::create_normal_txn(1, inputs.clone(), recipients, 0);

//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(tx.vout.len(), 2);
//     assert_eq!(tx.vout[0].value, 1000000);
//     assert_eq!(tx.vout[1].value, 2000000);
//     assert!(!tx.txid.is_empty());
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");
//     assert_eq!(
//         tx.vout[0].script_pubkey,
//         format!("76a914{}88ac", wallet1.public_key_hash)
//     );
//     assert_eq!(
//         tx.vout[1].script_pubkey,
//         format!("76a914{}88ac", wallet2.public_key_hash)
//     );
// }

// #[test]
// fn test_create_multisig_txn() {
//     let wallet1 = Wallet::new();
//     let wallet2 = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let pubkeys = vec![wallet1.public_key.clone(), wallet2.public_key.clone()];
//     let tx = Transaction::create_multisig_txn(1, inputs.clone(), 2, pubkeys.clone(), 1000000, 0);

//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(tx.vout.len(), 1);
//     assert_eq!(tx.vout[0].value, 1000000);
//     assert!(tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(tx.vout[0].script_pubkey.ends_with("87"));
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");

//     let result = std::panic::catch_unwind(|| {
//         Transaction::create_multisig_txn(1, inputs, 3, pubkeys, 1000000, 0)
//     });
//     assert!(result.is_err(), "Should panic when m > n");
// }

// #[test]
// fn test_create_timelocked_txn() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let tx = Transaction::create_timelocked_txn(
//         1,
//         inputs.clone(),
//         500000,
//         wallet.address.clone(),
//         1000000,
//         500000,
//     );

//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(tx.vout.len(), 1);
//     assert_eq!(tx.vout[0].value, 1000000);
//     assert!(tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(tx.vout[0].script_pubkey.ends_with("87"));
//     assert_eq!(tx.locktime, 500000);
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");
// }

// #[test]
// fn test_create_timelocked_csv_txn() {
//     let wallet = Wallet::new();
//     let recipient = Wallet::new();
//     let mut utxo_set = UtxoSet::new();

//     // Setup UTXO
//     let utxo = Utxo::new(
//         TxOutput {
//             value: 1500000,
//             script_pubkey: Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     utxo_set.add_utxo("1234".repeat(16), 0, utxo);

//     let value = 1000000;
//     let fee = 1000;
//     let csv_lock_blocks = 100;
//     let tx = Transaction::create_new_timelocked_csv_txn(
//         &wallet,
//         &utxo_set,
//         value,
//         fee,
//         csv_lock_blocks,
//         &recipient.address,
//     )
//     .expect("Failed to create CSV transaction");

//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(
//         tx.vin[0].sequence, csv_lock_blocks,
//         "Sequence should be set to csv_lock_blocks"
//     );
//     assert_eq!(tx.vout.len(), 2, "Should include change output");
//     assert_eq!(tx.vout[0].value, value);
//     assert_eq!(
//         tx.vout[1].value,
//         1500000 - value - fee,
//         "Change should account for fee"
//     );
//     assert!(tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(tx.vout[0].script_pubkey.ends_with("87"));
//     assert_eq!(
//         tx.vout[1].script_pubkey,
//         Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         "Change output should use sender's script"
//     );
//     assert!(!tx.txid.is_empty());
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");

//     // Test insufficient funds
//     let result = Transaction::create_new_timelocked_csv_txn(
//         &wallet,
//         &utxo_set,
//         2000000,
//         fee,
//         csv_lock_blocks,
//         &recipient.address,
//     );
//     assert!(result.is_err());
//     assert!(result.unwrap_err().contains("Insufficient funds"));
// }
// #[test]
// fn test_compute_txid_and_hash() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
//     let tx1 = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);
//     let tx2 = Transaction::new(1, 0, inputs, outputs.clone(), None);

//     assert_eq!(
//         tx1.txid, tx2.txid,
//         "Identical non-SegWit transactions should have same TXID"
//     );
//     assert_eq!(
//         tx1.hash, tx2.hash,
//         "Identical non-SegWit transactions should have same hash"
//     );
//     assert_eq!(tx1.hash, tx1.txid, "Hash should equal TXID for non-SegWit");

//     let different_inputs = vec![sample_input(&"5678".repeat(16), 0, "signature+pubkey")];
//     let tx3 = Transaction::new(1, 0, different_inputs, outputs.clone(), None);
//     assert_ne!(
//         tx1.txid, tx3.txid,
//         "Different inputs should yield different TXID"
//     );
//     assert_ne!(
//         tx1.hash, tx3.hash,
//         "Different inputs should yield different hash"
//     );
// }

// #[test]
// fn test_compute_sighash() {
//     let wallet = Wallet::new();
//     let inputs = vec![
//         sample_input(&"1234".repeat(16), 0, "signature+pubkey"),
//         sample_input(&"5678".repeat(16), 1, "signature2+pubkey2"),
//     ];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
//     let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);

//     let sighash = tx.compute_sighash(
//         0,
//         &wallet.public_key_hash,
//         1500000,
//         crate::wallet::transaction::SIGHASH_ALL,
//     );
//     assert_eq!(sighash.len(), 32, "Sighash should be 32 bytes");

//     let sighash2 = tx.compute_sighash(
//         1,
//         &wallet.public_key_hash,
//         1500000,
//         crate::wallet::transaction::SIGHASH_ALL,
//     );
//     assert_ne!(
//         sighash, sighash2,
//         "Sighashes for different inputs should differ"
//     );
// }

// #[test]
// fn test_compute_segwit_sighash() {
//     let wallet = Wallet::new();
//     let inputs = vec![
//         sample_input(&"1234".repeat(16), 0, ""),
//         sample_input(&"5678".repeat(16), 1, ""),
//     ];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, true)];
//     // Generate valid witness data
//     let dummy_data = vec![0u8; 32]; // Dummy hash for signing
//     let sig1 = wallet.sign_data(&dummy_data);
//     let sig2 = wallet.sign_data(&dummy_data); // Could use a different wallet or data
//     let witnesses = Some(vec![
//         vec![sig1.clone(), wallet.public_key.clone()],
//         vec![sig2, wallet.public_key.clone()],
//     ]);
//     let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), witnesses);

//     let sighash = tx.compute_segwit_sighash(
//         0,
//         1500000,
//         &Transaction::create_p2wpkh_script(&wallet.public_key_hash),
//         crate::wallet::transaction::SIGHASH_ALL,
//     );
//     assert_eq!(sighash.len(), 32, "SegWit sighash should be 32 bytes");

//     let sighash2 = tx.compute_segwit_sighash(
//         1,
//         1500000,
//         &Transaction::create_p2wpkh_script(&wallet.public_key_hash),
//         crate::wallet::transaction::SIGHASH_ALL,
//     );
//     assert_ne!(
//         sighash, sighash2,
//         "SegWit sighashes for different inputs should differ"
//     );
// }

// #[test]
// fn test_get_size_vsize_weight() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "")]; // Empty script_sig for non-SegWit
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
//     let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);
//     let (size, vsize, weight) = tx.get_size_vsize_weight();

//     assert!(size > 0, "Size should be positive");
//     assert!(vsize > 0, "VSize should be positive");
//     assert!(weight > 0, "Weight should be positive");
//     assert_eq!(
//         vsize as u64,
//         (weight + 3) / 4,
//         "VSize should be ceiling of weight/4"
//     );

//     let dummy_data = vec![0u8; 32]; // Dummy hash for signing
//     let sig = wallet.sign_data(&dummy_data);
//     let witnesses = Some(vec![vec![sig, wallet.public_key.clone()]]);
//     let segwit_tx = Transaction::new(1, 0, inputs, outputs, witnesses);
//     let (segwit_size, segwit_vsize, segwit_weight) = segwit_tx.get_size_vsize_weight();
//     assert!(
//         segwit_size > size,
//         "SegWit transaction size should be larger due to witnesses"
//     );
//     assert!(
//         segwit_vsize < segwit_size,
//         "SegWit vsize should be less than total SegWit size due to weight discount"
//     );
// }
// #[test]
// fn test_segwit_transaction() {
//     let wallet = Wallet::new();
//     let wallet2 = Wallet::new();
//     let mut utxo_set = UtxoSet::new();
//     let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
//     let utxo = Utxo::new(
//         TxOutput {
//             value: 150000,
//             script_pubkey: sender_script.clone(),
//         },
//         100,
//         false,
//     );
//     utxo_set.add_utxo("1234".repeat(16), 0, utxo);
//     let recipient_address = wallet2.address.clone();

//     // Test 1: P2PKH to SegWit (P2WPKH)
//     let p2pkh_tx =
//         Transaction::create_new_transaction(&wallet, &utxo_set, 90000, 1000, &recipient_address)
//             .expect("Failed to create P2PKH transaction");
//     let segwit_tx = p2pkh_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, None)
//         .expect("Failed to modify transaction to SegWit");

//     assert!(segwit_tx.witnesses.is_some(), "Witnesses should be present");
//     let witnesses = segwit_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         witnesses.len(),
//         p2pkh_tx.vin.len(),
//         "Witness count should match input count"
//     );
//     assert!(
//         !witnesses[0].is_empty(),
//         "Witness stack should contain data"
//     );
//     assert_eq!(
//         witnesses[0].len(),
//         2,
//         "Witness stack should contain signature and pubkey"
//     );
//     assert!(
//         segwit_tx
//             .vin
//             .iter()
//             .all(|input| input.script_sig.is_empty()),
//         "script_sig should be empty"
//     );
//     assert!(
//         segwit_tx
//             .vout
//             .iter()
//             .all(|output| output.script_pubkey.starts_with("0014")),
//         "Outputs should be P2WPKH"
//     );
//     assert_ne!(
//         segwit_tx.hash, segwit_tx.txid,
//         "Hash should differ from TXID for SegWit"
//     );

//     // Test 2: P2SH Multisig transaction
//     let pubkeys = vec![wallet.public_key.clone(), wallet2.public_key.clone()];
//     let multisig_tx =
//         Transaction::create_new_multisig_txn(&wallet, &utxo_set, 2, pubkeys.clone(), 5000, 1000)
//             .expect("Failed to create multisig transaction");
//     let redeem_script = Transaction::create_multisig_redeem_script(2, &pubkeys);
//     let redeem_scripts = Some(vec![Some(redeem_script.clone()), None]);
//     let segwit_multisig_tx = multisig_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, redeem_scripts)
//         .expect("Failed to modify multisig transaction to SegWit");

//     assert!(
//         segwit_multisig_tx.witnesses.is_some(),
//         "Witnesses should be present"
//     );
//     let multisig_witnesses = segwit_multisig_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         multisig_witnesses.len(),
//         multisig_tx.vin.len(),
//         "Witness count should match input count for multisig"
//     );
//     assert_eq!(
//         segwit_multisig_tx.vout[0].script_pubkey.starts_with("0020"),
//         true,
//         "First output should be P2WSH"
//     );
//     if segwit_multisig_tx.vout.len() > 1 {
//         assert_eq!(
//             segwit_multisig_tx.vout[1].script_pubkey.starts_with("0014"),
//             true,
//             "Change output should be P2WPKH"
//         );
//     }

//     // Test 3: P2SH CLTV transaction
//     let cltv_tx = Transaction::create_new_timelocked_cltv_txn(
//         &wallet,
//         &utxo_set,
//         90000,
//         1000,
//         500000,
//         &recipient_address,
//     )
//     .expect("Failed to create CLTV transaction");
//     let cltv_redeem_script =
//         Transaction::create_cltv_redeem_script(500000, &wallet2.public_key_hash); // Use recipient's pubkey_hash
//     let redeem_script_bytes = hex::decode(&cltv_redeem_script).expect("Invalid redeem script hex");
//     let calculated_hash = ChainUtil::hash160(&redeem_script_bytes);
//     let script_pubkey = cltv_tx.vout[0].script_pubkey.clone();
//     let expected_hash = &script_pubkey[4..44];
//     println!(
//         "Redeem script: {}, Calculated hash: {}, Expected hash: {}",
//         cltv_redeem_script, calculated_hash, expected_hash
//     );
//     assert_eq!(
//         calculated_hash, expected_hash,
//         "Redeem script hash must match P2SH script hash"
//     );

//     let cltv_redeem_scripts = Some(vec![Some(cltv_redeem_script), None]);
//     let segwit_cltv_tx = cltv_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, cltv_redeem_scripts)
//         .expect("Failed to modify CLTV transaction to SegWit");

//     assert!(
//         segwit_cltv_tx.witnesses.is_some(),
//         "Witnesses should be present"
//     );
//     let cltv_witnesses = segwit_cltv_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         cltv_witnesses.len(),
//         cltv_tx.vin.len(),
//         "Witness count should match input count for CLTV"
//     );
//     assert_eq!(
//         segwit_cltv_tx.vout[0].script_pubkey.starts_with("0020"),
//         true,
//         "CLTV output should be P2WSH"
//     );
//     if segwit_cltv_tx.vout.len() > 1 {
//         assert_eq!(
//             segwit_cltv_tx.vout[1].script_pubkey.starts_with("0014"),
//             true,
//             "Change output should be P2WPKH"
//         );
//     }

//     // ... (remaining tests unchanged)
// }
// #[test]
// fn test_utxo_set() {
//     let mut utxo_set = UtxoSet::new();
//     let wallet = Wallet::new();
//     let tx = Transaction::new_coinbase(
//         1,
//         wallet.address.clone(),
//         123,
//         5000000000,
//         100000,
//         "nonce",
//         "tag",
//     );

//     let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
//     utxo_set.add_utxo(tx.txid.clone(), 0, utxo.clone());

//     assert!(utxo_set.has_utxo(&tx.txid, 0));
//     assert_eq!(
//         utxo_set.get_utxo(&tx.txid, 0).unwrap().out.value,
//         5000100000
//     );
//     assert_eq!(utxo_set.total_value(), 5000100000);

//     let removed = utxo_set.remove_utxo(&tx.txid, 0).unwrap();
//     assert_eq!(removed.out.value, 5000100000);
//     assert!(!utxo_set.has_utxo(&tx.txid, 0));
//     assert_eq!(utxo_set.total_value(), 0);

//     assert!(Utxo::extract_utxo(&tx, 1, 123456).is_none());
// }

// #[test]
// fn test_utxo_extraction_edge_cases() {
//     let tx = Transaction::new(1, 0, vec![], vec![], None);
//     assert!(
//         Utxo::extract_utxo(&tx, 0, 123456).is_none(),
//         "Should return None for empty outputs"
//     );

//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let tx = Transaction::new(
//         1,
//         0,
//         inputs,
//         vec![sample_output(1000000, &wallet.public_key_hash, false)],
//         None,
//     );
//     let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
//     assert!(
//         !utxo.f_coinbase,
//         "Non-coinbase transaction should have f_coinbase false"
//     );
// }

// #[test]
// fn test_serialization() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "")];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, true)];
//     let dummy_data = vec![0u8; 32];
//     let sig = wallet.sign_data(&dummy_data);
//     let witnesses = Some(vec![vec![sig, wallet.public_key.clone()]]);
//     let tx = Transaction::new(1, 0, inputs, outputs, witnesses);

//     let serialized = tx.serialize_non_witness();
//     assert!(
//         !serialized.is_empty(),
//         "Serialized transaction should not be empty"
//     );
// }

// #[test]
// fn test_display() {
//     let wallet = Wallet::new();
//     let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
//     let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
//     let tx = Transaction::new(1, 0, inputs, outputs, None);
//     let display = format!("{}", tx);
//     assert!(display.contains("txid"));
//     assert!(display.contains("hash"));
//     assert!(display.contains("version"));
//     assert!(display.contains("locktime"));
//     assert!(display.contains("inputs"));
//     assert!(display.contains("outputs"));
//     assert!(display.contains("witnesses: none"));
// }

// #[test]
// fn test_create_new_transaction() {
//     let wallet = Wallet::new();
//     let recipient = Wallet::new();
//     let mut utxo_set = UtxoSet::new();
//     let utxo_value = 1500000;
//     let value = 1000000;
//     let fee = 1000;

//     // Setup UTXO
//     let utxo = Utxo::new(
//         TxOutput {
//             value: utxo_value,
//             script_pubkey: Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     let txid = "1234".repeat(16);
//     utxo_set.add_utxo(txid.clone(), 0, utxo);

//     // Create and sign transaction
//     let mut tx =
//         Transaction::create_new_transaction(&wallet, &utxo_set, value, fee, &recipient.address)
//             .expect("Failed to create transaction");
//     tx = tx.clone().sign_transaction(&wallet, &mut tx, &utxo_set);

//     // Setup TransactionPool and validate
//     let mut pool = TransactionPool::new();
//     pool.utxo_set = utxo_set.clone();
//     let result = pool.validate_transaction(&tx, DEFAULT_MIN_FEE_RATE as f64, &utxo_set);
//     assert!(
//         result.is_ok(),
//         "Transaction validation failed: {:?}",
//         result.err()
//     );

//     // Assertions
//     assert_eq!(tx.vin.len(), 1);
//     assert_eq!(tx.vout.len(), 2, "Should include change output");
//     assert_eq!(tx.vout[0].value, value);
//     assert_eq!(
//         tx.vout[1].value,
//         utxo_value - value - fee,
//         "Change should account for fee"
//     );
//     assert_eq!(
//         tx.vout[0].script_pubkey,
//         Transaction::create_p2pkh_script(&recipient.public_key_hash)
//     );
//     assert_eq!(
//         tx.vout[1].script_pubkey,
//         Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         "Change output should use sender's script"
//     );
//     assert!(!tx.txid.is_empty());
//     assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
//     assert!(tx.witnesses.is_none(), "Witnesses should be None");
// }

// #[test]
// fn test_create_new_multisig_txn() {
//     let wallet = Wallet::new();
//     let mut utxo_set = UtxoSet::new();
//     let utxo_value = 1500000;
//     let value = 5000;
//     let fee = 1000;

//     // Setup UTXO
//     let utxo = Utxo::new(
//         TxOutput {
//             value: utxo_value,
//             script_pubkey: Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     let txid = "1234".repeat(16);
//     utxo_set.add_utxo(txid.clone(), 0, utxo);

//     // Create and sign multisig transaction
//     let pubkeys = vec![wallet.public_key.clone(), Wallet::new().public_key];
//     let mut multisig_tx =
//         Transaction::create_new_multisig_txn(&wallet, &utxo_set, 2, pubkeys.clone(), value, fee)
//             .expect("Failed to create multisig transaction");
//     multisig_tx = multisig_tx
//         .clone()
//         .sign_transaction(&wallet, &mut multisig_tx, &utxo_set);

//     // Setup TransactionPool and validate
//     let mut pool = TransactionPool::new();
//     pool.utxo_set = utxo_set.clone();
//     let result = pool.validate_transaction(&multisig_tx, DEFAULT_MIN_FEE_RATE as f64, &utxo_set);
//     assert!(
//         result.is_ok(),
//         "Multisig transaction validation failed: {:?}",
//         result.err()
//     );

//     // Assertions
//     assert_eq!(multisig_tx.vin.len(), 1);
//     assert_eq!(multisig_tx.vout.len(), 2, "Should include change output");
//     assert_eq!(multisig_tx.vout[0].value, value);
//     assert!(multisig_tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(multisig_tx.vout[0].script_pubkey.ends_with("87"));
//     if multisig_tx.vout.len() > 1 {
//         assert_eq!(
//             multisig_tx.vout[1].script_pubkey,
//             Transaction::create_p2pkh_script(&wallet.public_key_hash),
//             "Change output should use sender's script"
//         );
//         assert_eq!(
//             multisig_tx.vout[1].value,
//             utxo_value - value - fee,
//             "Change should account for fee"
//         );
//     }
//     assert!(!multisig_tx.txid.is_empty());
//     assert_eq!(
//         multisig_tx.hash, multisig_tx.txid,
//         "Hash should equal TXID for non-SegWit"
//     );
//     assert!(multisig_tx.witnesses.is_none(), "Witnesses should be None");
// }

// #[test]
// fn test_create_new_timelocked_cltv_txn() {
//     let wallet = Wallet::new();
//     let recipient = Wallet::new();
//     let mut utxo_set = UtxoSet::new();
//     let utxo_value = 1500000;
//     let value = 90000;
//     let fee = 1000;
//     let cltv_lock_time = 500000;

//     // Setup UTXO
//     let utxo = Utxo::new(
//         TxOutput {
//             value: utxo_value,
//             script_pubkey: Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     let txid = "1234".repeat(16);
//     utxo_set.add_utxo(txid.clone(), 0, utxo);

//     // Create and sign CLTV transaction
//     let mut cltv_tx = Transaction::create_new_timelocked_cltv_txn(
//         &wallet,
//         &utxo_set,
//         value,
//         fee,
//         cltv_lock_time,
//         &recipient.address,
//     )
//     .expect("Failed to create CLTV transaction");
//     cltv_tx = cltv_tx
//         .clone()
//         .sign_transaction(&wallet, &mut cltv_tx, &utxo_set);

//     // Setup TransactionPool and validate
//     let mut pool = TransactionPool::new();
//     pool.utxo_set = utxo_set.clone();
//     let result = pool.validate_transaction(&cltv_tx, DEFAULT_MIN_FEE_RATE as f64, &utxo_set);
//     assert!(
//         result.is_ok(),
//         "CLTV transaction validation failed: {:?}",
//         result.err()
//     );

//     // Assertions
//     assert_eq!(cltv_tx.vin.len(), 1);
//     assert_eq!(cltv_tx.vout.len(), 2, "Should include change output");
//     assert_eq!(cltv_tx.vout[0].value, value);
//     assert!(cltv_tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(cltv_tx.vout[0].script_pubkey.ends_with("87"));
//     assert_eq!(cltv_tx.locktime, cltv_lock_time);
//     if cltv_tx.vout.len() > 1 {
//         assert_eq!(
//             cltv_tx.vout[1].script_pubkey,
//             Transaction::create_p2pkh_script(&wallet.public_key_hash),
//             "Change output should use sender's script"
//         );
//         assert_eq!(
//             cltv_tx.vout[1].value,
//             utxo_value - value - fee,
//             "Change should account for fee"
//         );
//     }
//     assert!(!cltv_tx.txid.is_empty());
//     assert_eq!(
//         cltv_tx.hash, cltv_tx.txid,
//         "Hash should equal TXID for non-SegWit"
//     );
//     assert!(cltv_tx.witnesses.is_none(), "Witnesses should be None");
// }

// #[test]
// fn test_create_new_timelocked_csv_txn() {
//     let wallet = Wallet::new();
//     let recipient = Wallet::new();
//     let mut utxo_set = UtxoSet::new();
//     let utxo_value = 1500000;
//     let value = 90000;
//     let fee = 1000;
//     let csv_lock_blocks = 100;

//     // Setup UTXO
//     let utxo = Utxo::new(
//         TxOutput {
//             value: utxo_value,
//             script_pubkey: Transaction::create_p2pkh_script(&wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     let txid = "1234".repeat(16);
//     utxo_set.add_utxo(txid.clone(), 0, utxo);

//     // Create and sign CSV transaction
//     let mut csv_tx = Transaction::create_new_timelocked_csv_txn(
//         &wallet,
//         &utxo_set,
//         value,
//         fee,
//         csv_lock_blocks,
//         &recipient.address,
//     )
//     .expect("Failed to create CSV transaction");
//     csv_tx = csv_tx
//         .clone()
//         .sign_transaction(&wallet, &mut csv_tx, &utxo_set);

//     // Setup TransactionPool and validate
//     let mut pool = TransactionPool::new();
//     pool.utxo_set = utxo_set.clone();
//     let result = pool.validate_transaction(&csv_tx, DEFAULT_MIN_FEE_RATE as f64, &utxo_set);
//     assert!(
//         result.is_ok(),
//         "CSV transaction validation failed: {:?}",
//         result.err()
//     );

//     // Assertions
//     assert_eq!(csv_tx.vin.len(), 1);
//     assert_eq!(
//         csv_tx.vin[0].sequence, csv_lock_blocks,
//         "Sequence should be set to csv_lock_blocks"
//     );
//     assert_eq!(csv_tx.vout.len(), 2, "Should include change output");
//     assert_eq!(csv_tx.vout[0].value, value);
//     assert!(csv_tx.vout[0].script_pubkey.starts_with("a914"));
//     assert!(csv_tx.vout[0].script_pubkey.ends_with("87"));
//     if csv_tx.vout.len() > 1 {
//         assert_eq!(
//             csv_tx.vout[1].script_pubkey,
//             Transaction::create_p2pkh_script(&wallet.public_key_hash),
//             "Change output should use sender's script"
//         );
//         assert_eq!(
//             csv_tx.vout[1].value,
//             utxo_value - value - fee,
//             "Change should account for fee"
//         );
//     }
//     assert!(!csv_tx.txid.is_empty());
//     assert_eq!(
//         csv_tx.hash, csv_tx.txid,
//         "Hash should equal TXID for non-SegWit"
//     );
//     assert!(csv_tx.witnesses.is_none(), "Witnesses should be None");
// }

// #[test]
// fn test_modify_txn_to_add_segwit() {
//     let wallet = Wallet::new();
//     let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
//     let mut utxo_set = UtxoSet::new();

//     // UTXO for the main test case
//     let utxo1 = Utxo::new(
//         TxOutput {
//             value: 150000,
//             script_pubkey: sender_script.clone(),
//         },
//         100,
//         false,
//     );
//     utxo_set.add_utxo("1234".repeat(16), 0, utxo1);

//     let recipient_address = ChainUtil::address_from_pubkey_hash(&wallet.public_key_hash)
//         .expect("Failed to create recipient address");

//     // Test 1: Regular P2PKH transaction
//     let p2pkh_tx =
//         Transaction::create_new_transaction(&wallet, &utxo_set, 90000, 1000, &recipient_address)
//             .expect("Failed to create P2PKH transaction");
//     let mut segwit_tx = p2pkh_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, None)
//         .expect("Failed to modify P2PKH transaction to SegWit");
//     segwit_tx = segwit_tx
//         .clone()
//         .sign_transaction(&wallet, &mut segwit_tx, &utxo_set);

//     assert!(segwit_tx.witnesses.is_some(), "Witnesses should be present");
//     let witnesses = segwit_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         witnesses.len(),
//         p2pkh_tx.vin.len(),
//         "Witness count should match input count"
//     );
//     assert!(
//         !witnesses[0].is_empty(),
//         "Witness stack should contain data"
//     );
//     assert_eq!(
//         witnesses[0].len(),
//         2,
//         "Witness stack should contain signature and pubkey"
//     );
//     assert!(
//         segwit_tx
//             .vin
//             .iter()
//             .all(|input| input.script_sig.is_empty()),
//         "script_sig should be empty"
//     );
//     assert!(
//         segwit_tx
//             .vout
//             .iter()
//             .all(|output| output.script_pubkey.starts_with("0014")),
//         "Outputs should be P2WPKH"
//     );
//     assert_ne!(
//         segwit_tx.hash, segwit_tx.txid,
//         "Hash should differ from TXID for SegWit"
//     );

//     // Test 2: P2SH Multisig transaction
//     let pubkeys = vec![wallet.public_key.clone(), Wallet::new().public_key];
//     let multisig_tx =
//         Transaction::create_new_multisig_txn(&wallet, &utxo_set, 2, pubkeys.clone(), 5000, 1000)
//             .expect("Failed to create multisig transaction");
//     let redeem_script = Transaction::create_multisig_redeem_script(2, &pubkeys);
//     let redeem_scripts = Some(vec![Some(redeem_script.clone()), None]);
//     let mut segwit_multisig_tx = multisig_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, redeem_scripts)
//         .expect("Failed to modify multisig transaction to SegWit");
//     segwit_multisig_tx =
//         segwit_multisig_tx
//             .clone()
//             .sign_transaction(&wallet, &mut segwit_multisig_tx, &utxo_set);

//     assert!(
//         segwit_multisig_tx.witnesses.is_some(),
//         "Witnesses should be present"
//     );
//     let multisig_witnesses = segwit_multisig_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         multisig_witnesses.len(),
//         multisig_tx.vin.len(),
//         "Witness count should match input count for multisig"
//     );
//     assert_eq!(
//         segwit_multisig_tx.vout[0].script_pubkey.starts_with("0020"),
//         true,
//         "First output should be P2WSH"
//     );
//     if segwit_multisig_tx.vout.len() > 1 {
//         assert_eq!(
//             segwit_multisig_tx.vout[1].script_pubkey.starts_with("0014"),
//             true,
//             "Change output should be P2WPKH"
//         );
//     }

//     // Test 3: P2SH CLTV transaction
//     let cltv_tx = Transaction::create_new_timelocked_cltv_txn(
//         &wallet,
//         &utxo_set,
//         90000,
//         1000,
//         500000,
//         &recipient_address,
//     )
//     .expect("Failed to create CLTV transaction");
//     let cltv_redeem_script =
//         Transaction::create_cltv_redeem_script(500000, &wallet.public_key_hash);
//     let cltv_redeem_scripts = Some(vec![Some(cltv_redeem_script.clone()), None]);
//     let mut segwit_cltv_tx = cltv_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, cltv_redeem_scripts)
//         .expect("Failed to modify CLTV transaction to SegWit");
//     segwit_cltv_tx =
//         segwit_cltv_tx
//             .clone()
//             .sign_transaction(&wallet, &mut segwit_cltv_tx, &utxo_set);

//     assert!(
//         segwit_cltv_tx.witnesses.is_some(),
//         "Witnesses should be present"
//     );
//     let cltv_witnesses = segwit_cltv_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         cltv_witnesses.len(),
//         cltv_tx.vin.len(),
//         "Witness count should match input count for CLTV"
//     );
//     assert_eq!(
//         segwit_cltv_tx.vout[0].script_pubkey.starts_with("0020"),
//         true,
//         "CLTV output should be P2WSH"
//     );
//     if segwit_cltv_tx.vout.len() > 1 {
//         assert_eq!(
//             segwit_cltv_tx.vout[1].script_pubkey.starts_with("0014"),
//             true,
//             "Change output should be P2WPKH"
//         );
//     }

//     // Test 4: P2SH CSV transaction
//     let csv_tx = Transaction::create_new_timelocked_csv_txn(
//         &wallet,
//         &utxo_set,
//         90000,
//         1000,
//         100,
//         &recipient_address,
//     )
//     .expect("Failed to create CSV transaction");
//     let csv_redeem_script = Transaction::create_csv_redeem_script(100, &wallet.public_key_hash);
//     let csv_redeem_scripts = Some(vec![Some(csv_redeem_script.clone()), None]);
//     let mut segwit_csv_tx = csv_tx
//         .modify_txn_to_add_segwit(&wallet, &utxo_set, csv_redeem_scripts)
//         .expect("Failed to modify CSV transaction to SegWit");
//     segwit_csv_tx = segwit_csv_tx
//         .clone()
//         .sign_transaction(&wallet, &mut segwit_csv_tx, &utxo_set);

//     assert!(
//         segwit_csv_tx.witnesses.is_some(),
//         "Witnesses should be present"
//     );
//     let csv_witnesses = segwit_csv_tx.witnesses.clone().unwrap();
//     assert_eq!(
//         csv_witnesses.len(),
//         csv_tx.vin.len(),
//         "Witness count should match input count for CSV"
//     );
//     assert_eq!(
//         segwit_csv_tx.vout[0].script_pubkey.starts_with("0020"),
//         true,
//         "CSV output should be P2WSH"
//     );
//     if segwit_csv_tx.vout.len() > 1 {
//         assert_eq!(
//             segwit_csv_tx.vout[1].script_pubkey.starts_with("0014"),
//             true,
//             "Change output should be P2WPKH"
//         );
//     }

//     // Test 5: Already SegWit transaction
//     let result = segwit_tx.modify_txn_to_add_segwit(&wallet, &utxo_set, None);
//     match result {
//         Ok(_) => panic!("Should fail for already SegWit transaction"),
//         Err(e) => assert_eq!(e, "Transaction already contains witness data"),
//     }

//     // Test 6: Wrong wallet
//     let mut wrong_utxo_set = UtxoSet::new();
//     let wrong_wallet = Wallet::new();
//     let wrong_utxo = Utxo::new(
//         TxOutput {
//             value: 150000,
//             script_pubkey: Transaction::create_p2pkh_script(&wrong_wallet.public_key_hash),
//         },
//         100,
//         false,
//     );
//     wrong_utxo_set.add_utxo("1234".repeat(16), 0, wrong_utxo);
//     let result = p2pkh_tx.modify_txn_to_add_segwit(&wallet, &wrong_utxo_set, None);
//     match result {
//         Ok(_) => panic!("Should fail for unowned input"),
//         Err(e) => assert_eq!(e, "Input not owned by wallet"),
//     }

//     // Test 7: Missing UTXO
//     let empty_utxo_set = UtxoSet::new();
//     let result = p2pkh_tx.modify_txn_to_add_segwit(&wallet, &empty_utxo_set, None);
//     match result {
//         Ok(_) => panic!("Should fail for missing UTXO"),
//         Err(e) => assert!(e.contains("UTXO not found")),
//     }

//     // Test 8: Invalid redeem script
//     let invalid_redeem_script = "invalid".to_string();
//     let result = multisig_tx.modify_txn_to_add_segwit(
//         &wallet,
//         &utxo_set,
//         Some(vec![Some(invalid_redeem_script), None]),
//     );
//     match result {
//         Ok(_) => panic!("Should fail for invalid redeem script"),
//         Err(e) => assert!(e.contains("Invalid redeem script hex")),
//     }

//     // Test 9: Mismatched redeem script
//     let wrong_redeem_script = Transaction::create_multisig_redeem_script(1, &pubkeys);
//     let result = multisig_tx.modify_txn_to_add_segwit(
//         &wallet,
//         &utxo_set,
//         Some(vec![Some(wrong_redeem_script), None]),
//     );
//     match result {
//         Ok(_) => panic!("Should fail for mismatched redeem script"),
//         Err(e) => assert_eq!(e, "Redeem script does not match script hash"),
//     }

//     // Test 10: Incorrect redeem scripts length
//     let result = multisig_tx.modify_txn_to_add_segwit(&wallet, &utxo_set, Some(vec![]));
//     match result {
//         Ok(_) => panic!("Should fail for incorrect redeem scripts length"),
//         Err(e) => assert_eq!(e, "Redeem scripts length does not match outputs"),
//     }
// }
