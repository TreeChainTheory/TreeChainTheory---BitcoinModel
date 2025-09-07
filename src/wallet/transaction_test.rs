use crate::chain_util::ChainUtil;
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;

fn sample_input(txid: &str, vout: u32, script_sig: &str) -> TxInput {
    TxInput {
        txid: txid.to_string(),
        vout,
        script_sig: hex::encode(script_sig),
        sequence: 0xffffffff,
    }
}

fn sample_output(value: u64, pubkey_hash: &str, is_p2wpkh: bool) -> TxOutput {
    TxOutput {
        value,
        script_pubkey: if is_p2wpkh {
            format!("0014{}", pubkey_hash)
        } else {
            format!("76a914{}88ac", pubkey_hash)
        },
    }
}

#[test]
fn test_new_transaction() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
    let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);

    assert_eq!(tx.version, 1);
    assert_eq!(tx.locktime, 0);
    assert_eq!(tx.vin, inputs);
    assert_eq!(tx.vout, outputs);
    assert!(!tx.txid.is_empty(), "TXID should be computed");
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");
    assert_eq!(
        tx.vout[0].script_pubkey,
        format!("76a914{}88ac", wallet.public_key_hash)
    );
}

#[test]
fn test_new_coinbase() {
    let wallet = Wallet::new();
    let tx = Transaction::new_coinbase(
        1,
        wallet.address.clone(),
        123,
        5000000000,
        100000,
        "nonce",
        "tag",
    );

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vin[0].txid, "00".repeat(32));
    assert_eq!(tx.vin[0].vout, u32::MAX);
    assert_eq!(tx.vout[0].value, 5000100000);
    assert_eq!(tx.locktime, 0);
    assert!(!tx.txid.is_empty(), "TXID should be computed");
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");
    assert_eq!(
        tx.vout[0].script_pubkey,
        format!("76a914{}88ac", wallet.public_key_hash)
    );

    let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
    assert_eq!(utxo.out.value, 5000100000);
    assert_eq!(utxo.queue_index, 123456);
    assert!(utxo.f_coinbase);
}

#[test]
fn test_create_normal_txn() {
    let wallet1 = Wallet::new();
    let wallet2 = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let recipients = vec![
        (1000000, wallet1.address.clone()),
        (2000000, wallet2.address.clone()),
    ];
    let tx = Transaction::create_normal_txn(1, inputs.clone(), recipients, 0);

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 2);
    assert_eq!(tx.vout[0].value, 1000000);
    assert_eq!(tx.vout[1].value, 2000000);
    assert!(!tx.txid.is_empty());
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");
    assert_eq!(
        tx.vout[0].script_pubkey,
        format!("76a914{}88ac", wallet1.public_key_hash)
    );
    assert_eq!(
        tx.vout[1].script_pubkey,
        format!("76a914{}88ac", wallet2.public_key_hash)
    );
}

#[test]
fn test_create_multisig_txn() {
    let wallet1 = Wallet::new();
    let wallet2 = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let pubkeys = vec![wallet1.public_key.clone(), wallet2.public_key.clone()];
    let tx = Transaction::create_multisig_txn(1, inputs.clone(), 2, pubkeys.clone(), 1000000, 0);

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vout[0].value, 1000000);
    assert!(tx.vout[0].script_pubkey.starts_with("a914"));
    assert!(tx.vout[0].script_pubkey.ends_with("87"));
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");

    let result = std::panic::catch_unwind(|| {
        Transaction::create_multisig_txn(1, inputs, 3, pubkeys, 1000000, 0)
    });
    assert!(result.is_err(), "Should panic when m > n");
}

#[test]
fn test_create_timelocked_txn() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let tx = Transaction::create_timelocked_txn(
        1,
        inputs.clone(),
        500000,
        wallet.address.clone(),
        1000000,
        500000,
    );

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vout[0].value, 1000000);
    assert!(tx.vout[0].script_pubkey.starts_with("a914"));
    assert!(tx.vout[0].script_pubkey.ends_with("87"));
    assert_eq!(tx.locktime, 500000);
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");
}

#[test]
fn test_create_timelocked_csv_txn() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let tx = Transaction::create_timelocked_csv_txn(
        1,
        inputs.clone(),
        100,
        wallet.address.clone(),
        1000000,
        0,
    );

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(
        tx.vin[0].sequence, 100,
        "Sequence should be set to csv_lock_blocks"
    );
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vout[0].value, 1000000);
    assert!(tx.vout[0].script_pubkey.starts_with("a914"));
    assert!(tx.vout[0].script_pubkey.ends_with("87"));
    assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
    assert!(tx.witnesses.is_none(), "Witnesses should be None");
}

#[test]
fn test_compute_txid_and_hash() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
    let tx1 = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);
    let tx2 = Transaction::new(1, 0, inputs, outputs.clone(), None);

    assert_eq!(
        tx1.txid, tx2.txid,
        "Identical non-SegWit transactions should have same TXID"
    );
    assert_eq!(
        tx1.hash, tx2.hash,
        "Identical non-SegWit transactions should have same hash"
    );
    assert_eq!(tx1.hash, tx1.txid, "Hash should equal TXID for non-SegWit");

    let different_inputs = vec![sample_input(&"5678".repeat(16), 0, "signature+pubkey")];
    let tx3 = Transaction::new(1, 0, different_inputs, outputs.clone(), None);
    assert_ne!(
        tx1.txid, tx3.txid,
        "Different inputs should yield different TXID"
    );
    assert_ne!(
        tx1.hash, tx3.hash,
        "Different inputs should yield different hash"
    );
}

#[test]
fn test_get_size_vsize_weight() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let outputs = vec![sample_output(1000000, &wallet.public_key_hash, false)];
    let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone(), None);

    let (size, vsize, weight) = tx.get_size_vsize_weight();
    assert!(size > 0, "Size should be positive");
    assert_eq!(vsize, size, "vSize should equal size for non-SegWit");
    assert_eq!(weight, size * 4, "Weight should be size * 4 for non-SegWit");

    let witnesses = Some(vec![vec![
        hex::encode("signature"),
        wallet.public_key.clone(),
    ]]);
    let segwit_tx = Transaction::new(1, 0, inputs, outputs, witnesses);
    let (segwit_size, segwit_vsize, segwit_weight) = segwit_tx.get_size_vsize_weight();
    assert!(
        segwit_size > size,
        "SegWit size should include witness data"
    );
    assert!(
        segwit_vsize < segwit_size,
        "vSize should be less than size for SegWit"
    );
    let expected_weight = segwit_vsize * 4;
    assert!(
        (segwit_weight as i32 - expected_weight as i32).abs() <= 1,
        "Weight calculation for SegWit: expected ~{}, got {}",
        expected_weight,
        segwit_weight
    );
}

#[test]
fn test_create_new_transaction() {
    let wallet = Wallet::new();
    let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
    let mut utxo_set = UtxoSet::new();
    let pool = TransactionPool::new();

    // UTXOs for the main test case
    let utxo1 = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: sender_script.clone(),
        },
        100,
        false,
    );
    let utxo2 = Utxo::new(
        TxOutput {
            value: 25000,
            script_pubkey: sender_script.clone(),
        },
        101,
        false,
    );
    let utxo3 = Utxo::new(
        TxOutput {
            value: 12500,
            script_pubkey: sender_script.clone(),
        },
        102,
        false,
    );

    utxo_set.add_utxo("1234".repeat(16), 0, utxo1);
    utxo_set.add_utxo("5678".repeat(16), 0, utxo2);
    utxo_set.add_utxo("9abc".repeat(16), 0, utxo3);

    let recipient_address = ChainUtil::address_from_pubkey_hash(&wallet.public_key_hash)
        .expect("Failed to create recipient address");

    // Test case: Successful transaction with value = 100000
    let result =
        Transaction::create_new_transaction(&wallet, &utxo_set, &pool, 100000, &recipient_address);
    match result {
        Ok(tx) => {
            assert_eq!(tx.version, 1, "Version should be 1");
            assert_eq!(
                tx.vout.len(),
                2,
                "Should have 2 outputs (recipient + change)"
            );
            assert_eq!(
                tx.vout[0].value, 100000,
                "Recipient output should be 100000"
            );
            assert!(
                tx.vout[1].value <= 87500,
                "Change output should account for fee"
            );
            assert!(tx.vin.len() >= 1, "Should use at least 1 input");
            assert!(
                !tx.vin[0].script_sig.is_empty(),
                "Input 0 should have script_sig"
            );
            assert!(tx.witnesses.is_none(), "Witnesses should be None");
            assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
        }
        Err(e) => panic!("Transaction creation failed: {}", e),
    }

    // Test case: Insufficient funds
    let result =
        Transaction::create_new_transaction(&wallet, &utxo_set, &pool, 200000, &recipient_address);
    match result {
        Ok(_) => panic!("Transaction should have failed due to insufficient funds"),
        Err(e) => assert!(
            e.contains("Insufficient funds"),
            "Expected insufficient funds error"
        ),
    }

    // Test case: No UTXOs
    let empty_utxo_set = UtxoSet::new();
    let result = Transaction::create_new_transaction(
        &wallet,
        &empty_utxo_set,
        &pool,
        1000,
        &recipient_address,
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to no UTXOs"),
        Err(e) => assert_eq!(
            e, "No unspent UTXOs found for wallet",
            "Expected no UTXOs error"
        ),
    }

    // Test case: Small transaction with sufficient funds
    let mut small_utxo_set = UtxoSet::new();
    let small_utxo = Utxo::new(
        TxOutput {
            value: 15000,
            script_pubkey: sender_script,
        },
        100,
        false,
    );
    small_utxo_set.add_utxo("abcd".repeat(16), 0, small_utxo);
    let result = Transaction::create_new_transaction(
        &wallet,
        &small_utxo_set,
        &pool,
        9000,
        &recipient_address,
    );
    match result {
        Ok(tx) => {
            assert_eq!(
                tx.vout.len(),
                2,
                "Should have 2 outputs (no change due to dust threshold)"
            );
            assert_eq!(tx.vout[0].value, 9000, "Recipient output should be 9000");
            assert!(tx.witnesses.is_none(), "Witnesses should be None");
        }
        Err(e) => panic!("Transaction creation failed: {}", e),
    }
}

#[test]
fn test_create_new_multisig_txn() {
    let wallet = Wallet::new();
    let wallet2 = Wallet::new();
    let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
    let mut utxo_set = UtxoSet::new();
    let pool = TransactionPool::new();

    // UTXOs for the main test case
    let utxo1 = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: sender_script.clone(),
        },
        100,
        false,
    );
    let utxo2 = Utxo::new(
        TxOutput {
            value: 25000,
            script_pubkey: sender_script.clone(),
        },
        101,
        false,
    );
    utxo_set.add_utxo("1234".repeat(16), 0, utxo1);
    utxo_set.add_utxo("5678".repeat(16), 0, utxo2);

    let pubkeys = vec![wallet.public_key.clone(), wallet2.public_key.clone()];
    let m = 2;

    // Test case: Successful multisig transaction
    let result =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set, &pool, 100000, m, pubkeys.clone());
    match result {
        Ok(tx) => {
            assert_eq!(tx.version, 1, "Version should be 1");
            assert_eq!(
                tx.vout.len(),
                2,
                "Should have 2 outputs (multisig + change)"
            );
            assert_eq!(tx.vout[0].value, 100000, "Multisig output should be 100000");
            assert!(
                tx.vout[0].script_pubkey.starts_with("a914"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[0].script_pubkey.ends_with("87"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[1].value <= 75000,
                "Change output should account for fee"
            );
            assert!(tx.vin.len() >= 1, "Should use at least 1 input");
            assert!(
                !tx.vin[0].script_sig.is_empty(),
                "Input 0 should have script_sig"
            );
            assert!(tx.witnesses.is_none(), "Witnesses should be None");
            assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
        }
        Err(e) => panic!("Multisig transaction creation failed: {}", e),
    }

    // Test case: Insufficient funds
    let result =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set, &pool, 200000, m, pubkeys.clone());
    match result {
        Ok(_) => panic!("Transaction should have failed due to insufficient funds"),
        Err(e) => assert!(
            e.contains("Insufficient funds"),
            "Expected insufficient funds error"
        ),
    }

    // Test case: No UTXOs
    let empty_utxo_set = UtxoSet::new();
    let result = Transaction::create_new_multisig_txn(
        &wallet,
        &empty_utxo_set,
        &pool,
        1000,
        m,
        pubkeys.clone(),
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to no UTXOs"),
        Err(e) => assert_eq!(
            e, "No unspent UTXOs found for wallet",
            "Expected no UTXOs error"
        ),
    }

    // Test case: Invalid m value
    let result =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set, &pool, 1000, 3, pubkeys.clone());
    match result {
        Ok(_) => panic!("Transaction should have failed due to invalid m"),
        Err(e) => assert_eq!(
            e, "Invalid m value for multisig",
            "Expected invalid m error"
        ),
    }

    // Test case: Invalid pubkey
    let invalid_pubkeys = vec![wallet.public_key.clone(), "invalid_hex".to_string()];
    let result =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set, &pool, 1000, 2, invalid_pubkeys);
    match result {
        Ok(_) => panic!("Transaction should have failed due to invalid pubkey"),
        Err(e) => assert!(
            e.contains("Invalid pubkey hex"),
            "Expected invalid pubkey error"
        ),
    }
}

#[test]
fn test_create_new_timelocked_cltv_txn() {
    let wallet = Wallet::new();
    let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
    let mut utxo_set = UtxoSet::new();
    let pool = TransactionPool::new();

    // UTXOs for the main test case
    let utxo1 = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: sender_script.clone(),
        },
        100,
        false,
    );
    let utxo2 = Utxo::new(
        TxOutput {
            value: 25000,
            script_pubkey: sender_script.clone(),
        },
        101,
        false,
    );
    utxo_set.add_utxo("1234".repeat(16), 0, utxo1);
    utxo_set.add_utxo("5678".repeat(16), 0, utxo2);

    let recipient_address = ChainUtil::address_from_pubkey_hash(&wallet.public_key_hash)
        .expect("Failed to create recipient address");
    let cltv_lock_time = 500000;

    // Test case: Successful CLTV transaction
    let result = Transaction::create_new_timelocked_cltv_txn(
        &wallet,
        &utxo_set,
        &pool,
        100000,
        cltv_lock_time,
        &recipient_address,
    );
    match result {
        Ok(tx) => {
            assert_eq!(tx.version, 1, "Version should be 1");
            assert_eq!(
                tx.locktime, cltv_lock_time,
                "Locktime should match CLTV lock time"
            );
            assert_eq!(tx.vout.len(), 2, "Should have 2 outputs (CLTV + change)");
            assert_eq!(tx.vout[0].value, 100000, "CLTV output should be 100000");
            assert!(
                tx.vout[0].script_pubkey.starts_with("a914"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[0].script_pubkey.ends_with("87"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[1].value <= 75000,
                "Change output should account for fee"
            );
            assert!(tx.vin.len() >= 1, "Should use at least 1 input");
            assert!(
                !tx.vin[0].script_sig.is_empty(),
                "Input 0 should have script_sig"
            );
            assert!(tx.witnesses.is_none(), "Witnesses should be None");
            assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
        }
        Err(e) => panic!("CLTV transaction creation failed: {}", e),
    }

    // Test case: Insufficient funds
    let result = Transaction::create_new_timelocked_cltv_txn(
        &wallet,
        &utxo_set,
        &pool,
        200000,
        cltv_lock_time,
        &recipient_address,
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to insufficient funds"),
        Err(e) => assert!(
            e.contains("Insufficient funds"),
            "Expected insufficient funds error"
        ),
    }

    // Test case: No UTXOs
    let empty_utxo_set = UtxoSet::new();
    let result = Transaction::create_new_timelocked_cltv_txn(
        &wallet,
        &empty_utxo_set,
        &pool,
        1000,
        cltv_lock_time,
        &recipient_address,
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to no UTXOs"),
        Err(e) => assert_eq!(
            e, "No unspent UTXOs found for wallet",
            "Expected no UTXOs error"
        ),
    }

    // Test case: Invalid recipient address
    let result = Transaction::create_new_timelocked_cltv_txn(
        &wallet,
        &utxo_set,
        &pool,
        1000,
        cltv_lock_time,
        "invalid_address",
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to invalid address"),
        Err(e) => assert_eq!(
            e, "Invalid recipient address",
            "Expected invalid address error"
        ),
    }
}

#[test]
fn test_create_new_timelocked_csv_txn() {
    let wallet = Wallet::new();
    let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
    let mut utxo_set = UtxoSet::new();
    let pool = TransactionPool::new();

    // UTXOs for the main test case
    let utxo1 = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: sender_script.clone(),
        },
        100,
        false,
    );
    let utxo2 = Utxo::new(
        TxOutput {
            value: 25000,
            script_pubkey: sender_script.clone(),
        },
        101,
        false,
    );
    utxo_set.add_utxo("1234".repeat(16), 0, utxo1);
    utxo_set.add_utxo("5678".repeat(16), 0, utxo2);

    let recipient_address = ChainUtil::address_from_pubkey_hash(&wallet.public_key_hash)
        .expect("Failed to create recipient address");
    let csv_lock_blocks = 100;

    // Test case: Successful CSV transaction
    let result = Transaction::create_new_timelocked_csv_txn(
        &wallet,
        &utxo_set,
        &pool,
        100000,
        csv_lock_blocks,
        &recipient_address,
    );
    match result {
        Ok(tx) => {
            assert_eq!(tx.version, 1, "Version should be 1");
            assert_eq!(tx.locktime, 0, "Locktime should be 0");
            assert_eq!(tx.vout.len(), 2, "Should have 2 outputs (CSV + change)");
            assert_eq!(tx.vout[0].value, 100000, "CSV output should be 100000");
            assert!(
                tx.vout[0].script_pubkey.starts_with("a914"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[0].script_pubkey.ends_with("87"),
                "Output should be P2SH"
            );
            assert!(
                tx.vout[1].value <= 75000,
                "Change output should account for fee"
            );
            assert!(tx.vin.len() >= 1, "Should use at least 1 input");
            assert!(
                !tx.vin[0].script_sig.is_empty(),
                "Input 0 should have script_sig"
            );
            assert_eq!(
                tx.vin[0].sequence, csv_lock_blocks,
                "Sequence should match csv_lock_blocks"
            );
            assert!(tx.witnesses.is_none(), "Witnesses should be None");
            assert_eq!(tx.hash, tx.txid, "Hash should equal TXID for non-SegWit");
        }
        Err(e) => panic!("CSV transaction creation failed: {}", e),
    }

    // Test case: Insufficient funds
    let result = Transaction::create_new_timelocked_csv_txn(
        &wallet,
        &utxo_set,
        &pool,
        200000,
        csv_lock_blocks,
        &recipient_address,
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to insufficient funds"),
        Err(e) => assert!(
            e.contains("Insufficient funds"),
            "Expected insufficient funds error"
        ),
    }

    // Test case: No UTXOs
    let empty_utxo_set = UtxoSet::new();
    let result = Transaction::create_new_timelocked_csv_txn(
        &wallet,
        &empty_utxo_set,
        &pool,
        1000,
        csv_lock_blocks,
        &recipient_address,
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to no UTXOs"),
        Err(e) => assert_eq!(
            e, "No unspent UTXOs found for wallet",
            "Expected no UTXOs error"
        ),
    }

    // Test case: Invalid recipient address
    let result = Transaction::create_new_timelocked_csv_txn(
        &wallet,
        &utxo_set,
        &pool,
        1000,
        csv_lock_blocks,
        "invalid_address",
    );
    match result {
        Ok(_) => panic!("Transaction should have failed due to invalid address"),
        Err(e) => assert_eq!(
            e, "Invalid recipient address",
            "Expected invalid address error"
        ),
    }
}

#[test]
fn test_modify_txn_to_add_segwit() {
    let wallet = Wallet::new();
    let sender_script = Transaction::create_p2pkh_script(&wallet.public_key_hash);
    let mut utxo_set = UtxoSet::new();
    let pool = TransactionPool::new();

    // UTXO for the main test case
    let utxo1 = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: sender_script.clone(),
        },
        100,
        false,
    );
    utxo_set.add_utxo("1234".repeat(16), 0, utxo1);

    let recipient_address = ChainUtil::address_from_pubkey_hash(&wallet.public_key_hash)
        .expect("Failed to create recipient address");

    // Test 1: Regular P2PKH transaction
    let p2pkh_tx =
        Transaction::create_new_transaction(&wallet, &utxo_set, &pool, 90000, &recipient_address)
            .expect("Failed to create P2PKH transaction");

    let segwit_tx = p2pkh_tx
        .modify_txn_to_add_segwit(&wallet, &utxo_set, None)
        .expect("Failed to modify P2PKH transaction to SegWit");

    assert!(segwit_tx.witnesses.is_some(), "Witnesses should be present");
    let witnesses = segwit_tx.witnesses.clone().unwrap();
    assert_eq!(
        witnesses.len(),
        p2pkh_tx.vin.len(),
        "Witness count should match input count"
    );
    assert!(
        !witnesses[0].is_empty(),
        "Witness stack should contain data"
    );
    assert_eq!(
        witnesses[0].len(),
        2,
        "Witness stack should contain signature and pubkey"
    );
    assert!(
        segwit_tx
            .vin
            .iter()
            .all(|input| input.script_sig.is_empty()),
        "script_sig should be empty"
    );
    assert!(
        segwit_tx
            .vout
            .iter()
            .all(|output| output.script_pubkey.starts_with("0014")),
        "Outputs should be P2WPKH"
    );
    assert_ne!(
        segwit_tx.hash, segwit_tx.txid,
        "Hash should differ from TXID for SegWit"
    );

    // Test 2: P2SH Multisig transaction
    let pubkeys = vec![wallet.public_key.clone(), Wallet::new().public_key];
    let multisig_tx =
        Transaction::create_new_multisig_txn(&wallet, &utxo_set, &pool, 90000, 2, pubkeys.clone())
            .expect("Failed to create multisig transaction");
    let redeem_script = Transaction::create_multisig_redeem_script(2, &pubkeys);

    // Provide redeem script for multisig output and None for change output
    let redeem_scripts = Some(vec![Some(redeem_script.clone()), None]);
    let segwit_multisig_tx = multisig_tx
        .modify_txn_to_add_segwit(&wallet, &utxo_set, redeem_scripts)
        .expect("Failed to modify multisig transaction to SegWit");

    assert!(
        segwit_multisig_tx.witnesses.is_some(),
        "Witnesses should be present"
    );
    let multisig_witnesses = segwit_multisig_tx.witnesses.clone().unwrap();
    assert_eq!(
        multisig_witnesses.len(),
        multisig_tx.vin.len(),
        "Witness count should match input count for multisig"
    );
    assert_eq!(
        segwit_multisig_tx.vout[0].script_pubkey.starts_with("0020"),
        true,
        "First output should be P2WSH"
    );
    if segwit_multisig_tx.vout.len() > 1 {
        assert_eq!(
            segwit_multisig_tx.vout[1].script_pubkey.starts_with("0014"),
            true,
            "Change output should be P2WPKH"
        );
    }

    // Test 3: P2SH CLTV transaction
    let cltv_tx = Transaction::create_new_timelocked_cltv_txn(
        &wallet,
        &utxo_set,
        &pool,
        90000,
        500000,
        &recipient_address,
    )
    .expect("Failed to create CLTV transaction");
    let cltv_redeem_script =
        Transaction::create_cltv_redeem_script(500000, &wallet.public_key_hash);

    // Provide redeem script for CLTV output and None for change output
    let cltv_redeem_scripts = Some(vec![Some(cltv_redeem_script.clone()), None]);
    let segwit_cltv_tx = cltv_tx
        .modify_txn_to_add_segwit(&wallet, &utxo_set, cltv_redeem_scripts)
        .expect("Failed to modify CLTV transaction to SegWit");

    assert!(
        segwit_cltv_tx.witnesses.is_some(),
        "Witnesses should be present"
    );
    let cltv_witnesses = segwit_cltv_tx.witnesses.clone().unwrap();
    assert_eq!(
        cltv_witnesses.len(),
        cltv_tx.vin.len(),
        "Witness count should match input count for CLTV"
    );
    assert_eq!(
        segwit_cltv_tx.vout[0].script_pubkey.starts_with("0020"),
        true,
        "CLTV output should be P2WSH"
    );
    if segwit_cltv_tx.vout.len() > 1 {
        assert_eq!(
            segwit_cltv_tx.vout[1].script_pubkey.starts_with("0014"),
            true,
            "Change output should be P2WPKH"
        );
    }

    // Test 4: Already SegWit transaction
    let result = segwit_tx.modify_txn_to_add_segwit(&wallet, &utxo_set, None);
    match result {
        Ok(_) => panic!("Should fail for already SegWit transaction"),
        Err(e) => assert_eq!(e, "Transaction already contains witness data"),
    }

    // Test 5: Wrong wallet
    let mut wrong_utxo_set = UtxoSet::new();
    let wrong_wallet = Wallet::new();
    let wrong_utxo = Utxo::new(
        TxOutput {
            value: 150000,
            script_pubkey: Transaction::create_p2pkh_script(&wrong_wallet.public_key_hash),
        },
        100,
        false,
    );
    wrong_utxo_set.add_utxo("1234".repeat(16), 0, wrong_utxo);
    let result = p2pkh_tx.modify_txn_to_add_segwit(&wallet, &wrong_utxo_set, None);
    match result {
        Ok(_) => panic!("Should fail for unowned input"),
        Err(e) => assert_eq!(e, "Input not owned by wallet"),
    }

    // Test 6: Missing UTXO
    let empty_utxo_set = UtxoSet::new();
    let result = p2pkh_tx.modify_txn_to_add_segwit(&wallet, &empty_utxo_set, None);
    match result {
        Ok(_) => panic!("Should fail for missing UTXO"),
        Err(e) => assert!(e.contains("UTXO not found")),
    }

    // Test 7: Invalid redeem script
    let invalid_redeem_script = "invalid".to_string();
    let result = multisig_tx.modify_txn_to_add_segwit(
        &wallet,
        &utxo_set,
        Some(vec![Some(invalid_redeem_script), None]),
    );
    match result {
        Ok(_) => panic!("Should fail for invalid redeem script"),
        Err(e) => assert!(e.contains("Invalid redeem script hex")),
    }

    // Test 8: Mismatched redeem script
    let wrong_redeem_script = Transaction::create_multisig_redeem_script(1, &pubkeys);
    let result = multisig_tx.modify_txn_to_add_segwit(
        &wallet,
        &utxo_set,
        Some(vec![Some(wrong_redeem_script), None]),
    );
    match result {
        Ok(_) => panic!("Should fail for mismatched redeem script"),
        Err(e) => assert_eq!(e, "Redeem script does not match script hash"),
    }

    // Test 9: Incorrect redeem scripts length
    let result = multisig_tx.modify_txn_to_add_segwit(&wallet, &utxo_set, Some(vec![]));
    match result {
        Ok(_) => panic!("Should fail for incorrect redeem scripts length"),
        Err(e) => assert_eq!(e, "Redeem scripts length does not match outputs"),
    }
}

#[test]
fn test_utxo_set() {
    let mut utxo_set = UtxoSet::new();
    let wallet = Wallet::new();
    let tx = Transaction::new_coinbase(
        1,
        wallet.address.clone(),
        123,
        5000000000,
        100000,
        "nonce",
        "tag",
    );

    let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
    utxo_set.add_utxo(tx.txid.clone(), 0, utxo.clone());

    assert!(utxo_set.has_utxo(&tx.txid, 0));
    assert_eq!(
        utxo_set.get_utxo(&tx.txid, 0).unwrap().out.value,
        5000100000
    );
    assert_eq!(utxo_set.total_value(), 5000100000);

    let removed = utxo_set.remove_utxo(&tx.txid, 0).unwrap();
    assert_eq!(removed.out.value, 5000100000);
    assert!(!utxo_set.has_utxo(&tx.txid, 0));
    assert_eq!(utxo_set.total_value(), 0);

    assert!(Utxo::extract_utxo(&tx, 1, 123456).is_none());
}

#[test]
fn test_utxo_extraction_edge_cases() {
    let tx = Transaction::new(1, 0, vec![], vec![], None);
    assert!(
        Utxo::extract_utxo(&tx, 0, 123456).is_none(),
        "Should return None for empty outputs"
    );

    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")];
    let tx = Transaction::new(
        1,
        0,
        inputs,
        vec![sample_output(1000000, &wallet.public_key_hash, false)],
        None,
    );
    let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
    assert!(
        !utxo.f_coinbase,
        "Non-coinbase transaction should have f_coinbase false"
    );
}

#[test]
fn test_display_transaction() {
    let wallet = Wallet::new();
    let tx = Transaction::new(
        1,
        0,
        vec![sample_input(&"1234".repeat(16), 0, "signature+pubkey")],
        vec![sample_output(1000000, &wallet.public_key_hash, false)],
        None,
    );
    let display = format!("{}", tx);
    assert!(display.contains(&tx.txid));
    assert!(display.contains(&tx.hash));
    assert!(display.contains("version: 1"));
    assert!(display.contains("locktime: 0"));
    assert!(display.contains("inputs: 1"));
    assert!(display.contains("outputs: 1"));
    assert!(display.contains("witnesses: none"));
}
