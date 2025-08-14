use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PQPEntry {
    pub queue_index: u32,
    pub block_hash: String,
    pub parent_hash: String,
    pub miner_address: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub hash: String,
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
    pub pqp_commitment: String,

    pub nTx: u32,
    pub tx: Vec<String>,
}

impl Block {
    pub fn new(
        hash: String,
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
        pqp_commitment: String,
        nTx: u32,
        tx: Vec<String>,
    ) -> Self {
        Self {
            hash,
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
            pqp_commitment,
            nTx,
            tx,
        }
    }

    pub fn genisis() -> Block {
        Block::new(
            "c0e423b2e59bd86bb3404075436032eab0df865eb1ed6c168ab97336e2f0d4a5".to_string(),
            0,
            "a".to_string(),
            1,
            "00".repeat(32),
            "00".repeat(32),
            0,
            "1d00ffff".to_string(),
            0,
            0,
            PQPEntry {
                queue_index: 0,
                block_hash: "00".repeat(32),
                parent_hash: "00".repeat(32),
                miner_address: "GENISIS_LEADER_HEX".to_string(),
                signature: "".repeat(64),
            },
            "00".repeat(32),
            0,
            vec![],
        )
    }

    pub fn block_hash(block: &mut Block) {
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
        hasher.update(hex::decode(&block.pqp_entry.block_hash).unwrap_or_default());
        hasher.update(hex::decode(&block.pqp_entry.parent_hash).unwrap_or_default());
        hasher.update(block.pqp_entry.miner_address.as_bytes());
        hasher.update(hex::decode(&block.pqp_entry.signature).unwrap_or_default());
        hasher.update(hex::decode(&block.pqp_commitment).unwrap_or_default());
        hasher.update(block.nTx.to_le_bytes());
        for tx in &block.tx {
            hasher.update(tx.as_bytes());
        }

        block.hash = hex::encode(hasher.finalize());
    }
}
