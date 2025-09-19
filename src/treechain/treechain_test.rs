use crate::config::CHILDREN;
use crate::treechain::block::{Block, PQPEntry};
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn test_pqp_initialization() {
    let pqp = PQP::new();
    assert_eq!(pqp.pool.len(), 1);

    let genesis_entry = &pqp.pool[0];
    let genesis_block = Block::genesis();

    assert_eq!(genesis_entry.queue_index, 0);
    assert_eq!(genesis_entry.block_hash, genesis_block.hash);
    assert_eq!(genesis_entry.pqp_commitment, genesis_block.pqp_commitment);
}

#[test]
fn test_add_entry_normal_behavior() {
    let mut pqp = PQP::new();
    let current_parent = pqp
        .current_parent()
        .expect("Should have current parent")
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();
    let new_entry = ParentQueueEntry::new(
        1,
        1,
        "block1".to_string(),
        current_parent.block_hash.clone(),
        "Miner1".to_string(),
        latest.pqp_commitment.clone(),
        "sig1".to_string(),
        "pqp_commitment".to_string(),
    );

    pqp.add_entry(new_entry.clone());

    assert_eq!(pqp.pool.len(), 2);
    assert!(pqp.pool.contains(&new_entry));
}

#[test]
fn test_current_and_next_parent() {
    let mut pqp = PQP::new();
    let current_parent = pqp
        .current_parent()
        .expect("Should have current parent")
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();

    // Add CHILDREN entries with increasing queue_index
    let mut entries = Vec::new();
    for i in 1..=(CHILDREN as u32) {
        let entry = ParentQueueEntry::new(
            i,
            1,
            format!("block{}", i),
            current_parent.block_hash.clone(),
            format!("Miner{}", i),
            latest.pqp_commitment.clone(),
            format!("sig{}", i),
            format!("pqp_commit_{}", i),
        );
        pqp.add_entry(entry.clone());
        entries.push(entry);
    }

    // The current parent (after max CHILDREN children) should not be the initial parent
    let current = pqp.current_parent().expect("Should have current parent");
    assert_ne!(current.block_hash, current_parent.block_hash);

    // The entries should be in increasing order, so current is the child with smallest queue_index,
    // and next is the child with next smallest queue_index
    let mut sorted_entries = entries.clone();
    sorted_entries.sort_by_key(|e| e.queue_index);

    let child1 = &sorted_entries[0];
    let child2 = &sorted_entries[1 % sorted_entries.len()];

    assert_eq!(current.block_hash, child1.block_hash);

    // If more than one child, check next parent as well
    if entries.len() > 1 {
        let next = pqp.next_parent().expect("Should have next parent");
        assert_eq!(next.block_hash, child2.block_hash);
    }
}

#[test]
fn test_children_limit_removes_current_parent() {
    let mut pqp = PQP::new();

    let parent_hash = pqp
        .current_parent()
        .expect("Should have current parent")
        .block_hash
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();

    // Add CHILDREN entries with parent_hash matching the current parent
    for i in 1..=CHILDREN as u32 {
        let entry = ParentQueueEntry::new(
            i,
            1,
            format!("block{}", i),
            parent_hash.clone(),
            format!("Miner{}", i),
            latest.pqp_commitment.clone(),
            format!("sig{}", i),
            format!("pqp_commit_{}", i),
        );
        pqp.add_entry(entry);
    }

    // After adding CHILDREN entries, current parent should be removed
    let current = pqp.current_parent();
    assert!(current.is_some());
    assert_ne!(
        current.unwrap().block_hash,
        parent_hash,
        "Current parent should have been removed"
    );
}

#[test]
fn test_next_parent_children_removal_of_current_parent() {
    let mut pqp = PQP::new();
    let current_parent = pqp
        .current_parent()
        .expect("Should have current parent")
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();

    // Add a next parent with queue_index larger than genesis and its child entries
    let next_parent = ParentQueueEntry::new(
        1,
        1,
        "next_parent_block".to_string(),
        current_parent.block_hash.clone(),
        "MinerX".to_string(),
        latest.pqp_commitment.clone(),
        "sigX".to_string(),
        "pqp_commit_next_parent".to_string(),
    );
    pqp.add_entry(next_parent.clone());
    // Add a child entry for the new next parent block
    let next_parent = pqp.next_parent().expect("Should have next parent").clone();
    let next_parent_child = ParentQueueEntry::new(
        2,
        2,
        "child_of_next_parent".to_string(),
        next_parent.block_hash.clone(),
        "MinerY".to_string(),
        next_parent.pqp_commitment.clone(),
        "sigY".to_string(),
        "pqp_commit_child_next_parent".to_string(),
    );

    pqp.add_entry(next_parent_child);
    // The current parent (genesis) should have been removed as next parent has children
    let current = pqp.current_parent();
    assert!(current.is_some());
    assert_ne!(
        current.unwrap().block_hash,
        current_parent.block_hash,
        "Current parent should be removed when next parent has children"
    );
}

#[test]
fn test_treechain_add_block_and_get_children() {
    let mut treechain = TreeChain::new();

    let genesis_hash = Block::genesis().hash;

    // Mine new blocks and add them
    for i in 1..=3 {
        let pqp_entry = PQPEntry {
            queue_index: i,
            miner_address: format!("Miner{}", i),
            prev_pqp_commitment: "".to_string(),
            signature: "sig".to_string(),
        };

        let block = Block::new(
            format!("hash{}", i),
            format!("pqp_commit_{}", i),
            1,
            i.to_string(),
            1,
            genesis_hash.clone(),
            "merkle_root".to_string(),
            0,
            "1d00ffff".to_string(),
            0,
            0,
            pqp_entry,
            0,
            vec![],
        );

        treechain.add_block(block);
    }

    let children = treechain
        .get_children(&genesis_hash)
        .expect("Genesis should have children");
    assert_eq!(children.len(), 3);
    assert_eq!(
        children,
        &vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string()
        ]
    );
}

#[test]
fn test_pqp_prev_pqp_commit_with_siblings() {
    let mut pqp = PQP::new();
    let current_parent = pqp
        .current_parent()
        .expect("Should have current parent")
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();

    let mut prev_commitment = latest.pqp_commitment.clone();

    // Add up to CHILDREN siblings
    for i in 1..=(CHILDREN as u32) {
        let sibling = ParentQueueEntry::new(
            i,
            1,
            format!("sibling{}", i),
            current_parent.block_hash.clone(),
            format!("MinerSibling{}", i),
            prev_commitment.clone(),
            format!("sigSibling{}", i),
            format!("commit_sibling{}", i),
        );
        let prev_pool_len = pqp.pool.len();
        pqp.add_entry(sibling.clone());

        // The first sibling should be accepted, following siblings must have correct prev_pqp_commitment
        if i == 1 {
            assert_eq!(pqp.pool.len(), prev_pool_len + 1);
            assert!(pqp.pool.contains(&sibling));
        } else {
            // Use an incorrect prev_pqp_commitment for negative test
            let bad_sibling = ParentQueueEntry::new(
                i,
                2,
                format!("bad_sibling{}", i),
                current_parent.block_hash.clone(),
                format!("MinerBad{}", i),
                "incorrect_commitment".to_string(),
                format!("sigBad{}", i),
                format!("bad_commit_sibling{}", i),
            );
            let prev_pool_len = pqp.pool.len();
            pqp.add_entry(bad_sibling.clone());
            assert_eq!(
                pqp.pool.len(),
                prev_pool_len,
                "Incorrect sibling should not be added"
            );
        }

        prev_commitment = sibling.pqp_commitment.clone();
    }

    // After all siblings are added, ensure that all their prev_pqp_commitment values are chained correctly
    let siblings: Vec<_> = pqp
        .pool
        .iter()
        .filter(|e| {
            e.parent_hash == current_parent.block_hash && e.block_hash.starts_with("sibling")
        })
        .collect();

    for win in siblings.windows(2) {
        let prev = &win[0];
        let next = &win[1];
        assert_eq!(next.prev_pqp_commitment.clone(), prev.pqp_commitment);
    }
}

#[test]
fn test_prev_commit_for_first_child_is_latest_commit() {
    let mut pqp = PQP::new();
    let current_parent = pqp
        .current_parent()
        .expect("Should have current parent")
        .clone();
    let latest = pqp.latest().expect("Should have latest entry").clone();

    // For the first child of the parent, prev_pqp_commitment must be latest.pqp_commitment
    let child = ParentQueueEntry::new(
        1,
        1,
        "first_child".to_string(),
        current_parent.block_hash.clone(),
        "MinerFirst".to_string(),
        latest.pqp_commitment.clone(),
        "sigFirst".to_string(),
        "commit_first_child".to_string(),
    );
    pqp.add_entry(child.clone());
    assert!(pqp.pool.contains(&child));

    // Now try with a wrong prev_pqp_commitment
    let bad_child = ParentQueueEntry::new(
        2,
        2,
        "bad_child".to_string(),
        current_parent.block_hash.clone(),
        "MinerBad".to_string(),
        "wrong_commitment".to_string(),
        "sigBad".to_string(),
        "commit_bad_child".to_string(),
    );
    let prev_len = pqp.pool.len();
    pqp.add_entry(bad_child);
    assert_eq!(pqp.pool.len(), prev_len);
}
#[test]
fn test_out_of_order_child_rejected_and_next_parent_behavior() {
    let mut pqp = PQP::new();
    let current_parent = pqp.current_parent().unwrap().clone();
    let latest = pqp.latest().unwrap().clone();

    // Add a new next_parent right after genesis (same parent is genesis)
    let new_parent_entry = ParentQueueEntry::new(
        1,
        1,
        "next_parent".to_string(),
        current_parent.block_hash.clone(),
        "MinerNext".to_string(),
        latest.pqp_commitment.clone(),
        "sigNext".to_string(),
        "commit_next_parent".to_string(),
    );
    pqp.add_entry(new_parent_entry.clone());

    // Try to add a child for current_parent, but now that next_parent exists, if its child is added, current_parent must be removed
    let next_parent = pqp.next_parent().unwrap().clone();
    let child_for_next_parent = ParentQueueEntry::new(
        2,
        2,
        "child_of_next".to_string(),
        next_parent.block_hash.clone(),
        "MinerChild".to_string(),
        new_parent_entry.pqp_commitment.clone(),
        "sigChild".to_string(),
        "commit_child_next".to_string(),
    );
    pqp.add_entry(child_for_next_parent.clone());

    let current = pqp.current_parent().unwrap();
    assert_ne!(current.block_hash, current_parent.block_hash); // genesis should be gone!

    // The child for next_parent should be present in the pool
    assert!(
        pqp.pool
            .iter()
            .any(|e| e.block_hash == child_for_next_parent.block_hash)
    );
}

#[test]
fn test_verify_and_add_block_success() {
    let mut treechain = TreeChain::new();
    let genesis = Block::genesis();
    // Clone genesis and slightly modify hash so it passes verification failure
    let mut block = genesis.clone();
    block.nonce = 1;
    block.pqp_entry.queue_index = 1;
    Block::calculate_hash_and_pqp_commitment(&mut block);

    let added = treechain.verify_and_add_block(&block);
    assert!(added);
    assert!(treechain.blocks.contains_key(&block.hash));
}

#[test]
fn test_verify_and_add_block_fail_on_bad_hash() {
    let mut treechain = TreeChain::new();
    let mut block = Block::genesis();
    block.hash = "bad_hash_1234567890".to_string(); // corrupt hash

    let added = treechain.verify_and_add_block(&block);
    assert!(!added);
}

#[test]
fn test_get_prev_pqp_empty_pool() {
    let pqp = PQP { pool: vec![] };
    assert_eq!(pqp.get_prev_pqp(1), "");
}

#[test]
fn test_get_prev_pqp_only_genesis() {
    let pqp = PQP::new();
    let expected = "2684fa0c2d3c863c19790bc716c2568b70acfb5e8c8a21c45352563a8079a2fd".to_string();
    assert_eq!(pqp.get_prev_pqp(1), expected);
}

#[test]
fn test_get_prev_pqp_incomplete_siblings() {
    // Manually construct pool with genesis + 1 child
    let genesis_entry = ParentQueueEntry {
        queue_index: 0,
        align: 0,
        block_hash: "gen".to_string(),
        parent_hash: "00".to_string(),
        miner_address: "gen".to_string(),
        prev_pqp_commitment: "00".to_string(),
        signature: "".to_string(),
        pqp_commitment: "gen_commit".to_string(),
    };

    let child1 = ParentQueueEntry {
        queue_index: 1,
        align: 1,
        block_hash: "c1".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner1".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig1".to_string(),
        pqp_commitment: "c1_commit".to_string(),
    };

    let pqp = PQP {
        pool: vec![genesis_entry, child1],
    };

    // Should fallback to genesis commit
    assert_eq!(pqp.get_prev_pqp(2), "gen_commit");
    assert_eq!(pqp.get_prev_pqp(1), "gen_commit"); // Even for existing align, since siblings only genesis
}

#[test]
fn test_get_prev_pqp_complete_siblings() {
    // Pool after completing siblings and removing genesis: [c1, c2, c3]
    let child1 = ParentQueueEntry {
        queue_index: 1,
        align: 1,
        block_hash: "c1".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner1".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig1".to_string(),
        pqp_commitment: "c1_commit".to_string(),
    };

    let child2 = ParentQueueEntry {
        queue_index: 2,
        align: 2,
        block_hash: "c2".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner2".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig2".to_string(),
        pqp_commitment: "c2_commit".to_string(),
    };

    let child3 = ParentQueueEntry {
        queue_index: 3,
        align: 3,
        block_hash: "c3".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner3".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig3".to_string(),
        pqp_commitment: "c3_commit".to_string(),
    };

    let pqp = PQP {
        pool: vec![child1, child2, child3],
    };

    // Matches exact align
    assert_eq!(pqp.get_prev_pqp(1), "c1_commit");
    assert_eq!(pqp.get_prev_pqp(2), "c2_commit");
    assert_eq!(pqp.get_prev_pqp(3), "c3_commit");

    // Fallback to max queue_index (c3)
    assert_eq!(pqp.get_prev_pqp(4), "c3_commit");
}

#[test]
fn test_get_prev_pqp_with_next_level_incomplete() {
    // Pool: [c1, c2, c3, d1] where d1 is child of c1
    let child1 = ParentQueueEntry {
        queue_index: 1,
        align: 1,
        block_hash: "c1".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner1".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig1".to_string(),
        pqp_commitment: "c1_commit".to_string(),
    };

    let child2 = ParentQueueEntry {
        queue_index: 2,
        align: 2,
        block_hash: "c2".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner2".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig2".to_string(),
        pqp_commitment: "c2_commit".to_string(),
    };

    let child3 = ParentQueueEntry {
        queue_index: 3,
        align: 3,
        block_hash: "c3".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner3".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig3".to_string(),
        pqp_commitment: "c3_commit".to_string(),
    };

    let d1 = ParentQueueEntry {
        queue_index: 4,
        align: 1,
        block_hash: "d1".to_string(),
        parent_hash: "c1".to_string(),
        miner_address: "miner4".to_string(),
        prev_pqp_commitment: "c1_commit".to_string(),
        signature: "sig4".to_string(),
        pqp_commitment: "d1_commit".to_string(),
    };

    let pqp = PQP {
        pool: vec![child1, child2, child3, d1],
    };

    // Should get from previous group (c1, c2, c3)
    assert_eq!(pqp.get_prev_pqp(1), "c1_commit");
    assert_eq!(pqp.get_prev_pqp(2), "c2_commit");
    assert_eq!(pqp.get_prev_pqp(3), "c3_commit");
    assert_eq!(pqp.get_prev_pqp(4), "c3_commit"); // Fallback
}

#[test]
fn test_get_prev_pqp_after_removing_previous_parent() {
    // Pool after filling children for c1 and removing c1: [c2, c3, d1, d2, d3]
    let child2 = ParentQueueEntry {
        queue_index: 2,
        align: 2,
        block_hash: "c2".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner2".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig2".to_string(),
        pqp_commitment: "c2_commit".to_string(),
    };

    let child3 = ParentQueueEntry {
        queue_index: 3,
        align: 3,
        block_hash: "c3".to_string(),
        parent_hash: "gen".to_string(),
        miner_address: "miner3".to_string(),
        prev_pqp_commitment: "gen_commit".to_string(),
        signature: "sig3".to_string(),
        pqp_commitment: "c3_commit".to_string(),
    };

    let d1 = ParentQueueEntry {
        queue_index: 4,
        align: 1,
        block_hash: "d1".to_string(),
        parent_hash: "c1".to_string(),
        miner_address: "miner4".to_string(),
        prev_pqp_commitment: "c1_commit".to_string(), // Assuming c1_commit from previous
        signature: "sig4".to_string(),
        pqp_commitment: "d1_commit".to_string(),
    };

    let d2 = ParentQueueEntry {
        queue_index: 5,
        align: 2,
        block_hash: "d2".to_string(),
        parent_hash: "c1".to_string(),
        miner_address: "miner5".to_string(),
        prev_pqp_commitment: "c2_commit".to_string(),
        signature: "sig5".to_string(),
        pqp_commitment: "d2_commit".to_string(),
    };

    let d3 = ParentQueueEntry {
        queue_index: 6,
        align: 3,
        block_hash: "d3".to_string(),
        parent_hash: "c1".to_string(),
        miner_address: "miner6".to_string(),
        prev_pqp_commitment: "c3_commit".to_string(),
        signature: "sig6".to_string(),
        pqp_commitment: "d3_commit".to_string(),
    };

    let pqp = PQP {
        pool: vec![child2, child3, d1, d2, d3],
    };

    // Should get from latest group (d1, d2, d3)
    assert_eq!(pqp.get_prev_pqp(1), "d1_commit");
    assert_eq!(pqp.get_prev_pqp(2), "d2_commit");
    assert_eq!(pqp.get_prev_pqp(3), "d3_commit");
    assert_eq!(pqp.get_prev_pqp(4), "d3_commit"); // Fallback to max (d3)
}
