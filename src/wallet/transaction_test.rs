use crate::chain_util::ChainUtil;
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;

// Helper function to create a sample TxInput
fn sample_input(txid: &str, vout: u32) -> TxInput {
    TxInput {
        txid: txid.to_string(),
        vout,
        script_sig: hex::encode("signature+pubkey"),
        sequence: 0xffffffff,
    }
}

// Helper function to create a sample TxOutput
fn sample_output(value: u64, pubkey_hash: &str) -> TxOutput {
    // Directly use the provided public key hash to create a P2PKH script
    TxOutput {
        value,
        script_pubkey: format!("76a914{}88ac", pubkey_hash),
    }
}

#[test]
fn test_new_transaction() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
    let outputs = vec![sample_output(1000000, &wallet.public_key_hash)];
    let tx = Transaction::new(1, 0, inputs.clone(), outputs.clone());

    assert_eq!(tx.version, 1);
    assert_eq!(tx.locktime, 0);
    assert_eq!(tx.vin, inputs);
    assert_eq!(tx.vout, outputs);
    assert!(!tx.txid.is_empty(), "TXID should be computed");
    // Verify the scriptPubKey uses the public key hash
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
        wallet.address.clone(), // Pass address, function extracts pubkey_hash
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
    // Verify the scriptPubKey uses the public key hash
    assert_eq!(
        tx.vout[0].script_pubkey,
        format!("76a914{}88ac", wallet.public_key_hash)
    );

    // Test UTXO extraction from coinbase
    let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();
    assert_eq!(utxo.out.value, 5000100000);
    assert_eq!(utxo.queue_index, 123456);
    assert!(utxo.f_coinbase);
}

#[test]
fn test_create_normal_txn() {
    let wallet1 = Wallet::new();
    let wallet2 = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
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
    // Verify scriptPubKey uses public key hashes
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
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
    let pubkeys = vec![wallet1.public_key.clone(), wallet2.public_key.clone()];
    let tx = Transaction::create_multisig_txn(1, inputs.clone(), 2, pubkeys.clone(), 1000000, 0);

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vout[0].value, 1000000);
    assert!(tx.vout[0].script_pubkey.starts_with("a914"));
    assert!(tx.vout[0].script_pubkey.ends_with("87"));

    // Test invalid m > n
    let result = std::panic::catch_unwind(|| {
        Transaction::create_multisig_txn(1, inputs, 3, pubkeys, 1000000, 0)
    });
    assert!(result.is_err(), "Should panic when m > n");
}

#[test]
fn test_create_timelocked_txn() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
    let tx = Transaction::create_timelocked_txn(
        1,
        inputs.clone(),
        500000,
        wallet.address.clone(),
        1000000,
        0,
    );

    assert_eq!(tx.vin.len(), 1);
    assert_eq!(tx.vout.len(), 1);
    assert_eq!(tx.vout[0].value, 1000000);
    assert!(tx.vout[0].script_pubkey.starts_with("a914"));
    assert!(tx.vout[0].script_pubkey.ends_with("87"));
}

#[test]
fn test_create_timelocked_csv_txn() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
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
}

#[test]
fn test_compute_txid() {
    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
    let outputs = vec![sample_output(1000000, &wallet.public_key_hash)];
    let tx1 = Transaction::new(1, 0, inputs.clone(), outputs.clone());
    let tx2 = Transaction::new(1, 0, inputs, outputs.clone());

    assert_eq!(
        tx1.txid, tx2.txid,
        "Identical transactions should have same TXID"
    );

    // Test with different inputs
    let different_inputs = vec![sample_input(&"5678".repeat(8), 0)];
    let tx3 = Transaction::new(1, 0, different_inputs, outputs.clone());
    assert_ne!(
        tx1.txid, tx3.txid,
        "Different inputs should yield different TXID"
    );
}

#[test]
fn test_utxo_set() {
    let mut utxo_set = UtxoSet::new();

    // Create a sample transaction
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

    // Extract UTXO
    let utxo = Utxo::extract_utxo(&tx, 0, 123456).unwrap();

    // Add to UTXO set
    utxo_set.add_utxo(tx.txid.clone(), 0, utxo.clone());

    // Test UTXO set operations
    assert!(utxo_set.has_utxo(&tx.txid, 0));
    assert_eq!(
        utxo_set.get_utxo(&tx.txid, 0).unwrap().out.value,
        5000100000
    );
    assert_eq!(utxo_set.total_value(), 5000100000);

    // Remove UTXO
    let removed = utxo_set.remove_utxo(&tx.txid, 0).unwrap();
    assert_eq!(removed.out.value, 5000100000);
    assert!(!utxo_set.has_utxo(&tx.txid, 0));
    assert_eq!(utxo_set.total_value(), 0);

    // Test invalid vout
    assert!(Utxo::extract_utxo(&tx, 1, 123456).is_none());
}

#[test]
fn test_utxo_extraction_edge_cases() {
    let tx = Transaction::new(1, 0, vec![], vec![]);
    assert!(
        Utxo::extract_utxo(&tx, 0, 123456).is_none(),
        "Should return None for empty outputs"
    );

    let wallet = Wallet::new();
    let inputs = vec![sample_input(&"1234".repeat(8), 0)];
    let tx = Transaction::new(
        1,
        0,
        inputs,
        vec![sample_output(1000000, &wallet.public_key_hash)],
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
        vec![sample_input(&"1234".repeat(8), 0)],
        vec![sample_output(1000000, &wallet.public_key_hash)],
    );
    let display = format!("{}", tx);
    assert!(display.contains(&tx.txid));
    assert!(display.contains("version: 1"));
    assert!(display.contains("locktime: 0"));
    assert!(display.contains("inputs: 1"));
    assert!(display.contains("outputs: 1"));
}
