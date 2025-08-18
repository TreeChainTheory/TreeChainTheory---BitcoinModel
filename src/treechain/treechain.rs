use crate::config::CHILDREN;
use crate::treechain::block::Block;
use indexmap::IndexMap;
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

    pub fn latest(&self) -> Option<&ParentQueueEntry> {
        self.pool.last()
    }

    pub fn current_parent(&self) -> Option<&ParentQueueEntry> {
        self.pool.iter().take(10).min_by_key(|e| e.queue_index)
    }

    pub fn next_parent(&self) -> Option<&ParentQueueEntry> {
        let mut candidates: Vec<&ParentQueueEntry> = self.pool.iter().take(10).collect();
        candidates.sort_by_key(|e| e.queue_index);
        if candidates.len() >= 2 {
            Some(candidates[1])
        } else {
            None
        }
    }

    pub fn add_entry(&mut self, entry: ParentQueueEntry) {
        let current_parent = self.current_parent().cloned();
        print!("\n current_parent: {:?}", current_parent);
        let next_parent = self.next_parent().cloned();
        print!("\n next_parent: {:?}", next_parent);
        let latest = self.latest().cloned();

        if let Some(current) = current_parent {
            if entry.parent_hash == current.block_hash {
                let latest_sibling = self
                    .pool
                    .iter()
                    .rev()
                    .take(10)
                    .find(|e| e.parent_hash == current.block_hash);

                let expected_prev_commit = if let Some(sibling) = latest_sibling {
                    // If siblings exist → use last sibling's prev_pqp_commitment
                    sibling.prev_pqp_commitment.clone()
                } else {
                    // If no siblings → use latest entry's pqp_commitment
                    let Some(latest) = latest else {
                        panic!("Latest entry should exist");
                    };
                    latest.pqp_commitment.clone()
                };

                if entry.prev_pqp_commitment != expected_prev_commit {
                    // Invalid commit → reject early
                    return;
                }
                let children_count_current = self
                    .pool
                    .iter()
                    .rev()
                    .take(10)
                    .filter(|e| e.parent_hash == current.block_hash)
                    .count();

                let next_parent_has_children = if let Some(next) = next_parent {
                    self.pool
                        .iter()
                        .rev()
                        .take(10)
                        .any(|e| e.parent_hash == next.block_hash)
                } else {
                    false
                };

                if !next_parent_has_children && children_count_current < CHILDREN as usize {
                    self.pool.push(entry);

                    let updated_children_count = self
                        .pool
                        .iter()
                        .rev()
                        .take(10)
                        .filter(|e| e.parent_hash == current.block_hash)
                        .count();

                    if updated_children_count >= CHILDREN as usize {
                        self.pool.retain(|e| e.block_hash != current.block_hash);
                    }
                } else {
                    self.pool.retain(|e| e.block_hash != current.block_hash);
                }
            } else if let Some(next) = next_parent {
                if entry.parent_hash == next.block_hash {
                    let Some(latest) = latest else {
                        panic!("Latest entry should exist");
                    };
                    let expected_prev_commit = latest.pqp_commitment.clone();

                    if entry.prev_pqp_commitment != expected_prev_commit {
                        return; // invalid → reject
                    }
                    self.pool.push(entry);
                    self.pool.retain(|e| e.block_hash != current.block_hash);
                }
            }
        } else {
            self.pool.push(entry);
        }
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
