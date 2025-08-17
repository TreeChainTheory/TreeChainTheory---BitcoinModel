use crate::treechain::block::Block;
use serde::{Deserialize, Serialize};

/// Represents an entry in the global PQP (Pending Queue of Parents)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParentQueueEntry {
    pub queue_index: u32,
    pub block_hash: String,
    pub parent_hash: String,
    pub prev_pqp_commitment: String,
    pub pqp_commitment: String,
}

#[derive(Debug)]
pub struct PQP {
    pub pool: Vec<ParentQueueEntry>,
}

impl PQP {
    pub fn new() -> Self {
        let genesis = Block::genesis();

        let genesis_entry = ParentQueueEntry {
            queue_index: 0,
            block_hash: genesis.hash.clone(),
            parent_hash: genesis.parent_hash.clone(),
            prev_pqp_commitment: genesis.pqp_entry.prev_pqp_commitment.clone(),
            pqp_commitment: genesis.pqp_commitment.clone(),
        };

        PQP {
            pool: vec![genesis_entry],
        }
    }

    pub fn add_entry(&mut self, entry: ParentQueueEntry) {
        self.pool.push(entry);
    }

    pub fn latest(&self) -> Option<&ParentQueueEntry> {
        self.pool.last()
    }

    pub fn next_parent(&self) -> Option<&ParentQueueEntry> {
        self.pool.iter().take(10).min_by_key(|e| e.queue_index)
    }
}

impl ParentQueueEntry {
    pub fn new(
        queue_index: u32,
        block_hash: String,
        parent_hash: String,
        prev_pqp_commitment: String,
        pqp_commitment: String,
    ) -> Self {
        Self {
            queue_index,
            block_hash,
            parent_hash,
            prev_pqp_commitment,
            pqp_commitment,
        }
    }
}
