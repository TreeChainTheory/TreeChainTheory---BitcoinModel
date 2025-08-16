use core::hash;
use num_bigint::BigUint;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PQPEntry {
    pub queue_index: u32,
    pub miner_address: String,
    pub signature: String,
    pub prev_pqp_commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub hash: String,
    pub pqp_commitment: String,
    pub level: u32,
    pub position: String,
    pub version: u32,
    pub parent_hash: String,
    pub merkle_root: String,
    pub timestamp: u128,
    pub bits: String,
    pub nonce: u32,
    pub align: u8,

    pub pqp_entry: PQPEntry,

    pub nTx: u32,
    pub tx: Vec<String>,
}

impl Block {
    pub fn new(
        hash: String,
        pqp_commitment: String,
        level: u32,
        position: String,
        version: u32,
        parent_hash: String,
        merkle_root: String,
        timestamp: u128,
        bits: String,
        nonce: u32,
        align: u8,
        pqp_entry: PQPEntry,
        nTx: u32,
        tx: Vec<String>,
    ) -> Self {
        Self {
            hash,
            pqp_commitment,
            level,
            position,
            version,
            parent_hash,
            merkle_root,
            timestamp,
            bits,
            nonce,
            align,
            pqp_entry,
            nTx,
            tx,
        }
    }

    pub fn genesis() -> Block {
        Block::new(
            "022c8507555c43ce9f4829631618e4beb13e94ae8254bc8ba6bb9feb938128f5".to_string(),
            "c72b1565891e4143ff7ff2f6572f4f268d54ded2738ae1d2e5a3dc079565f8a4".to_string(),
            0,
            "0".to_string(),
            1,
            "00".repeat(32),
            "00".repeat(32),
            0,
            "1d00ffff".to_string(),
            0,
            0,
            PQPEntry {
                queue_index: 0,
                miner_address: "GENISIS_LEADER_HEX".to_string(),
                signature: "".repeat(64),
                prev_pqp_commitment: "00".repeat(32),
            },
            0,
            vec![],
        )
    }

    pub fn calculate_hash_and_pqp_commitment(block: &mut Block) {
        // --- Step 1: Calculate Block Hash ---
        let mut hasher = Sha256::new();
        hasher.update(block.level.to_le_bytes());
        hasher.update(block.position.as_bytes());
        hasher.update(block.version.to_le_bytes());
        hasher.update(hex::decode(&block.parent_hash).unwrap_or_default());
        hasher.update(hex::decode(&block.merkle_root).unwrap_or_default());
        hasher.update(block.timestamp.to_le_bytes());
        hasher.update(hex::decode(&block.bits).unwrap_or_default());
        hasher.update(block.nonce.to_le_bytes());
        hasher.update(&[block.align]);

        hasher.update(block.pqp_entry.queue_index.to_le_bytes());
        hasher.update(block.pqp_entry.miner_address.as_bytes());
        hasher.update(hex::decode(&block.pqp_entry.signature).unwrap_or_default());
        hasher.update(hex::decode(&block.pqp_entry.prev_pqp_commitment).unwrap_or_default());
        hasher.update(block.nTx.to_le_bytes());
        for tx in &block.tx {
            hasher.update(tx.as_bytes());
        }

        block.hash = hex::encode(hasher.finalize());
        // --- Step 2: Calculate PQP Commitment ---
        let mut pqp_hasher = Sha256::new();
        pqp_hasher.update(block.pqp_entry.queue_index.to_le_bytes());
        pqp_hasher.update(hex::decode(&block.hash).unwrap_or_default());
        pqp_hasher.update(hex::decode(&block.parent_hash).unwrap_or_default());
        pqp_hasher.update(block.pqp_entry.miner_address.as_bytes());
        pqp_hasher.update(hex::decode(&block.pqp_entry.signature).unwrap_or_default());
        pqp_hasher.update(hex::decode(&block.pqp_entry.prev_pqp_commitment).unwrap_or_default());

        block.pqp_commitment = hex::encode(pqp_hasher.finalize());
    }

    //next  mine_block_example( parentblock , align ,bits , pqp_entry , tx ) , merkle_root(tx) , calculate_target(bits),
    pub fn mine_block_example(
        parent_block: &Block,
        align: u8,
        bits: String,
        pqp_entry: PQPEntry,
        tx: Vec<String>,
    ) -> Block {
        let markle_root = Block::merkle_root(tx.clone());
        let mut new_block = Block::new(
            "".to_string(),
            "00".repeat(32),
            parent_block.level + 1,
            format!("{}.{}", parent_block.position, align),
            1,
            parent_block.hash.clone(),
            markle_root,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as u128,
            bits.clone(),
            0,
            align,
            pqp_entry,
            tx.len() as u32,
            tx,
        );

        let target = Block::calculate_target(bits).expect("Invalid bits");

        let mut nonce = 0;
        loop {
            new_block.nonce = nonce;
            let mut candidate = new_block.clone();
            Block::calculate_hash_and_pqp_commitment(&mut candidate);

            if let Ok(bytes) = hex::decode(&candidate.hash) {
                let mut val = BigUint::from_bytes_le(&bytes);
                if val < target {
                    return candidate;
                }
            }

            nonce = nonce.wrapping_add(1);
            if nonce == u32::MAX {
                panic!("Nonce overflow, unable to find a valid block hash");
            }
        }
    }

    pub fn merkle_root(tx: Vec<String>) -> String {
        if tx.is_empty() {
            return "0".repeat(64);
        }
        let mut hashes = tx;
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            let mut i = 0;
            while i < hashes.len() {
                let left = &hashes[i];
                let right = if i + 1 < hashes.len() {
                    &hashes[i + 1]
                } else {
                    left
                };
                let concat = format!("{}{}", left, right);
                let bytes = hex::decode(&concat).unwrap();
                let hash = Sha256::digest(&bytes);
                let hash2 = Sha256::digest(&hash);
                next_level.push(hex::encode(hash2));
                i += 2;
            }
            hashes = next_level;
        }
        hashes[0].clone()
    }

    pub fn calculate_target(bits: String) -> Option<BigUint> {
        let bytes = hex::decode(bits).ok()?;
        if bytes.len() != 4 {
            return None;
        }
        let exponent = bytes[0] as usize;
        let mantissa = ((bytes[1] as u32) << 16) | ((bytes[2] as u32) << 8) | (bytes[3] as u32);
        let mut target = BigUint::from_u32(mantissa)?;
        if exponent > 3 {
            target = target << (8 * (exponent - 3));
        } else if exponent < 3 {
            target = target >> (8 * (3 - exponent));
        }
        Some(target)
    }
}
