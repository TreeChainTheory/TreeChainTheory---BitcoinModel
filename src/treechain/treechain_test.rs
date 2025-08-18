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
fn test_add_entry_normal_behavior() {
    let mut pqp = PQP::new();
    let current_parent = pqp.current_parent().expect("Should have current parent");
    let latest = pqp.latest().expect("Should have latest entry");
    let new_entry = ParentQueueEntry::new(
        1,
        "block1".to_string(),
        current_parent.block_hash.clone(),
        latest.pqp_commitment.clone(),
        "pqp_commitment".to_string(),
    );

    pqp.add_entry(new_entry.clone());

    assert_eq!(pqp.pool.len(), 2);
    assert!(pqp.pool.contains(&new_entry));
}

#[test]
fn test_current_and_next_parent() {
    let mut pqp = PQP::new();
    let current_parent = pqp.current_parent().expect("Should have current parent");
    let latest = pqp.latest().expect("Should have latest entry");
    // Add entries with queue_index 1 and 2
    let entry1 = ParentQueueEntry::new(
        1,
        "block1".to_string(),
        current_parent.block_hash.clone(),
        latest.pqp_commitment.clone(),
        "pqp_commit_1".to_string(),
    );
    let entry2 = ParentQueueEntry::new(
        2,
        "block2".to_string(),
        current_parent.block_hash.clone(),
        latest.pqp_commitment.clone(),
        "pqp_commit_2".to_string(),
    );

    pqp.add_entry(entry1.clone());
    pqp.add_entry(entry2.clone());

    let current = pqp.current_parent().expect("Should have current parent");
    let next = pqp.next_parent().expect("Should have next parent");

    // current parent should have smallest queue_index (0 is genesis, so the genesis entry)
    assert_eq!(current.queue_index, 1);
    // next parent should have second smallest queue_index (1)
    assert_eq!(next.queue_index, 2);
}

#[test]
fn test_children_limit_removes_current_parent() {
    let mut pqp = PQP::new();

    let parent_hash = pqp.pool[0].block_hash.clone();

    // Add CHILDREN entries with parent_hash matching the current parent
    for i in 1..=CHILDREN as u32 {
        let entry = ParentQueueEntry::new(
            i,
            format!("block{}", i),
            parent_hash.clone(),
            pqp.pool[0].prev_pqp_commitment.clone(),
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
    let latest = pqp.latest().expect("Should have latest entry");

    // Add a next parent with queue_index larger than genesis and its child entries
    let next_parent = ParentQueueEntry::new(
        1,
        "next_parent_block".to_string(),
        current_parent.block_hash.clone(),
        latest.pqp_commitment.clone(),
        "pqp_commit_next_parent".to_string(),
    );
    pqp.add_entry(next_parent.clone());
    // Add a child entry for the new next parent block
    let next_parent = pqp.next_parent().expect("Should have next parent").clone();
    let next_parent_child = ParentQueueEntry::new(
        2,
        "child_of_next_parent".to_string(),
        next_parent.block_hash.clone(),
        next_parent.pqp_commitment.clone(),
        "pqp_commit_child_next_parent".to_string(),
    );

    pqp.add_entry(next_parent_child);
    print!("\n pqp pool: {:?}", pqp.pool);
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
