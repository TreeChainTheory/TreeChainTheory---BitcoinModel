// use crate::treechain;
// use crate::treechain::block::{Block, PQPEntry};
// use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
// use crate::wallet::transaction::Transaction;
// use crate::wallet::transaction::{TxInput, TxOutput};
// use num_bigint::BigUint;

// #[test]
// fn test_block_genesis_block() {
//     let g = Block::genesis();
//     println!("Genesis Block: {:?}", g);
//     assert_eq!(g.level, 0);
//     assert_eq!(g.position, "0");
//     assert_eq!(g.tx.len(), 0);
//     assert_eq!(g.pqp_commitment.len(), 64);
// }

// #[test]
// fn test_mine_genesis_block() {
//     // Start with a genesis template (all fields as in genesis(), but nonce=0, hash="", pqp_commitment="")
//     let mut genesis_template = Block::new(
//         "".to_string(), // hash
//         "".to_string(), // pqp_commitment
//         0,
//         "0".to_string(),
//         1,
//         "00".repeat(32),
//         "00".repeat(32),
//         0,
//         Block::genesis().bits, // initial bits
//         0,                     // start nonce at 0
//         0,
//         PQPEntry {
//             queue_index: 0,
//             miner_address: "GENISIS_LEADER_HEX".to_string(),
//             prev_pqp_commitment: "00".repeat(32),
//             signature: "".repeat(64),
//         },
//         0,
//         vec![],
//     );

//     let target = Block::calculate_target(Block::genesis().bits).unwrap();
//     let mut nonce = 0u32;
//     let mut found = false;
//     let mut iterations = 0u64;

//     loop {
//         genesis_template.nonce = nonce;
//         Block::calculate_hash_and_pqp_commitment(&mut genesis_template);

//         let hash_bytes = hex::decode(&genesis_template.hash).unwrap_or_default();
//         let hash_int = BigUint::from_bytes_be(&hash_bytes);

//         if hash_int < target {
//             found = true;
//             println!(
//                 "✅ Mined genesis block! Nonce: {}, Hash: {}, Iterations: {}",
//                 nonce, genesis_template.hash, iterations
//             );
//             println!("Genesis Block: {:?}", genesis_template);
//             break;
//         }

//         nonce = nonce.wrapping_add(1);
//         iterations += 1;
//     }

//     assert!(found);
//     // Verify it matches the hardcoded genesis
//     assert_eq!(
//         genesis_template.hash,
//         "0000181c51c930a46ede1edbd3082c0e0d3673334fac3ddc60262c66a2c46b22"
//     );
//     assert_eq!(genesis_template.pqp_commitment.len(), 64);
// }

// #[test]
// fn test_merkle_root_single_tx() {
//     // Create a sample transaction
//     let sample_tx = Transaction::create_sample_transaction();

//     let txs = vec![sample_tx];
//     let root = Block::merkle_root(txs);
//     assert_eq!(root.len(), 64);
//     assert_ne!(root, "0".repeat(64));
// }

// #[test]
// fn test_merkle_root_two_txs() {
//     // Create two sample transactions
//     let tx1 = Transaction::create_sample_transaction();
//     let tx2 = Transaction::create_sample_transaction_with_different_output();

//     let txs = vec![tx1, tx2];
//     let root = Block::merkle_root(txs);
//     assert_eq!(root.len(), 64);
//     assert_ne!(root, "0".repeat(64));
// }

// #[test]
// fn test_target_calculation() {
//     let target = Block::calculate_target("1d00ffff".to_string());
//     assert!(target.is_some());
//     let t = target.unwrap();
//     assert!(t > BigUint::from(0u32));
// }

// #[test]
// fn test_calculate_hash_and_pqp_commitment() {
//     let parent = Block::genesis();
//     let pqp_entry = PQPEntry {
//         queue_index: 42,
//         miner_address: "MinerX".to_string(),
//         prev_pqp_commitment: parent.pqp_commitment.clone(),
//         signature: "22".repeat(32),
//     };

//     // Create a sample transaction for the block
//     let sample_tx = Transaction::create_sample_transaction();

//     let mut block = Block::new(
//         "".to_string(),
//         "00".repeat(32),
//         parent.level + 1,
//         "a.1".to_string(),
//         1,
//         parent.hash.clone(),
//         "ff".repeat(32),
//         123456789,
//         "1d00ffff".to_string(),
//         0,
//         1,
//         pqp_entry,
//         1,
//         vec![sample_tx],
//     );

//     Block::calculate_hash_and_pqp_commitment(&mut block);

//     // Both must be populated now
//     assert_eq!(block.hash.len(), 64);
//     assert_eq!(block.pqp_commitment.len(), 64);
// }
