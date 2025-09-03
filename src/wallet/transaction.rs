use crate::chain_util::ChainUtil;
use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

// Represents a transaction input (vin)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TxInput {
    pub txid: String,       // 32-byte txid in hex
    pub vout: u32,          // Output index from previous tx
    pub script_sig: String, // Hex-encoded signature + pubkey (simplified)
    pub sequence: u32,      // Sequence for locktime/RBF
}

// Represents a transaction output (vout)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TxOutput {
    pub value: u64,            // Amount in satoshis
    pub script_pubkey: String, // Hex-encoded P2PKH script
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
        miner_address: String, //it is miner_address not the pub key hex
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

        let pubkey_hash = ChainUtil::pubkey_hash_from_address(&miner_address).expect("msg");

        let miner_output = TxOutput {
            value: subsidy + fees,
            script_pubkey: Self::create_p2pkh_script(&pubkey_hash),
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

    fn create_p2sh_script(redeem_script: &str) -> String {
        let redeem_bytes = hex::decode(redeem_script).expect("Invalid redeem script hex");
        let script_hash = ChainUtil::hash160(&redeem_bytes);
        format!("a914{}87", script_hash)
    }

    pub fn create_normal_txn(
        version: u32,
        vin: Vec<TxInput>,
        recipients: Vec<(u64, String)>, // (value, recipient_address)
        locktime: u32,
    ) -> Self {
        let vout: Vec<TxOutput> = recipients
            .into_iter()
            .map(|(value, address)| {
                let pubkey_hash = ChainUtil::pubkey_hash_from_address(&address)
                    .expect("Invalid recipient address in create_normal_txn");
                TxOutput {
                    value,
                    script_pubkey: Self::create_p2pkh_script(&pubkey_hash),
                }
            })
            .collect();
        Self::new(version, locktime, vin, vout)
    }

    /// Standard multisig creation **requires raw public keys** (not addresses).
    pub fn create_multisig_txn(
        version: u32,
        vin: Vec<TxInput>,
        m: u8,
        pubkeys_hex: Vec<String>, // raw pubkey hex strings (not addresses)
        value: u64,
        locktime: u32,
    ) -> Self {
        let redeem_script = Self::create_multisig_redeem_script(m, &pubkeys_hex);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let vout = vec![TxOutput {
            value,
            script_pubkey,
        }];
        Self::new(version, locktime, vin, vout)
    }

    fn create_multisig_redeem_script(m: u8, pubkeys: &[String]) -> String {
        let n = pubkeys.len() as u8;
        assert!(m <= n, "m cannot be greater than n");
        // OP_m is 0x50 + m for small m (OP_1 .. OP_16 when m in 1..16).
        let mut script: Vec<u8> = vec![0x50 + m];
        for pubkey in pubkeys {
            let pub_bytes = hex::decode(pubkey).expect("Invalid pubkey hex");
            // push length then pubkey bytes
            script.push(pub_bytes.len() as u8);
            script.extend(pub_bytes);
        }
        // OP_n
        script.push(0x50 + n);
        script.push(0xae); // OP_CHECKMULTISIG
        hex::encode(script)
    }

    //normal timelocked cltv txn
    pub fn create_timelocked_txn(
        version: u32,
        vin: Vec<TxInput>,
        cltv_lock_time: u32,
        recipient_address: String, // address, will be converted internally
        value: u64,
        locktime: u32,
    ) -> Self {
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(&recipient_address)
            .expect("Invalid recipient address in create_timelocked_txn");
        let redeem_script = Self::create_cltv_redeem_script(cltv_lock_time, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let vout = vec![TxOutput {
            value,
            script_pubkey,
        }];
        Self::new(version, locktime, vin, vout)
    }

    //normal timelocked txn script
    fn create_cltv_redeem_script(cltv_lock_time: u32, pubkey_hash: &str) -> String {
        let mut script: Vec<u8> = vec![];
        let lock_bytes = cltv_lock_time.to_le_bytes();

        // Push the locktime as a raw byte vector (simple approach; not minimal push encoding)
        script.push(lock_bytes.len() as u8); // Assume <= 75
        script.extend_from_slice(&lock_bytes);

        script.push(0xb9); // OP_CHECKLOCKTIMEVERIFY
        script.push(0x75); // OP_DROP
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(0x14); // 20 bytes push
        let hash_bytes = hex::decode(pubkey_hash).expect("Invalid pubkey hash hex");
        script.extend(hash_bytes);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        hex::encode(script)
    }

    pub fn create_timelocked_csv_txn(
        version: u32,
        vin: Vec<TxInput>,
        csv_lock_blocks: u32, // Relative lock time in blocks
        recipient_address: String,
        value: u64,
        locktime: u32,
    ) -> Self {
        // Set sequence in inputs to enable CSV
        let vin = vin
            .into_iter()
            .map(|mut input| {
                input.sequence = csv_lock_blocks; // Set sequence to relative lock time
                input
            })
            .collect();
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(&recipient_address)
            .expect("Invalid recipient address in create_timelocked_csv_txn");
        let redeem_script = Self::create_csv_redeem_script(csv_lock_blocks, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let vout = vec![TxOutput {
            value,
            script_pubkey,
        }];
        Self::new(version, locktime, vin, vout)
    }

    fn create_csv_redeem_script(csv_lock_blocks: u32, pubkey_hash: &str) -> String {
        let mut script: Vec<u8> = vec![];
        let lock_bytes = csv_lock_blocks.to_le_bytes();
        script.push(lock_bytes.len() as u8); // Assume <= 75
        script.extend_from_slice(&lock_bytes);
        script.push(0xba); // OP_CHECKSEQUENCEVERIFY
        script.push(0x75); // OP_DROP
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(0x14); // 20 bytes
        let hash_bytes = hex::decode(pubkey_hash).expect("Invalid pubkey hash hex");
        script.extend(hash_bytes);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        hex::encode(script)
    }

    pub fn compute_txid(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());
        hasher.update((self.vin.len() as u8).to_le_bytes());
        for input in &self.vin {
            hasher.update(hex::decode(&input.txid).expect("Invalid txid hex"));
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
            "Transaction {{ txid: {}, version: {}, locktime: {}, inputs: {}, outputs: {} }}",
            self.txid,
            self.version,
            self.locktime,
            self.vin.len(),
            self.vout.len(),
        )
    }
}
