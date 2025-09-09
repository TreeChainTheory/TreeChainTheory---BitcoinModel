use crate::config::{BITS, CHILDREN};
use crate::treechain;
use crate::treechain::block::{Block, PQPEntry};
use indexmap::IndexMap;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
/// Represents an entry in the global PQP (Pending Queue of Parents)
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParentQueueEntry {
    pub queue_index: u32,
    pub align: u8,
    pub block_hash: String,
    pub parent_hash: String,
    pub miner_address: String,
    pub prev_pqp_commitment: String,
    pub signature: String,
    pub pqp_commitment: String,
}

#[derive(Debug, Clone)]
pub struct PQP {
    pub pool: Vec<ParentQueueEntry>,
}

impl PQP {
    pub fn new() -> Self {
        let genesis = Block::genesis();

        let genesis_entry = ParentQueueEntry {
            queue_index: 0,
            align: genesis.align,
            block_hash: genesis.hash.clone(),
            parent_hash: genesis.parent_hash.clone(),
            miner_address: genesis.pqp_entry.miner_address,
            prev_pqp_commitment: genesis.pqp_entry.prev_pqp_commitment.clone(),
            signature: genesis.pqp_entry.signature.clone(),
            pqp_commitment: genesis.pqp_commitment.clone(),
        };

        PQP {
            pool: vec![genesis_entry],
        }
    }

    pub fn latest(&self) -> Option<&ParentQueueEntry> {
        self.pool
            .iter()
            .rev()
            .take(CHILDREN as usize)
            .max_by_key(|e| e.queue_index)
        // self.pool.last()
    }

    pub fn last(&self) -> Option<ParentQueueEntry> {
        self.pool.last().cloned()
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

    pub fn add_entry_to_pqp(&mut self, entry: ParentQueueEntry, treechain: &TreeChain) {
        let current_parent = self.current_parent().cloned();
        // print!("\n current_parent: {:?}", current_parent);
        // print!("\n pqp_pool: {:?}", self.pool);
        let next_parent = self.next_parent().cloned();
        // print!("\n next_parent: {:?}", next_parent);
        let latest = self.latest().cloned();

        if let Some(current) = current_parent {
            if entry.parent_hash == current.block_hash {
                let expected_prev_commit = self.get_prev_pqp_commitment(entry.align, &treechain);
                // println!(
                //     "expected_prev_commit in add_entry(): {}",
                //     expected_prev_commit
                // );
                if entry.prev_pqp_commitment != expected_prev_commit {
                    // Invalid commit → reject early
                    println!(
                        "\n entry:{:?} expected: {:?} ",
                        entry.prev_pqp_commitment, expected_prev_commit
                    );
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
                        .take(CHILDREN as usize)
                        .any(|e| e.parent_hash == next.block_hash)
                } else {
                    false
                };

                if !next_parent_has_children && children_count_current < CHILDREN as usize {
                    let mut insert_pos = self.pool.len();

                    for i in (0..self.pool.len()).rev() {
                        if self.pool[i].queue_index < entry.queue_index {
                            // Insert *after* this one
                            insert_pos = i + 1;
                            break;
                        }
                    }

                    if insert_pos == self.pool.len() {
                        self.pool.push(entry);
                    } else {
                        self.pool.insert(insert_pos, entry);
                    }

                    // self.pool.push(entry.clone());
                    let updated_children_count = self
                        .pool
                        .iter()
                        .rev()
                        .take(10)
                        .filter(|e| e.parent_hash == current.block_hash)
                        .count();

                    if updated_children_count >= CHILDREN as usize {
                        if let Some(pos) = self
                            .pool
                            .iter()
                            .position(|e| e.block_hash == current.block_hash)
                        {
                            self.pool.remove(pos);
                        }
                    }
                } else {
                    if let Some(pos) = self
                        .pool
                        .iter()
                        .position(|e| e.block_hash == current.block_hash)
                    {
                        self.pool.remove(pos);
                    }
                }
                // println!("\n pqp at add_entry:{:?}", &self.pool);
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
                    if let Some(pos) = self
                        .pool
                        .iter()
                        .position(|e| e.block_hash == current.block_hash)
                    {
                        self.pool.remove(pos);
                    }
                }
            }
        } else {
            self.pool.push(entry);
        }
    }

    pub fn add_entry(&mut self, entry: ParentQueueEntry) {
        let current_parent = self.current_parent().cloned();
        // print!("\n current_parent: {:?}", current_parent);
        // print!("\n pqp_pool: {:?}", self.pool);
        let next_parent = self.next_parent().cloned();
        // print!("\n next_parent: {:?}", next_parent);
        let latest = self.latest().cloned();

        if let Some(current) = current_parent {
            if entry.parent_hash == current.block_hash {
                let expected_prev_commit = self.get_prev_pqp(entry.align);
                // println!(
                //     "expected_prev_commit in add_entry(): {}",
                //     expected_prev_commit
                // );
                if entry.prev_pqp_commitment != expected_prev_commit {
                    // Invalid commit → reject early
                    println!(
                        "\n entry:{:?} expected: {:?} ",
                        entry.prev_pqp_commitment, expected_prev_commit
                    );
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
                        .take(CHILDREN as usize)
                        .any(|e| e.parent_hash == next.block_hash)
                } else {
                    false
                };

                if !next_parent_has_children && children_count_current < CHILDREN as usize {
                    let mut insert_pos = self.pool.len();

                    for i in (0..self.pool.len()).rev() {
                        if self.pool[i].queue_index < entry.queue_index {
                            // Insert *after* this one
                            insert_pos = i + 1;
                            break;
                        }
                    }

                    if insert_pos == self.pool.len() {
                        self.pool.push(entry);
                    } else {
                        self.pool.insert(insert_pos, entry);
                    }

                    // self.pool.push(entry.clone());
                    let updated_children_count = self
                        .pool
                        .iter()
                        .rev()
                        .take(10)
                        .filter(|e| e.parent_hash == current.block_hash)
                        .count();

                    if updated_children_count >= CHILDREN as usize {
                        if let Some(pos) = self
                            .pool
                            .iter()
                            .position(|e| e.block_hash == current.block_hash)
                        {
                            self.pool.remove(pos);
                        }
                    }
                } else {
                    if let Some(pos) = self
                        .pool
                        .iter()
                        .position(|e| e.block_hash == current.block_hash)
                    {
                        self.pool.remove(pos);
                    }
                }
                // println!("\n pqp at add_entry:{:?}", &self.pool);
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
                    if let Some(pos) = self
                        .pool
                        .iter()
                        .position(|e| e.block_hash == current.block_hash)
                    {
                        self.pool.remove(pos);
                    }
                }
            }
        } else {
            self.pool.push(entry);
        }
    }

    pub fn remove_pqp_entry(&mut self, entry: ParentQueueEntry) -> bool {
        if let Some(rev_pos) = self
            .pool
            .iter()
            .rev()
            .position(|e| e.block_hash == entry.block_hash)
        {
            // Convert reverse index to forward index
            let pos = self.pool.len() - 1 - rev_pos;
            self.pool.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_prev_pqp_commitment(&self, align: u8, treechain: &TreeChain) -> String {
        if let Some(last) = self.pool.last() {
            if let Some(current_parent) = self.current_parent() {
                let prev_parent = if current_parent.block_hash == last.parent_hash {
                    // find another parent (not same as current's parent)
                    self.pool
                        .iter()
                        .rev()
                        .take(CHILDREN as usize)
                        .find(|e| e.parent_hash != last.parent_hash)
                } else {
                    Some(last) // default: use last
                };

                let sibling_aligns: Vec<u8> = self
                    .pool
                    .iter()
                    .rev()
                    .take(CHILDREN as usize)
                    .filter(|e| e.parent_hash == current_parent.block_hash)
                    .filter_map(|e| treechain.get_block(&e.block_hash))
                    .map(|block| block.align)
                    .collect();

                if sibling_aligns.contains(&align) {
                    // align already taken
                    let prev_pqp =
                        self.pool.iter().rev().find(|e| {
                            e.parent_hash == current_parent.block_hash && e.align == align
                        });
                    return prev_pqp
                        .map(|e| e.pqp_commitment.clone())
                        .unwrap_or_default();
                }

                if let Some(prev_parent) = prev_parent {
                    let siblings: Vec<&ParentQueueEntry> = self
                        .pool
                        .iter()
                        .rev()
                        .take(10)
                        .filter(|e| e.parent_hash == prev_parent.parent_hash)
                        .collect();

                    // exact align match
                    if let Some(sibling) = siblings.iter().find(|s| s.align == align) {
                        return sibling.pqp_commitment.clone();
                    }

                    // fallback: latest sibling
                    if let Some(last_sibling) = siblings.iter().max_by_key(|e| e.queue_index) {
                        return last_sibling.pqp_commitment.clone();
                    }
                }
            }
        }
        self.pool
            .last()
            .map(|e| e.pqp_commitment.clone())
            .unwrap_or_default()
    }

    pub fn get_prev_pqp(&self, align: u8) -> String {
        if let Some(last) = self.pool.last() {
            if let Some(current_parent) = self.current_parent() {
                let prev_parent = if current_parent.block_hash == last.parent_hash {
                    // find another parent (not same as current's parent)
                    self.pool
                        .iter()
                        .rev()
                        .take(CHILDREN as usize)
                        .find(|e| e.parent_hash != last.parent_hash)
                } else {
                    Some(last) // default: use last
                };

                if let Some(prev_parent) = prev_parent {
                    let siblings: Vec<&ParentQueueEntry> = self
                        .pool
                        .iter()
                        .rev()
                        .take(10)
                        .filter(|e| e.parent_hash == prev_parent.parent_hash)
                        .collect();

                    // exact align match
                    if let Some(sibling) = siblings.iter().find(|s| s.align == align) {
                        return sibling.pqp_commitment.clone();
                    }

                    // fallback: latest sibling
                    if let Some(last_sibling) = siblings.iter().max_by_key(|e| e.queue_index) {
                        return last_sibling.pqp_commitment.clone();
                    }
                }
            }
        }
        // Fallback: pool non-empty
        self.pool
            .last()
            .map(|e| e.pqp_commitment.clone())
            .unwrap_or_default()
    }
}

impl ParentQueueEntry {
    pub fn new(
        queue_index: u32,
        align: u8,
        block_hash: String,
        parent_hash: String,
        miner_address: String,
        prev_pqp_commitment: String,
        signature: String,
        pqp_commitment: String,
    ) -> Self {
        Self {
            queue_index,
            align,
            block_hash,
            parent_hash,
            miner_address,
            prev_pqp_commitment,
            signature,
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

    pub fn calculate_length(&self) -> usize {
        self.blocks
            .iter()
            .filter(|(hash, _)| !hash.is_empty())
            .count()
    }

    pub fn add_block(&mut self, block: Block) {
        println!("add block called");
        let hash = block.hash.clone();
        let parent_hash = block.parent_hash.clone();
        let queue_index = block.pqp_entry.queue_index as usize;
        println!(
            "blocks len: {} and queue_index: {}",
            self.blocks.len(),
            queue_index
        );

        // Insert placeholders for indices up to queue_index - 1
        while self.blocks.len() < queue_index {
            // println!(
            //     "blocks len: {} and queue_index: {}",
            //     self.blocks.len(),
            //     queue_index
            // );
            let placeholder_index = self.blocks.len() as u32;
            let placeholder_block = Block::empty_placeholder(placeholder_index);
            let placeholder_hash = format!("placeholder_{}", placeholder_index);
            self.blocks.insert(placeholder_hash, placeholder_block);
        }
        println!("finished while lop");
        // Check if there's a block at queue_index
        if let Some((existing_hash, existing_block)) = self.blocks.get_index(queue_index) {
            if !(existing_block.position.is_empty()) {
                // If there's a non-placeholder block at queue_index, do not overwrite
                println!("existing_block: {:?}", existing_block);
                println!(
                    "❌ Block with queue_index {} already exists and is not a placeholder: {}",
                    queue_index, existing_hash
                );
                return;
            }

            self.blocks.swap_remove_index(queue_index); // remove placeholder
            let tail = self.blocks.split_off(queue_index); // save everything after index
            self.blocks.insert(hash.clone(), block); // insert our new block at correct spot
            self.blocks.extend(tail); // restore the rest
        } else {
            println!("calling block insert");
            self.blocks.insert(hash.clone(), block);
        }

        // Update children_map for the parent
        if !parent_hash.is_empty() {
            self.children_map
                .entry(parent_hash)
                .or_insert_with(Vec::new)
                .push(hash);
        }
    }
    pub fn get_block(&self, hash: &str) -> Option<&Block> {
        self.blocks.get(hash)
    }
    pub fn get_block_by_queueindex(&self, index: usize) -> Option<&Block> {
        self.blocks.values().nth(index)
    }
    pub fn get_children(&self, parent_hash: &str) -> Option<&Vec<String>> {
        self.children_map.get(parent_hash)
    }

    pub fn blocks_at_level(&self, level: u32) -> Vec<&String> {
        let mut result = Vec::new();

        for block in self.blocks.values() {
            if block.level == level {
                result.push(&block.hash);
            }
        }
        result
    }

    pub fn mine_block_demo_2(
        &mut self,
        pqp: &mut PQP,
        mut align: u8,
        tx: Vec<String>,
        miner_address: String,
        signature: String,
        abort: &Arc<SyncMutex<bool>>,
    ) -> Option<Block> {
        if align == 0 || align > CHILDREN {
            return None;
        }
        let current_parent = pqp
            .current_parent()
            .expect("No current parent PQP entry found");
        println!("\n current_parent hash: {:?}", current_parent.block_hash);

        let (queue_index, align, prev_pqp) = match self.calculate_queue_index(pqp, align) {
            Some(result) => result,
            None => return None,
        };

        let pqp_entry = PQPEntry {
            queue_index,
            miner_address: miner_address.clone(),
            prev_pqp_commitment: prev_pqp,
            signature: signature.clone(),
        };

        let parent_block_hash = &current_parent.block_hash.clone();
        println!("got parent block hash");
        let parent_block = self
            .get_block(parent_block_hash)
            .expect("failed to get block");
        println!(
            "parent block: level:{}, postion:{},queue_index:{}",
            parent_block.level, parent_block.position, parent_block.pqp_entry.queue_index
        );
        let merkle_root = Block::merkle_root(tx.clone());

        let target = Block::calculate_target(BITS.to_string()).expect("Invalid bits");

        let mut new_block = Block::new(
            "".to_string(),
            "".to_string(),
            parent_block.level + 1,
            format!("{}.{}", parent_block.position, align),
            1,
            parent_block_hash.clone(),
            merkle_root,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as u128,
            BITS.to_string(),
            0,
            align,
            pqp_entry,
            tx.len() as u32,
            tx.clone(),
        );
        let mut nonce = 0;

        loop {
            new_block.nonce = nonce;
            let mut candidate = new_block.clone();
            Block::calculate_hash_and_pqp_commitment(&mut candidate);
            if let Ok(bytes) = hex::decode(&candidate.hash) {
                let val = BigUint::from_bytes_le(&bytes);
                if val < target {
                    let new_pqp_entry = ParentQueueEntry::new(
                        candidate.pqp_entry.queue_index,
                        candidate.align,
                        candidate.hash.clone(),
                        candidate.parent_hash.clone(),
                        candidate.pqp_entry.miner_address.clone(),
                        candidate.pqp_entry.prev_pqp_commitment.clone(),
                        candidate.pqp_entry.signature.clone(),
                        candidate.pqp_commitment.clone(),
                    );
                    pqp.add_entry_to_pqp(new_pqp_entry.clone(), &self);
                    let exist: bool = pqp
                        .pool
                        .iter()
                        .rev()
                        .take(CHILDREN as usize)
                        .any(|e| e.block_hash == new_pqp_entry.block_hash);
                    if exist {
                        println!("\nNew PQP entry added: {:?}", new_pqp_entry);
                        self.add_block(candidate.clone());
                        return Some(candidate);
                    } else {
                        println!("❌ Failed :  PQP entry mismatch");
                        return None;
                    }
                }
            }

            nonce = nonce.wrapping_add(1);
            if nonce % 1000 == 0 {
                if let Ok(guard) = abort.lock() {
                    if *guard {
                        println!("Mining aborted due to received block");
                        return None;
                    }
                }
            }
            if nonce == u32::MAX {
                println!("❌ Failed :  Nonce Exhausted");
                return None;
            }
        }
    }

    pub fn mine_block_demo(
        &mut self,
        pqp: &mut PQP,
        mut align: u8,
        tx: Vec<String>,
        miner_address: String,
        signature: String,
    ) -> Option<Block> {
        if align == 0 || align > CHILDREN {
            return None; // Invalid alignment
        }
        let current_parent = pqp
            .current_parent()
            .expect("No current parent PQP entry found");
        println!("\n current_parent hash: {:?}", current_parent.block_hash);

        let (queue_index, align, prev_pqp) = match self.calculate_queue_index(pqp, align) {
            Some(result) => result,
            None => return None,
        };

        let pqp_entry = PQPEntry {
            queue_index,
            miner_address: miner_address.clone(),
            prev_pqp_commitment: prev_pqp,
            signature: signature.clone(),
        };

        let parent_block_hash = &current_parent.block_hash.clone();
        println!("got parent block hash");
        let parent_block = self
            .get_block(parent_block_hash)
            .expect("failed to get block");
        println!(
            "parent block: level:{}, postion:{},queue_index:{}",
            parent_block.level, parent_block.position, parent_block.pqp_entry.queue_index
        );
        let merkle_root = Block::merkle_root(tx.clone());

        let target = Block::calculate_target(BITS.to_string()).expect("Invalid bits");

        let mut new_block = Block::new(
            "".to_string(),
            "".to_string(),
            parent_block.level + 1,
            format!("{}.{}", parent_block.position, align),
            1,
            parent_block_hash.clone(),
            merkle_root,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as u128,
            BITS.to_string(),
            0,
            align,
            pqp_entry,
            tx.len() as u32,
            tx.clone(),
        );
        let mut nonce = 0;

        loop {
            new_block.nonce = nonce;
            let mut candidate = new_block.clone();
            Block::calculate_hash_and_pqp_commitment(&mut candidate);
            if let Ok(bytes) = hex::decode(&candidate.hash) {
                let val = BigUint::from_bytes_le(&bytes);
                if val < target {
                    let new_pqp_entry = ParentQueueEntry::new(
                        candidate.pqp_entry.queue_index,
                        candidate.align,
                        candidate.hash.clone(),
                        candidate.parent_hash.clone(),
                        candidate.pqp_entry.miner_address.clone(),
                        candidate.pqp_entry.prev_pqp_commitment.clone(),
                        candidate.pqp_entry.signature.clone(),
                        candidate.pqp_commitment.clone(),
                    );
                    pqp.add_entry_to_pqp(new_pqp_entry.clone(), &self);
                    let exist: bool = pqp
                        .pool
                        .iter()
                        .rev()
                        .take(CHILDREN as usize)
                        .any(|e| e.block_hash == new_pqp_entry.block_hash);
                    if exist {
                        println!("\nNew PQP entry added: {:?}", new_pqp_entry);
                        self.add_block(candidate.clone());
                        return Some(candidate);
                    } else {
                        println!("❌ Failed :  PQP entry mismatch");
                        return None;
                    }
                }
            }

            nonce = nonce.wrapping_add(1);
            if nonce == u32::MAX {
                println!("❌ Failed :  Nonce Exhausted");
                return None;
            }
        }
    }

    pub fn calculate_qi(&self, pqp: &PQP, mut align: u8) -> Option<(u32, String, String)> {
        // println!("calculate_qi called with align: {}", align);
        if align == 0 || align > CHILDREN {
            return None;
        }
        let current_parent = pqp
            .current_parent()
            .expect("No current parent PQP entry found");
        let latest_pqp = pqp.latest()?;
        println!("\n current_parent hash: {:?}", current_parent.block_hash);

        let sibling_aligns: Vec<u8> = pqp
            .pool
            .iter()
            .rev()
            .take(CHILDREN as usize)
            .filter(|e| e.parent_hash == current_parent.block_hash)
            .filter_map(|e| self.get_block(&e.block_hash))
            .map(|block| block.align)
            .collect();

        if sibling_aligns.contains(&align) {
            //this should be changed to be changed to next parent thing
            let next_parent = pqp.next_parent();
            if next_parent.is_none() {
                return None;
            }
            let prev_pqp = pqp.get_prev_pqp_commitment(align, &self);
            let mut max_queue_index = latest_pqp.queue_index;
            if max_queue_index % CHILDREN as u32 != 0 {
                max_queue_index += ((CHILDREN as u32) - (max_queue_index % CHILDREN as u32));
            }
            let base_index = max_queue_index;
            let queue_index = base_index + align as u32;
            println!("returned {},{}", queue_index, prev_pqp);
            return Some((
                queue_index,
                prev_pqp,
                next_parent.unwrap().block_hash.clone(),
            ));
        }

        let prev_pqp = pqp.get_prev_pqp_commitment(align, &self);
        let mut max_queue_index = latest_pqp.queue_index;
        if max_queue_index % CHILDREN as u32 != 0 {
            max_queue_index += ((CHILDREN as u32) - (max_queue_index % CHILDREN as u32));
        }
        let base_index = if latest_pqp.parent_hash == current_parent.block_hash {
            max_queue_index - CHILDREN as u32
        } else {
            max_queue_index
        };

        let queue_index = base_index + align as u32;
        Some((queue_index, prev_pqp, current_parent.block_hash.clone()))
    }

    pub fn calculate_queue_index(&self, pqp: &PQP, mut align: u8) -> Option<(u32, u8, String)> {
        let current_parent = pqp
            .current_parent()
            .expect("No current parent PQP entry found");
        let latest_pqp = pqp.latest()?;
        println!("\n current_parent hash: {:?}", current_parent.block_hash);

        // Collect sibling aligns
        let sibling_aligns: Vec<u8> = pqp
            .pool
            .iter()
            .rev()
            .take(CHILDREN as usize)
            .filter(|e| e.parent_hash == current_parent.block_hash)
            .filter_map(|e| self.get_block(&e.block_hash))
            .map(|block| block.align)
            .collect();

        // If requested align is already taken, pick first available
        if sibling_aligns.contains(&align) {
            let mut chosen_align = None;
            for candidate in 1..=CHILDREN {
                if !sibling_aligns.contains(&candidate) {
                    chosen_align = Some(candidate);
                    break;
                }
            }
            if let Some(new_align) = chosen_align {
                align = new_align;
            } else {
                // No available align; cannot mine new sibling block
                return None;
            }
        }

        let prev_pqp = pqp.get_prev_pqp(align);
        let mut max_queue_index = latest_pqp.queue_index;
        if max_queue_index % CHILDREN as u32 != 0 {
            max_queue_index += ((CHILDREN as u32) - (max_queue_index % CHILDREN as u32));
        }

        // Determine base index depending on parent relationship
        let base_index = if latest_pqp.parent_hash == current_parent.block_hash {
            pqp.pool
                .iter()
                .rev()
                .find(|entry| entry.parent_hash != current_parent.block_hash)
                .map(|entry| entry.queue_index)
                .unwrap_or(max_queue_index)
        } else {
            max_queue_index
        };

        let queue_index = base_index + align as u32;

        println!(
            "\n base_index: {}, max_queue_index: {}, align: {}, latest_pqp.queue_index: {}, queue_index: {}",
            base_index, max_queue_index, align, latest_pqp.queue_index, queue_index
        );

        Some((queue_index, align, prev_pqp))
    }

    pub fn is_valid_tree(&self, pqp: &PQP) -> bool {
        println!("is valid tree started with length: {}", self.blocks.len());
        for entry in &pqp.pool {
            let mut current_hash = entry.block_hash.clone();
            loop {
                let block_opt = self.get_block(&current_hash);
                let block = match block_opt {
                    Some(b) => b.clone(),
                    None => {
                        println!("❌ Block not found for hash: {}", current_hash);
                        return false;
                    }
                };
                let genesis = Block::genesis();
                if block.hash == genesis.hash {
                    break;
                }

                let mut base_index = if block.pqp_entry.queue_index >= CHILDREN as u32 {
                    (block.pqp_entry.queue_index - CHILDREN as u32) as usize
                } else {
                    0
                };
                println!("base_index: {}", base_index);

                let mut prev_pqp_block = None;
                let mut prev_pqp_block_hash = String::new();

                // Check if block at (queue_index - CHILDREN) exists
                if block.pqp_entry.queue_index >= CHILDREN as u32 {
                    if let Some((hash, base_block)) = self.blocks.get_index(base_index) {
                        if !hash.starts_with("placeholder_") {
                            prev_pqp_block = Some(base_block);
                            prev_pqp_block_hash = hash.clone();
                        } else {
                            // If block at base_index is a placeholder, adjust base_index
                            let mut num = block.pqp_entry.queue_index - CHILDREN as u32;
                            // Increase num to the next multiple of CHILDREN if not already
                            if num % CHILDREN as u32 != 0 {
                                num += (CHILDREN as u32) - (num % CHILDREN as u32);
                            }
                            base_index = num as usize;
                            let mut attempts = 0;
                            // Decrease index up to CHILDREN times until a non-placeholder block is found
                            while attempts < CHILDREN as usize {
                                if let Some((hash, base_block)) = self.blocks.get_index(base_index)
                                {
                                    if !hash.starts_with("placeholder_") {
                                        prev_pqp_block = Some(base_block);
                                        prev_pqp_block_hash = hash.clone();
                                        break;
                                    }
                                }
                                if base_index == 0 {
                                    break;
                                }
                                base_index = base_index.saturating_sub(1);
                                attempts += 1;
                            }
                        }
                    }
                }

                // Set base_index to the queue_index of the found non-placeholder block
                base_index = prev_pqp_block
                    .map(|b| b.pqp_entry.queue_index as usize)
                    .unwrap_or(0);
                println!(
                    "base_index adjusted to: {} for block with queue_index: {}",
                    base_index, block.pqp_entry.queue_index
                );

                if let Some((hash, base_block)) = self.blocks.get_index(base_index) {
                    prev_pqp_block = Some(base_block);
                    prev_pqp_block_hash = hash.clone();

                    // Traverse backward to find a block with matching align and same parent_hash
                    for idx in (0..base_index + 1).rev() {
                        if let Some((hash, candidate_block)) = self.blocks.get_index(idx) {
                            if candidate_block.align == block.align
                                && candidate_block.parent_hash
                                    == prev_pqp_block.unwrap().parent_hash
                            {
                                prev_pqp_block = Some(candidate_block);
                                prev_pqp_block_hash = hash.clone();
                                break;
                            } else if candidate_block.parent_hash
                                != prev_pqp_block.unwrap().parent_hash
                            {
                                break;
                            }
                        }
                    }
                }

                let (prev_pqp_block_hash, prev_pqp_block) = match prev_pqp_block {
                    Some(block) => (prev_pqp_block_hash, block),
                    None => {
                        println!("❌ Failed to get base block at index {}", base_index);
                        return false;
                    }
                };

                if prev_pqp_block.pqp_commitment != block.pqp_entry.prev_pqp_commitment {
                    println!(
                        "❌ Previous PQP commitment mismatch for block {} \nExpected: {} \nFound: {}",
                        block.hash,
                        prev_pqp_block.pqp_commitment,
                        block.pqp_entry.prev_pqp_commitment
                    );
                    return false;
                }

                let mut candidate = block.clone();
                Block::calculate_hash_and_pqp_commitment(&mut candidate);

                if candidate.hash != block.hash {
                    println!(
                        "❌ Hash mismatch for block {} \nExpected: {} \nFound:    {}",
                        current_hash, block.hash, candidate.hash
                    );
                    return false;
                }

                if candidate.pqp_commitment != block.pqp_commitment {
                    println!(
                        "❌ PQP commitment mismatch for block {} \nExpected: {} \nFound:    {}",
                        current_hash, block.pqp_commitment, candidate.pqp_commitment
                    );
                    return false;
                }

                current_hash = block.parent_hash.clone();
            }
        }
        println!("✅ Tree and PQP validated successfully");
        true
    }

    // pub fn is_valid_tree(&self, pqp: &PQP) -> bool {
    //     println!("is valid tree started with length: {}", self.blocks.len());
    //     for entry in &pqp.pool {
    //         let mut current_hash = entry.block_hash.clone();
    //         loop {
    //             let block_opt = self.get_block(&current_hash);
    //             let block = match block_opt {
    //                 Some(b) => b.clone(),
    //                 None => {
    //                     println!("❌ Block not found for hash: {}", current_hash);
    //                     return false;
    //                 }
    //             };
    //             let genesis = Block::genesis();
    //             if block.hash == genesis.hash {
    //                 break;
    //             }

    //             let mut base_index = if block.pqp_entry.queue_index >= CHILDREN as u32 {
    //                 (block.pqp_entry.queue_index - CHILDREN as u32) as usize
    //             } else {
    //                 0
    //             };

    //             let mut prev_pqp_block = None;
    //             let mut prev_pqp_block_hash = String::new();
    //             let mut attempts = 0;

    //             while attempts < CHILDREN as usize {
    //                 if let Some((hash, base_block)) = self.blocks.get_index(base_index) {
    //                     if !hash.starts_with("placeholder_") {
    //                         prev_pqp_block = Some(base_block);
    //                         prev_pqp_block_hash = hash.clone();
    //                         break;
    //                     }
    //                     if base_index == 0 {
    //                         break;
    //                     }
    //                     base_index = base_index.saturating_sub(1);
    //                     attempts += 1;
    //                 } else {
    //                     break;
    //                 }
    //             }

    //             base_index = prev_pqp_block
    //                 .map(|b| b.pqp_entry.queue_index as usize)
    //                 .unwrap_or(0);

    //             if let Some((hash, base_block)) = self.blocks.get_index(base_index) {
    //                 prev_pqp_block = Some(base_block);
    //                 prev_pqp_block_hash = hash.clone();

    //                 for idx in (0..base_index + 1).rev() {
    //                     if let Some((hash, candidate_block)) = self.blocks.get_index(idx) {
    //                         if candidate_block.align == block.align
    //                             && candidate_block.parent_hash
    //                                 == prev_pqp_block.unwrap().parent_hash
    //                         {
    //                             prev_pqp_block = Some(candidate_block);
    //                             prev_pqp_block_hash = hash.clone();
    //                             break;
    //                         } else if candidate_block.parent_hash
    //                             != prev_pqp_block.unwrap().parent_hash
    //                         {
    //                             break;
    //                         }
    //                     }
    //                 }
    //             }

    //             let (prev_pqp_block_hash, prev_pqp_block) = match prev_pqp_block {
    //                 Some(block) => (prev_pqp_block_hash, block),
    //                 None => {
    //                     println!("❌ Failed to get base block at index {}", base_index);
    //                     return false;
    //                 }
    //             };

    //             if prev_pqp_block.pqp_commitment != block.pqp_entry.prev_pqp_commitment {
    //                 println!(
    //                     "❌ Previous PQP commitment mismatch for block {} \nExpected: {} \nFound: {}",
    //                     block.hash,
    //                     prev_pqp_block.pqp_commitment,
    //                     block.pqp_entry.prev_pqp_commitment
    //                 );
    //                 return false;
    //             }

    //             let mut candidate = block.clone();
    //             Block::calculate_hash_and_pqp_commitment(&mut candidate);

    //             if candidate.hash != block.hash {
    //                 println!(
    //                     "❌ Hash mismatch for block {} \nExpected: {} \nFound:    {}",
    //                     current_hash, block.hash, candidate.hash
    //                 );
    //                 return false;
    //             }

    //             if candidate.pqp_commitment != block.pqp_commitment {
    //                 println!(
    //                     "❌ PQP commitment mismatch for block {} \nExpected: {} \nFound:    {}",
    //                     current_hash, block.pqp_commitment, candidate.pqp_commitment
    //                 );
    //                 return false;
    //             }

    //             current_hash = block.parent_hash.clone();
    //         }
    //     }
    //     println!("✅ Tree and PQP validated successfully");
    //     true
    // }

    pub fn is_valid_pqp(&self, pqp: &PQP) -> bool {
        let pool = &pqp.pool;
        if pool.is_empty() {
            println!("❌ PQP pool is empty");
            return false;
        }
        // for entry in pqp.pool.iter().rev() {
        //     println!(
        //         "\n q_i: {} p_h: {} h: {} ",
        //         entry.queue_index, entry.parent_hash, entry.block_hash
        //     );
        // }

        // step 1: Verify each PQP entry in reverse order
        for (i, entry) in pool.iter().rev().enumerate() {
            let block_opt = self.get_block(&entry.block_hash);
            let block = match block_opt {
                Some(b) => b,
                None => {
                    println!("❌ Block not found for PQP entry: {}", entry.block_hash);
                    return false;
                }
            };
            //step 2 recalculate pqp_commitment

            let mut pqp_hasher = Sha256::new();
            pqp_hasher.update(entry.queue_index.to_le_bytes());
            pqp_hasher.update(&[block.align]);
            pqp_hasher.update(hex::decode(&block.hash).unwrap_or_default());
            pqp_hasher.update(hex::decode(&entry.parent_hash).unwrap_or_default());
            pqp_hasher.update(entry.miner_address.as_bytes());
            pqp_hasher.update(hex::decode(&entry.prev_pqp_commitment).unwrap_or_default());
            pqp_hasher.update(hex::decode(&entry.signature).unwrap_or_default());

            let computed_commitment = hex::encode(pqp_hasher.finalize());
            if computed_commitment != entry.pqp_commitment {
                println!(
                    "❌ PQP commitment mismatch for entry at queue_index {}: \nExpected: {} \nFound:    {}",
                    entry.queue_index, entry.pqp_commitment, computed_commitment
                );
                return false;
            }

            // Step 3: Signature verification - To be added later
            if i + 1 < pool.len() {
                let prev_entry = &pool[pool.len() - 1 - (i + 1)];
                if prev_entry.queue_index >= entry.queue_index {
                    println!(
                        "❌ Invalid queue index order: {} >= {}",
                        prev_entry.queue_index, entry.queue_index
                    );
                    return false;
                }

                if entry.parent_hash != prev_entry.parent_hash {
                    let mut prev_siblings = vec![];
                    if let Some(child_hashes) = self.children_map.get(&prev_entry.parent_hash) {
                        for hash in child_hashes {
                            if let Some(block) = self.get_block(hash) {
                                // Optional: skip placeholder blocks via `block.position.is_empty()`
                                let pqp_entry = TreeChain::parent_queue_entry_from_block(block);
                                prev_siblings.push(pqp_entry);
                            }
                        }
                    }
                    // println!("prev siblings: {:?}", prev_siblings);
                    let mut found = false;
                    for sib in &prev_siblings {
                        if entry.align == sib.align {
                            if entry.prev_pqp_commitment != sib.pqp_commitment {
                                println!(
                                    "failed at entry.prev_pqp_commitment != sib.pqp_commitment "
                                );
                                return false;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        if let Some(max_sibling) =
                            prev_siblings.iter().max_by_key(|e| e.queue_index)
                        {
                            if entry.prev_pqp_commitment != max_sibling.pqp_commitment {
                                println!("entry: {:?}", entry);
                                println!("prev siblings: {:?}", prev_siblings);
                                println!(
                                    "failed at entry.prev_pqp_commitment != max_sibling.pqp_commitment"
                                );
                                return false;
                            }
                        }
                    }
                } else {
                    let mut ancestor_opt = None;
                    for ancestor in pqp.pool.iter().rev() {
                        if ancestor.parent_hash != entry.parent_hash
                            && (ancestor.queue_index < entry.queue_index)
                        {
                            ancestor_opt = Some(ancestor);
                            break;
                        }
                    }
                    // println!("anc:{}", ancestor_opt.unwrap().parent_hash);
                    if let Some(ancestor) = ancestor_opt {
                        let mut siblings = vec![];
                        if let Some(child_hashes) = self.children_map.get(&ancestor.parent_hash) {
                            for hash in child_hashes {
                                if let Some(block) = self.get_block(hash) {
                                    // Optional: skip placeholder blocks via `block.position.is_empty()`
                                    let pqp_entry = TreeChain::parent_queue_entry_from_block(block);
                                    siblings.push(pqp_entry);
                                }
                            }
                        }

                        let mut found = false;
                        for sib in &siblings {
                            if entry.align == sib.align {
                                if entry.prev_pqp_commitment != sib.pqp_commitment {
                                    println!(
                                        "entry q_i: {} sib q_i:{} ; entry a_i: {} sib a_i:{}",
                                        entry.queue_index, sib.queue_index, entry.align, sib.align
                                    );
                                    println!("failed here 1");
                                    return false;
                                }
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            if let Some(max_sibling) = siblings.iter().max_by_key(|e| e.queue_index)
                            {
                                if entry.prev_pqp_commitment != max_sibling.pqp_commitment {
                                    println!("siblings: {:?}", siblings);
                                    println!(
                                        "entry q_i: {} maxsib q_i:{} ; entry a_i: {} maxsib a_i:{}",
                                        entry.queue_index,
                                        max_sibling.queue_index,
                                        entry.align,
                                        max_sibling.align
                                    );
                                    println!("failed here 2");
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("✅ PQP entries validated successfully");
        // step 6: Verify PQP entries using blocks in tree

        let blocks_len = self.blocks.len();
        if blocks_len == 0 {
            println!("❌ No blocks found in the tree");
            return false;
        }

        let (mut current_hash, current_block) = match self.blocks.get_index(blocks_len - 1) {
            Some(pair) => pair,
            None => {
                println!("❌ Failed to get last block");
                return false;
            }
        };
        let target_parent_hash = &current_block.parent_hash;

        let mut collected_pqp_entries: Vec<ParentQueueEntry> = Vec::new();
        let mut target_parent_children_count: u8 = 1;

        let binding = Self::parent_queue_entry_from_block(current_block);
        collected_pqp_entries.push(binding);
        for idx in (0..blocks_len - 1).rev() {
            let (hash, block) = match self.blocks.get_index(idx) {
                Some(pair) => pair,
                None => {
                    println!("❌ Failed to get block at index {}", idx);
                    return false;
                }
            };

            if block.parent_hash == *target_parent_hash {
                target_parent_children_count += 1;
            }

            if hash == target_parent_hash {
                if target_parent_children_count < CHILDREN {
                    collected_pqp_entries.insert(0, Self::parent_queue_entry_from_block(block));
                }
                break;
            }

            // Skip placeholder blocks if hash is empty string
            if block.position.is_empty() {
                continue;
            }

            // Insert each extracted ParentQueueEntry at front (to keep order old→new)
            collected_pqp_entries.insert(0, Self::parent_queue_entry_from_block(block));
        }

        // 6. Compare collected PQP entries with the last N entries in PQP.pool
        let recent_count = collected_pqp_entries.clone().len();
        let pool_recent = pool;

        if pool_recent.len() != recent_count {
            println!("❌ Mismatch in PQP entries count collected vs pool");
            return false;
        }

        for (collected_entry, pool_entry) in collected_pqp_entries.iter().zip(pool_recent.iter()) {
            if *collected_entry != *pool_entry {
                println!(
                    "❌ PQP entry mismatch at queue_index {}",
                    collected_entry.queue_index
                );
                return false;
            }
        }

        println!("✅ PQP entries in tree match the current PQP pool");
        true
    }

    pub fn extract_pqp_from_tree(&self) -> PQP {
        let mut pqp = PQP::new();
        let blocks_len = self.blocks.len();
        if blocks_len == 0 {
            println!("❌ No blocks found in the tree");
            pqp.pool = vec![];
            return pqp;
        }

        let (mut current_hash, current_block) = match self.blocks.get_index(blocks_len - 1) {
            Some(pair) => pair,
            None => {
                println!("❌ Failed to get last block");
                pqp.pool = vec![];
                return pqp;
            }
        };
        let target_parent_hash = &current_block.parent_hash;
        let mut collected_pqp_entries: Vec<ParentQueueEntry> = Vec::new();
        let mut target_parent_children_count: u8 = 1;

        let binding = Self::parent_queue_entry_from_block(current_block);
        collected_pqp_entries.push(binding);

        for idx in (0..blocks_len - 1).rev() {
            let (hash, block) = match self.blocks.get_index(idx) {
                Some(pair) => pair,
                None => {
                    println!("❌ Failed to get block at index {}", idx);
                    pqp.pool = vec![];
                    return pqp;
                }
            };

            if block.parent_hash == *target_parent_hash {
                target_parent_children_count += 1;
            }

            if hash == target_parent_hash {
                if target_parent_children_count < CHILDREN {
                    collected_pqp_entries.insert(0, Self::parent_queue_entry_from_block(block));
                }
                break;
            }

            // Skip placeholder blocks if hash is empty string
            if block.position.is_empty() {
                continue;
            }

            // Insert each extracted ParentQueueEntry at front (to keep order old→new)
            collected_pqp_entries.insert(0, Self::parent_queue_entry_from_block(block));
        }
        pqp.pool = collected_pqp_entries;
        return pqp;
    }

    pub fn parent_queue_entry_from_block(block: &Block) -> ParentQueueEntry {
        ParentQueueEntry {
            queue_index: block.pqp_entry.queue_index,
            align: block.align,
            block_hash: block.hash.clone(),
            parent_hash: block.parent_hash.clone(),
            miner_address: block.pqp_entry.miner_address.clone(),
            prev_pqp_commitment: block.pqp_entry.prev_pqp_commitment.clone(),
            signature: block.pqp_entry.signature.clone(),
            pqp_commitment: block.pqp_commitment.clone(),
        }
    }

    pub fn verify_and_add_block(&mut self, block: &Block) -> bool {
        // Verify the block hash and PQP commitment
        if !block.verify_hash_pqp_commitment() {
            println!("❌ Block verification failed for hash: {}", block.hash);
            return false;
        }
        // Optionally: verify that parent block exists
        if block.level != 0 && !self.blocks.contains_key(&block.parent_hash) {
            println!("❌ Parent block missing for block: {}", block.hash);
            return false;
        }
        self.add_block(block.clone());
        true
    }

    pub fn prepare_block_template(
        &self,
        pqp: &mut PQP,
        align: u8,
        tx: Vec<String>,
        miner_address: String,
        signature: String,
    ) -> Option<Block> {
        let calc = self.calculate_qi(pqp, align);
        if let Some((queue_index, prev_pqp, parent_hash)) = calc {
            // Assuming this is how you get parent (from current PQP)
            // let parent_entry = match pqp.current_parent() {
            //     Some(entry) => entry.clone(),
            //     None => return None,
            // };
            // let parent_hash = parent_entry.block_hash;

            // Get parent block to calculate level/position (adjust if your logic differs)
            let parent_block = match self.blocks.get(&parent_hash.clone()) {
                Some(block) => block.clone(),
                None => return None,
            };

            let level = parent_block.level + 1;

            // Assume a method to calculate position; replace with your actual logic (e.g., based on align/parent.position)
            let position = format!("{}.{}", parent_block.position, align); // Placeholder; adjust

            let merkle_root = Block::merkle_root(tx.clone());

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_millis();

            let bits = crate::config::BITS.to_string(); // Or however bits are determined

            let pqp_entry = PQPEntry {
                queue_index,
                miner_address,
                prev_pqp_commitment: prev_pqp,
                signature,
            };

            let mut block = Block::new(
                "".to_string(), // hash (computed later)
                "".to_string(), // pqp_commitment (computed later)
                level,
                position,
                1, // version
                parent_hash,
                merkle_root,
                timestamp,
                bits,
                0, // nonce starts at 0
                align,
                pqp_entry,
                tx.len() as u32,
                tx,
            );

            Some(block)
        } else {
            None
        }
    }
}
