use crate::config::{EXPECTED_TIME, HALVING_INTERVAL, INITIAL_SUBSIDY};
use crate::wallet::transaction::Transaction;
use num_bigint::BigUint;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Represents an entry in the Pending Queue of Parents (PQP).
/// Each block includes a reference to one PQP entry, which ensures
/// that block creation is tied to a miner's eligibility
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PQPEntry {
    pub queue_index: u32,
    /// The miner's public identity (in this prototype a hex string,
    /// but in a real implementation this would be raw bytes or a public key type).
    pub miner_address: String,
    /// Commitment to the previous PQP state (hex-encoded string here,
    /// normally a `[u8; 32]` or `Vec<u8>` in production).
    pub prev_pqp_commitment: String,
    /// The miner's digital signature over this PQP entry
    /// (again stored as hex string, but in reality should be raw bytes).
    pub signature: String,
}

/// Represents a blockchain block.
/// Each block contains references to its parent, a set of transactions,
/// a PQP entry, and proof-of-work style fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    /// Block hash (calculated over block contents).
    /// In this prototype: stored as a hex string, but in reality: `[u8; 32]`.
    pub hash: String,
    /// In this prototype: stored as a hex string, but in reality: `[u8; 32]`.
    pub pqp_commitment: String,
    pub level: u32,
    /// Logical position (tree-based placement, not just linear height).
    pub position: String,
    /// Block version (allows protocol upgrades) for now as this is just a prototype its only 1.
    pub version: u32,
    /// Parent block's hash (hex string here, `[u8; 32]` in real-world).
    pub parent_hash: String,
    /// Root of the Merkle tree built from transactions.
    pub merkle_root: String,
    pub timestamp: u128,
    /// Difficulty bits (compact representation of target).
    /// Stored as hex string (real chain: 4 raw bytes).
    pub bits: String,
    pub nonce: u32,
    pub align: u8,
    pub pqp_entry: PQPEntry,

    pub n_tx: u32,

    /// List of transactions (will be changed later after buiilding transactions).
    pub tx: Vec<Transaction>,
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
        n_tx: u32,
        tx: Vec<Transaction>,
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
            n_tx,
            tx,
        }
    }

    pub fn genesis() -> Block {
        Block::new(
            "000011428da0831df234bd3a0f404f575cf14c9c630f9f6b1c54820a9e2e65ed".to_string(),
            "bad9f937944415e9195541968ca121ac25cca5ce29d7dbea629083218ed9d56c".to_string(),
            0,
            "0".to_string(),
            1,
            "00".repeat(32),
            "00".repeat(32),
            0,
            "1e7fffff".to_string(),
            25231,
            0,
            PQPEntry {
                queue_index: 0,
                miner_address: "GENISIS_LEADER_HEX".to_string(),
                prev_pqp_commitment: "00".repeat(32),
                signature: "".repeat(64),
            },
            0,
            vec![],
        )
    }

    pub fn empty_placeholder(queue_index: u32) -> Block {
        // Create a dummy PQPEntry with empty or zeroed fields
        let pqp_entry = PQPEntry {
            queue_index,
            miner_address: "".to_string(),
            prev_pqp_commitment: "".repeat(32),
            signature: "".repeat(64),
        };

        // Use a unique placeholder parent_hash, e.g. all zeros
        let parent_hash = "".to_string();

        // Create the block with other fields as empty or zero
        let block = Block::new(
            "".to_string(), // hash (to be calculated)
            "".to_string(), // pqp_commitment (to be calculated)
            0,              // level
            "".to_string(), // position
            0,              // version
            parent_hash,    // parent_hash
            "".repeat(32),  // merkle_root
            0,              // timestamp
            "".repeat(4),   // bits
            0,              // nonce
            0,              // align
            pqp_entry,      // pqp_entry
            0,              // nTx
            vec![],         // tx
        );

        // Calculate hash and pqp commitment based on placeholder content
        // Block::calculate_hash_and_pqp_commitment(&mut block);

        block
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
        hasher.update(hex::decode(&block.pqp_entry.prev_pqp_commitment).unwrap_or_default());
        hasher.update(hex::decode(&block.pqp_entry.signature).unwrap_or_default());
        hasher.update(block.n_tx.to_le_bytes());
        for tx in &block.tx {
            hasher.update(hex::decode(&tx.txid).unwrap_or_default());
        }

        block.hash = hex::encode(hasher.finalize());
        // --- Step 2: Calculate PQP Commitment ---
        let mut pqp_hasher = Sha256::new();
        pqp_hasher.update(block.pqp_entry.queue_index.to_le_bytes());
        pqp_hasher.update(&[block.align]);
        pqp_hasher.update(hex::decode(&block.hash).unwrap_or_default());
        pqp_hasher.update(hex::decode(&block.parent_hash).unwrap_or_default());
        pqp_hasher.update(block.pqp_entry.miner_address.as_bytes());
        pqp_hasher.update(hex::decode(&block.pqp_entry.prev_pqp_commitment).unwrap_or_default());
        pqp_hasher.update(hex::decode(&block.pqp_entry.signature).unwrap_or_default());

        block.pqp_commitment = hex::encode(pqp_hasher.finalize());
    }

    pub fn verify_hash_pqp_commitment(&self) -> bool {
        // Recalculate hash and PQP commitment locally
        let mut hasher = Sha256::new();
        hasher.update(self.level.to_le_bytes());
        hasher.update(self.position.as_bytes());
        hasher.update(self.version.to_le_bytes());
        hasher.update(hex::decode(&self.parent_hash).unwrap_or_default());
        hasher.update(hex::decode(&self.merkle_root).unwrap_or_default());
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(hex::decode(&self.bits).unwrap_or_default());
        hasher.update(self.nonce.to_le_bytes());
        hasher.update(&[self.align]);

        hasher.update(self.pqp_entry.queue_index.to_le_bytes());
        hasher.update(self.pqp_entry.miner_address.as_bytes());
        hasher.update(hex::decode(&self.pqp_entry.prev_pqp_commitment).unwrap_or_default());
        hasher.update(hex::decode(&self.pqp_entry.signature).unwrap_or_default());
        hasher.update(self.n_tx.to_le_bytes());
        for tx in &self.tx {
            hasher.update(hex::decode(&tx.txid).unwrap_or_default());
        }
        let recalculated_hash = hex::encode(hasher.finalize());

        if recalculated_hash != self.hash {
            return false;
        }

        // Recalculate the PQP commitment
        let mut pqp_hasher = Sha256::new();
        pqp_hasher.update(self.pqp_entry.queue_index.to_le_bytes());
        pqp_hasher.update(&[self.align]);
        pqp_hasher.update(hex::decode(&self.hash).unwrap_or_default());
        pqp_hasher.update(hex::decode(&self.parent_hash).unwrap_or_default());
        pqp_hasher.update(self.pqp_entry.miner_address.as_bytes());
        pqp_hasher.update(hex::decode(&self.pqp_entry.prev_pqp_commitment).unwrap_or_default());
        pqp_hasher.update(hex::decode(&self.pqp_entry.signature).unwrap_or_default());

        let recalculated_pqp_commitment = hex::encode(pqp_hasher.finalize());

        recalculated_pqp_commitment == self.pqp_commitment
    }

    //next  mine_block_example( parentblock , align ,bits , pqp_entry , tx ) , merkle_root(tx) , calculate_target(bits),

    pub fn merkle_root(tx: Vec<Transaction>) -> String {
        if tx.is_empty() {
            return "0".repeat(64);
        }
        let mut hashes: Vec<String> = tx
            .iter()
            .map(|t| {
                // Hash transaction using double SHA-256 of its serialized form
                let serialized = t.serialize_non_witness();
                let hash = Sha256::digest(&serialized); // First SHA-256
                hex::encode(Sha256::digest(hash)) // Second SHA-256
            })
            .collect();

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

                let concat = [
                    hex::decode(left).unwrap_or_default(),
                    hex::decode(right).unwrap_or_default(),
                ]
                .concat();
                let hash = Sha256::digest(&concat);
                let hash2 = Sha256::digest(hash);
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

    pub fn adjust_bits(
        prev_bits: &str,
        first_timestamp: u128,
        last_timestamp: u128,
    ) -> Option<String> {
        // 1. Decode previous target
        let prev_target = Self::calculate_target(prev_bits.to_string())?;

        // 2. Calculate actual time span
        let mut actual_time = (last_timestamp - first_timestamp) as i128;

        // Expected = 2 weeks in seconds
        let expected_time: i128 = EXPECTED_TIME;

        // 3. Clamp adjustment between 1/4x and 4x
        let min_time = expected_time / 4;
        let max_time = expected_time * 4;
        if actual_time < min_time {
            actual_time = min_time;
        }
        if actual_time > max_time {
            actual_time = max_time;
        }

        // 4. Compute new target
        let mut new_target = prev_target * (actual_time as u128);
        new_target /= expected_time as u128;

        // 5. Convert target back into compact bits
        Some(Self::target_to_bits(&new_target))
    }

    pub fn target_to_bits(target: &BigUint) -> String {
        let mut target_bytes = target.to_bytes_be();
        while !target_bytes.is_empty() && target_bytes[0] == 0 {
            target_bytes.remove(0);
        }

        let exponent = target_bytes.len() as u8;
        let mut _mantissa: u32 = 0;

        if exponent <= 3 {
            _mantissa = target_bytes
                .iter()
                .fold(0u32, |acc, &b| (acc << 8) | b as u32);
            _mantissa <<= 8 * (3 - exponent as usize);
        } else {
            _mantissa = ((target_bytes[0] as u32) << 16)
                | ((target_bytes[1] as u32) << 8)
                | (target_bytes[2] as u32);
        }

        let compact: u32 = ((exponent as u32) << 24) | _mantissa;
        format!("{:08x}", compact)
    }

    pub fn adjust_subsidy(height: u64) -> u64 {
        let halvings = height / HALVING_INTERVAL;

        // If subsidy shifts beyond 64 halvings, return 0
        if halvings >= 64 {
            return 0;
        }

        INITIAL_SUBSIDY >> halvings
    }
}
