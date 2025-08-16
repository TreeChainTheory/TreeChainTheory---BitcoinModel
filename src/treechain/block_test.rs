use crate::treechain::block::{Block, PQPEntry};
use num_bigint::BigUint;

#[test]
fn test_block_genesis_block() {
    let g = Block::genesis();
    assert_eq!(g.level, 0);
    assert_eq!(g.position, "a");
    assert_eq!(g.tx.len(), 0);
}

#[test]
fn test_merkle_root_single_tx() {
    let txs = vec!["abcd".repeat(16)];
    let root = Block::merkle_root(txs.clone());
    assert_eq!(root.len(), 64);
}

#[test]
fn test_target_calculation() {
    // Bitcoin’s genesis bits = 0x1d00ffff
    let target = Block::calculate_target("1d00ffff".to_string());
    assert!(target.is_some());
    let t = target.unwrap();
    assert!(t > BigUint::from(0u32))
}

#[test]
fn test_mining_block() {
    let parent = Block::genesis();
    let pqp = PQPEntry {
        queue_index: 1,
        block_hash: "00".repeat(32),
        parent_hash: parent.hash.clone(),
        miner_address: "Miner1".to_string(),
        signature: "11".repeat(32),
    };

    let txs = vec!["txn_assbabc".repeat(8)];
    let mined = Block::mine_block_example(&parent, 0, "207fffff".to_string(), pqp, txs);
    assert_eq!(mined.level, parent.level + 1);
    assert_eq!(mined.parent_hash, parent.hash);
    assert_eq!(mined.pqp_entry.parent_hash, parent.hash);
    assert!(!mined.hash.is_empty());
}
