use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

// Represents a transaction input (vin)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxInput {
    pub txid: String,       // 32-byte txid in hex
    pub vout: u32,          // Output index from previous tx
    pub script_sig: String, // Hex-encoded signature + pubkey (simplified)
    pub sequence: u32,      // Sequence for locktime/RBF
}

// Represents a transaction output (vout)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxOutput {
    pub value: u64,            // Amount in satoshis
    pub script_pubkey: String, // Hex-encoded P2PKH script
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>, // Derived address (e.g., Base58)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub txid: String,        // Computed txid (double-SHA256)
    pub version: u32,        // Transaction version
    pub vin: Vec<TxInput>,   // List of inputs
    pub vout: Vec<TxOutput>, // List of outputs
    pub locktime: u32,       // Earliest block/time for inclusion
}

impl Transaction {
    pub fn new(version: u32, locktime: u32, vin: Vec<TxInput>, vout: Vec<TxOutput>) -> Self {
        let mut tx = Transaction {
            txid: String::new(),
            version,
            vin,
            vout,
            locktime,
        };
        tx.txid = tx.compute_txid();
        tx
    }

    pub fn new_coinbase(
        version: u32,
        miner_address: String,
        queue_index: u32,
        subsidy: u64,
        fees: u64,
        extra_nonce: &str, // arbitrary data
        tag: &str,
    ) -> Self {
        let coinbase_input = TxInput {
            txid: "00".repeat(32),
            vout: u32::MAX,
            script_sig: hex::encode(format!("{}{}{}", queue_index, extra_nonce, tag).as_bytes()),
            sequence: 0xffffffff,
        };

        let miner_output = TxOutput {
            value: subsidy + fees,
            script_pubkey: Self::create_p2pkh_script(miner_address),
            address: miner_address,
        };

        let mut tx = Transaction {
            txid: String::new(),
            version,
            vin: vec![coinbase_input],
            vout: vec![miner_output],
            locktime: 0,
        };

        tx.txid = tx.compute_txid();
        tx
    }

    fn create_p2pkh_script(pubkey_hash: &str) -> String {
        format!("76a914{}88ac", pubkey_hash)
    }

    pub fn compute_txid(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());
        hasher.update(self.queue_index.to_le_bytes());
        hasher.update((self.vin.len() as u8).to_le_bytes());
        for input in &self.vin {
            hasher.update(hex::decode(&input.prev_txid).expect("Invalid txid hex"));
            hasher.update(input.vout.to_le_bytes());
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            hasher.update((script_bytes.len() as u8).to_le_bytes());
            hasher.update(script_bytes);
            hasher.update(input.sequence.to_le_bytes());
        }

        hasher.update((self.vout.len() as u8).to_le_bytes());
        for output in &self.vout {
            hasher.update(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            hasher.update((script_bytes.len() as u8).to_le_bytes());
            hasher.update(script_bytes);
        }
        hasher.update(self.locktime.to_le_bytes());
        // Double-SHA256
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        hex::encode(hash2)
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction {{ txid: {}, version: {}, locktime: {}, inputs: {}, outputs: {}, is_coinbase: {} }}",
            self.txid,
            self.version,
            self.locktime,
            self.vin.len(),
            self.vout.len(),
            self.is_coinbase.unwrap_or(false)
        )
    }
}
