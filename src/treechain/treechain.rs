use crate::treechain::block::Block;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug)]
pub struct TreeChain {
    pub blocks: IndexMap<String, Block>,
    pub children_map: IndexMap<String, Vec<String>>,
}

impl TreeChain {
    pub fn new() -> Self {
        let genesis = Block::genesis();
        let genesis_hash = genesis.hash.clone();
        let mut blocks = IndexMap::new();
        blocks.insert(genesis_hash.clone(), genesis);
        let mut children_map = IndexMap::new();
        children_map.insert(genesis_hash, vec![]);
        TreeChain {
            blocks,
            children_map,
        }
    }

    pub fn add_block(&mut self, block: Block) {
        let hash = block.hash.clone();
        let parent_hash = block.parent_hash.clone();

        self.blocks.insert(hash.clone(), block);
        self.children_map
            .entry(parent_hash)
            .or_insert_with(Vec::new)
            .push(hash);
    }

    pub fn get_block(&self, hash: &str) -> Option<&Block> {
        self.blocks.get(hash)
    }

    pub fn get_children(&self, parent_hash: &str) -> Option<&Vec<String>> {
        self.children_map.get(parent_hash)
    }

    pub fn blocks_at_level(&self, level: u32) -> Vec<&String> {
        let mut result = Vec::new();

        for block in self.blocks.values() {
            if block.level == level {
                result.push(&block.hash);
            } else {
                break;
            }
        }
        result
    }
}
