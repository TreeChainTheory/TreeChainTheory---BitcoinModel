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
        miner_pubkey: String, //it is raw pubkey hex not the address
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

        let pubkey_hash = Self::pubkey_hash_from_pubkey(&miner_pubkey);
        let miner_address =
            Self::address_from_pubkey_hash(&pubkey_hash).expect("Failed to derive address");

        let miner_output = TxOutput {
            value: subsidy + fees,
            script_pubkey: Self::create_p2pkh_script(pubkey_hash),
            address: Some(miner_address),
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

    pub fn pubkey_hash_from_pubkey(pubkey_hex: &str) -> String {
        let pubkey_bytes = hex::decode(pubkey_hex).expect("Invalid pubkey hex");

        // SHA256
        let sha256 = Sha256::digest(&pubkey_bytes);

        // RIPEMD160
        let ripemd = Ripemd160::digest(&sha256);

        hex::encode(ripemd)
    }

    pub fn pubkey_hash_from_address(address: &str) -> Result<String, String> {
        // Decode Base58Check address
        let decoded = bs58::decode(address)
            .into_vec()
            .map_err(|e| format!("Base58 decode error: {}", e))?;

        // Check length (25 bytes for P2PKH: 1 version byte + 20 hash bytes + 4 checksum bytes)
        if decoded.len() != 25 {
            return Err("Invalid address length".to_string());
        }

        // Verify version byte (0x00 for P2PKH mainnet)
        if decoded[0] != 0x00 {
            return Err("Invalid address version (only P2PKH supported)".to_string());
        }

        // Verify checksum
        let payload = &decoded[0..21]; // version + pubkey hash
        let checksum = &decoded[21..25];
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        if checksum != &hash2[0..4] {
            return Err("Invalid address checksum".to_string());
        }

        // Extract 20-byte pubkey hash
        let pubkey_hash = &decoded[1..21];
        Ok(hex::encode(pubkey_hash))
    }

    pub fn address_from_pubkey_hash(pubkey_hash: &str) -> Result<String, String> {
        let hash_bytes =
            hex::decode(pubkey_hash).map_err(|e| format!("Invalid pubkey hash hex: {}", e))?;
        if hash_bytes.len() != 20 {
            return Err("Pubkey hash must be 20 bytes".to_string());
        }

        // Create payload: version byte (0x00 for P2PKH) + pubkey hash
        let mut payload = vec![0x00];
        payload.extend_from_slice(&hash_bytes);

        // Compute checksum: first 4 bytes of SHA256(SHA256(payload))
        let mut hasher = Sha256::new();
        hasher.update(&payload);
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        payload.extend_from_slice(&hash2[0..4]);

        // Encode to Base58Check
        Ok(bs58::encode(payload).into_string())
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction {{ txid: {}, version: {}, locktime: {}, inputs: {}, outputs: {} }}",
            self.txid,
            self.version,
            self.locktime,
            self.vin.len(),
            self.vout.len(),
        )
    }
}
