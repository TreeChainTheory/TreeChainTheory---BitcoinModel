use crate::treechain;
use crate::treechain::block::{Block, PQPEntry};
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
use crate::wallet::transaction::Transaction;
use crate::wallet::transaction::{TxInput, TxOutput};
use num_bigint::BigUint;

#[test]
fn test_block_genesis_block() {
    let g = Block::genesis();
    assert_eq!(g.level, 0);
    assert_eq!(g.position, "0");
    assert_eq!(g.tx.len(), 0);
    assert_eq!(g.pqp_commitment.len(), 64);
}

#[test]
fn test_merkle_root_single_tx() {
    // Create a sample transaction
    let sample_tx = Transaction::create_sample_transaction();

    let txs = vec![sample_tx];
    let root = Block::merkle_root(txs);
    assert_eq!(root.len(), 64);
    assert_ne!(root, "0".repeat(64));
}

#[test]
fn test_merkle_root_two_txs() {
    // Create two sample transactions
    let tx1 = Transaction::create_sample_transaction();
    let tx2 = Transaction::create_sample_transaction_with_different_output();

    let txs = vec![tx1, tx2];
    let root = Block::merkle_root(txs);
    assert_eq!(root.len(), 64);
    assert_ne!(root, "0".repeat(64));
}

#[test]
fn test_target_calculation() {
    let target = Block::calculate_target("1d00ffff".to_string());
    assert!(target.is_some());
    let t = target.unwrap();
    assert!(t > BigUint::from(0u32));
}

#[test]
fn test_calculate_hash_and_pqp_commitment() {
    let parent = Block::genesis();
    let pqp_entry = PQPEntry {
        queue_index: 42,
        miner_address: "MinerX".to_string(),
        prev_pqp_commitment: parent.pqp_commitment.clone(),
        signature: "22".repeat(32),
    };

    // Create a sample transaction for the block
    let sample_tx = Transaction::create_sample_transaction();

    let mut block = Block::new(
        "".to_string(),
        "00".repeat(32),
        parent.level + 1,
        "a.1".to_string(),
        1,
        parent.hash.clone(),
        "ff".repeat(32),
        123456789,
        "1d00ffff".to_string(),
        0,
        1,
        pqp_entry,
        1,
        vec![sample_tx],
    );

    Block::calculate_hash_and_pqp_commitment(&mut block);

    // Both must be populated now
    assert_eq!(block.hash.len(), 64);
    assert_eq!(block.pqp_commitment.len(), 64);
}
