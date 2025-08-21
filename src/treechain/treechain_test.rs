use crate::config::CHILDREN;
use crate::treechain::block::{Block, PQPEntry};
use crate::treechain::treechain::{PQP, ParentQueueEntry, TreeChain};

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
fn test_mine_block_demo() {
    // Setup: create new treechain and pqp
    let mut treechain = TreeChain::new();
    let mut pqp = PQP::new();

    // Prepare parameters
    let align = 1;
    let txs = vec!["deadbeef".repeat(8)];
    let miner_address = "MinerXYZ".to_string();
    let signature = "abc123sig".repeat(8);

    // Call mine_block_demo
    let mined_block = treechain.mine_block_demo(
        &mut pqp,
        align,
        txs.clone(),
        miner_address.clone(),
        signature.clone(),
    );

    // Ensure output is present
    assert!(mined_block.is_some());
    let mined_block = mined_block.unwrap();

    // The block should be in the chain
    let block_in_chain = treechain.get_block(&mined_block.hash);
    assert!(block_in_chain.is_some());

    // The block's miner address and signature should match what was provided
    assert_eq!(mined_block.pqp_entry.miner_address, miner_address);
    assert_eq!(mined_block.pqp_entry.signature, signature);

    // The block should be at the right level and have correct parent hash
    let parent = Block::genesis();
    assert_eq!(mined_block.level, parent.level + 1);
    assert_eq!(mined_block.parent_hash, parent.hash);

    // Check that the PQP was updated: new PQP entry present in pool, and parent block potentially removed
    let contains_mined_pqp = pqp.pool.iter().any(|e| e.block_hash == mined_block.hash);
    assert!(
        contains_mined_pqp,
        "PQP should contain new entry corresponding to mined block"
    );

    // The PQP commitments chain must be valid (prev_pqp_commitment matches previous entry's pqp_commitment for new block)
    if let Some(prev_entry) = pqp
        .pool
        .iter()
        .rev()
        .find(|e| e.block_hash == mined_block.parent_hash)
    {
        assert_eq!(
            mined_block.pqp_entry.prev_pqp_commitment,
            prev_entry.pqp_commitment
        );
    }
}

#[test]
fn test_mine_blocks_with_multiple_aligns() {
    let mut treechain = TreeChain::new();
    let mut pqp = PQP::new();

    let miner_address = "MinerXYZ".to_string();
    let signature = "sig123456".to_string();

    // Mine 10 pairs of blocks, two per iteration with align 0 and 1 respectively
    for i in 1..=10 {
        // Align 0 block
        let txs0 = vec![format!("tx{}_a", i).repeat(4)];
        let mined_block_0 = treechain
            .mine_block_demo(&mut pqp, 1, txs0, miner_address.clone(), signature.clone())
            .unwrap_or_else(|| panic!("Failed to mine block with align 1, iter: {}", i));
        println!(
            "\n Mined block (align={}) {}: hash {}",
            mined_block_0.align, i, mined_block_0.hash
        );

        // Align 1 block
        let txs1 = vec![format!("tx{}_b", i).repeat(4)];
        let mined_block_1 = treechain
            .mine_block_demo(&mut pqp, 2, txs1, miner_address.clone(), signature.clone())
            .unwrap_or_else(|| panic!("Failed to mine block with align 1, iter: {}", i));
        println!(
            "Mined block (align={}) {}: hash {}",
            mined_block_1.align, i, mined_block_1.hash
        );
    }

    // Print blocks grouped by level to show tree structure
    let max_level = treechain
        .blocks
        .values()
        .map(|b| b.level)
        .max()
        .unwrap_or(0);

    println!("\nTreeChain blocks by level:");
    // println!("Total blocks: {:?}", treechain.blocks);
    // println!("pqp: {:?}", pqp.pool);
    for level in 0..=max_level {
        let blocks_at_level = treechain.blocks_at_level(level);
        println!("Level {}: count {}", level, blocks_at_level.len());
        for hash in blocks_at_level {
            println!(
                "  {}  {}  {}",
                hash,
                treechain
                    .get_block(&hash)
                    .unwrap()
                    .pqp_entry
                    .queue_index
                    .clone(),
                treechain.get_block(&hash).unwrap().align
            );
        }
    }

    println!("pqp: {:?}", pqp.pool);
}

#[test]
fn test_is_valid_tree() {
    let mut treechain = TreeChain::new();
    let mut pqp = PQP::new();

    let miner_address = "MinerTest".to_string();
    let signature = "sigTest".to_string();

    // --- Mine a few blocks to populate tree and PQP ---
    let block1 = treechain
        .mine_block_demo(
            &mut pqp,
            1,
            vec!["tx1".to_string()],
            miner_address.clone(),
            signature.clone(),
        )
        .expect("Failed to mine block1");

    let block2 = treechain
        .mine_block_demo(
            &mut pqp,
            2,
            vec!["tx2".to_string()],
            miner_address.clone(),
            signature.clone(),
        )
        .expect("Failed to mine block2");

    // Tree with mined blocks should be valid
    assert!(
        treechain.is_valid_tree(&pqp),
        "Tree should be valid after honest mining"
    );

    // --- Tamper with block hash to break validity ---
    let bad_hash = "deadbeef".repeat(8);
    if let Some(mut bad_block) = treechain.blocks.get_mut(&block1.hash) {
        bad_block.hash = bad_hash.clone();
    }

    assert!(
        !treechain.is_valid_tree(&pqp),
        "Tree should be invalid after tampering"
    );
}
