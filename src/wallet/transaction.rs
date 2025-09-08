use crate::chain_util::ChainUtil;
use crate::config::USER_TXN_FREERATE;
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const SIGHASH_ALL: u32 = 0x01;

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
    pub txid: String,        // Computed txid (double-SHA256 of non-witness data)
    pub hash: String,        // Computed hash (wtxid if SegWit, else txid)
    pub version: u32,        // Transaction version
    pub vin: Vec<TxInput>,   // List of inputs
    pub vout: Vec<TxOutput>, // List of outputs
    pub witnesses: Option<Vec<Vec<String>>>, // Optional witnesses (hex strings) for SegWit
    pub locktime: u32,       // Earliest block/time for inclusion
}

impl Transaction {
    pub fn new(
        version: u32,
        locktime: u32,
        vin: Vec<TxInput>,
        vout: Vec<TxOutput>,
        witnesses: Option<Vec<Vec<String>>>,
    ) -> Self {
        let mut tx = Transaction {
            txid: String::new(),
            hash: String::new(),
            version,
            vin,
            vout,
            witnesses,
            locktime,
        };
        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();
        tx
    }

    pub fn double_sha(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        hasher.finalize().to_vec()
    }

    pub fn compute_non_witness_txid(&self) -> String {
        let serialized = self.serialize_non_witness();
        hex::encode(Self::double_sha(&serialized))
    }
    pub fn compute_hash(&self) -> String {
        if self.witnesses.is_none() {
            return self.compute_non_witness_txid();
        }

        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());
        hasher.update([0x00]); // marker
        hasher.update([0x01]); // flag
        hasher.update((self.vin.len() as u8).to_le_bytes());
        for input in &self.vin {
            hasher.update(hex::decode(&input.txid).expect("Invalid txid hex"));
            hasher.update(input.vout.to_le_bytes());
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            hasher.update((script_bytes.len() as u8).to_le_bytes());
            hasher.update(&script_bytes);
            hasher.update(input.sequence.to_le_bytes());
        }

        hasher.update((self.vout.len() as u8).to_le_bytes());
        for output in &self.vout {
            hasher.update(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            hasher.update((script_bytes.len() as u8).to_le_bytes());
            hasher.update(&script_bytes);
        }

        // Witnesses
        let witnesses = self.witnesses.as_ref().unwrap();
        for wit_stack in witnesses {
            hasher.update((wit_stack.len() as u8).to_le_bytes());
            for item in wit_stack {
                let item_bytes = hex::decode(item).expect("Invalid witness item hex");
                hasher.update((item_bytes.len() as u8).to_le_bytes());
                hasher.update(&item_bytes);
            }
        }

        hasher.update(self.locktime.to_le_bytes());
        // Double-SHA256
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        hex::encode(hash2)
    }

    pub fn new_coinbase(
        version: u32,
        miner_address: String,
        queue_index: u32,
        subsidy: u64,
        fees: u64,
        extra_nonce: &str,
        tag: &str,
    ) -> Self {
        let coinbase_input = TxInput {
            txid: "00".repeat(32),
            vout: u32::MAX,
            script_sig: hex::encode(format!("{}{}{}", queue_index, extra_nonce, tag).as_bytes()),
            sequence: 0xffffffff,
        };

        let pubkey_hash =
            ChainUtil::pubkey_hash_from_address(&miner_address).expect("Invalid miner address");

        let miner_output = TxOutput {
            value: subsidy + fees,
            script_pubkey: Self::create_p2pkh_script(&pubkey_hash),
        };

        Self::new(version, 0, vec![coinbase_input], vec![miner_output], None)
    }

    pub fn create_p2pkh_script(pubkey_hash: &str) -> String {
        format!("76a914{}88ac", pubkey_hash)
    }

    pub fn create_p2wpkh_script(pubkey_hash: &str) -> String {
        format!("0014{}", pubkey_hash)
    }

    pub fn create_p2sh_script(redeem_script: &str) -> String {
        let redeem_bytes = hex::decode(redeem_script).expect("Invalid redeem script hex");
        let script_hash = ChainUtil::hash160(&redeem_bytes);
        format!("a914{}87", script_hash)
    }

    pub fn create_normal_txn(
        version: u32,
        vin: Vec<TxInput>,
        recipients: Vec<(u64, String)>,
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
        Self::new(version, locktime, vin, vout, None)
    }

    pub fn create_multisig_txn(
        version: u32,
        vin: Vec<TxInput>,
        m: u8,
        pubkeys_hex: Vec<String>,
        value: u64,
        locktime: u32,
    ) -> Self {
        let redeem_script = Self::create_multisig_redeem_script(m, &pubkeys_hex);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let vout = vec![TxOutput {
            value,
            script_pubkey,
        }];
        Self::new(version, locktime, vin, vout, None)
    }

    pub fn create_multisig_redeem_script(m: u8, pubkeys: &[String]) -> String {
        let n = pubkeys.len() as u8;
        assert!(m <= n, "m cannot be greater than n");
        let mut script: Vec<u8> = vec![0x50 + m];
        for pubkey in pubkeys {
            let pub_bytes = hex::decode(pubkey).expect("Invalid pubkey hex");
            script.push(pub_bytes.len() as u8);
            script.extend(pub_bytes);
        }
        script.push(0x50 + n);
        script.push(0xae); // OP_CHECKMULTISIG
        hex::encode(script)
    }

    pub fn create_timelocked_txn(
        version: u32,
        vin: Vec<TxInput>,
        cltv_lock_time: u32,
        recipient_address: String,
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
        Self::new(version, locktime, vin, vout, None)
    }

    pub fn create_p2wsh_script(redeem_script: &str) -> String {
        let redeem_bytes = hex::decode(redeem_script).expect("Invalid redeem script hex");
        let mut hasher = Sha256::new();
        hasher.update(&redeem_bytes);
        let script_hash = hasher.finalize();
        format!("0020{}", hex::encode(script_hash))
    }

    pub fn create_cltv_redeem_script(cltv_lock_time: u32, pubkey_hash: &str) -> String {
        let mut script: Vec<u8> = vec![];
        let lock_bytes = cltv_lock_time.to_le_bytes();
        script.push(lock_bytes.len() as u8);
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
        csv_lock_blocks: u32,
        recipient_address: String,
        value: u64,
        locktime: u32,
    ) -> Self {
        let vin = vin
            .into_iter()
            .map(|mut input| {
                input.sequence = csv_lock_blocks;
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
        Self::new(version, locktime, vin, vout, None)
    }

    pub fn create_csv_redeem_script(csv_lock_blocks: u32, pubkey_hash: &str) -> String {
        let mut script: Vec<u8> = vec![];
        let lock_bytes = csv_lock_blocks.to_le_bytes();
        script.push(lock_bytes.len() as u8);
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

    pub fn get_size_vsize_weight(&self) -> (usize, usize, usize) {
        let is_segwit = self.witnesses.is_some();
        let mut size: usize = 0;

        // Version
        size += 4;

        if is_segwit {
            // Marker + flag
            size += 2;
        }

        // Input count (assume 1 byte varint)
        size += 1;

        // Inputs
        for input in &self.vin {
            // txid + vout
            size += 32 + 4;
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            let script_len = script_bytes.len();
            size += if script_len <= 252 { 1 } else { 3 };
            size += script_len;
            // sequence
            size += 4;
        }

        // Output count (assume 1 byte varint)
        size += 1;

        // Outputs
        for output in &self.vout {
            // value
            size += 8;
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            let script_len = script_bytes.len();
            size += if script_len <= 252 { 1 } else { 3 };
            size += script_len;
        }

        if is_segwit {
            // Witnesses
            let witnesses = self.witnesses.as_ref().unwrap();
            for wit_stack in witnesses {
                // witness count (assume 1 byte varint)
                size += 1;
                for item in wit_stack {
                    let item_bytes = hex::decode(item).expect("Invalid witness item hex");
                    let item_len = item_bytes.len();
                    size += if item_len <= 252 { 1 } else { 3 };
                    size += item_len;
                }
            }
        }

        // Locktime
        size += 4;

        if is_segwit {
            // Calculate non-witness size
            let mut non_wit_size: usize = 4 + 1;
            for input in &self.vin {
                non_wit_size += 32 + 4;
                let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
                let script_len = script_bytes.len();
                non_wit_size += if script_len <= 252 { 1 } else { 3 } + script_len + 4;
            }
            non_wit_size += 1;
            for output in &self.vout {
                let script_bytes =
                    hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
                let script_len = script_bytes.len();
                non_wit_size += 8 + if script_len <= 252 { 1 } else { 3 } + script_len;
            }
            non_wit_size += 4;

            let weight = non_wit_size * 3 + size;
            let vsize = ((weight + 3) / 4) as usize;
            (size, vsize, weight)
        } else {
            (size, size, size * 4)
        }
    }

    pub fn serialize_non_witness(&self) -> Vec<u8> {
        let mut serialized = Vec::new();
        serialized.extend(self.version.to_le_bytes());
        serialized.extend((self.vin.len() as u8).to_le_bytes());
        for input in &self.vin {
            println!("txid:{} ", &input.txid.clone());
            serialized.extend(hex::decode(&input.txid).expect("Invalid txid hex"));
            serialized.extend(input.vout.to_le_bytes());
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            serialized.extend((script_bytes.len() as u8).to_le_bytes());
            serialized.extend(script_bytes);
            serialized.extend(input.sequence.to_le_bytes());
        }
        serialized.extend((self.vout.len() as u8).to_le_bytes());
        for output in &self.vout {
            serialized.extend(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            serialized.extend((script_bytes.len() as u8).to_le_bytes());
            serialized.extend(script_bytes);
        }
        serialized.extend(self.locktime.to_le_bytes());
        serialized
    }

    pub fn serialize_with_witness(&self) -> Vec<u8> {
        let mut serialized = Vec::new();
        serialized.extend(self.version.to_le_bytes());
        if self.witnesses.is_some() {
            serialized.extend([0x00, 0x01]); // marker + flag
        }
        serialized.extend((self.vin.len() as u8).to_le_bytes());
        for input in &self.vin {
            serialized.extend(hex::decode(&input.txid).expect("Invalid txid hex"));
            serialized.extend(input.vout.to_le_bytes());
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            serialized.extend((script_bytes.len() as u8).to_le_bytes());
            serialized.extend(&script_bytes);
            serialized.extend(input.sequence.to_le_bytes());
        }

        serialized.extend((self.vout.len() as u8).to_le_bytes());
        for output in &self.vout {
            serialized.extend(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            serialized.extend((script_bytes.len() as u8).to_le_bytes());
            serialized.extend(&script_bytes);
        }
        if let Some(witnesses) = &self.witnesses {
            for wit_stack in witnesses {
                serialized.extend((wit_stack.len() as u8).to_le_bytes());
                for item in wit_stack {
                    let item_bytes = hex::decode(item).expect("Invalid witness item hex");
                    serialized.extend((item_bytes.len() as u8).to_le_bytes());
                    serialized.extend(&item_bytes);
                }
            }
        }
        serialized.extend(self.locktime.to_le_bytes());
        serialized
    }

    pub fn compute_segwit_sighash(
        &self,
        input_index: usize,
        input_value: u64,
        script_pubkey: &str,
        sighash_type: u32,
    ) -> Vec<u8> {
        let mut preimage = Vec::new();

        // 1. nVersion
        preimage.extend(self.version.to_le_bytes());

        // 2. hashPrevouts
        let mut prevouts = Vec::new();
        for input in &self.vin {
            prevouts.extend(hex::decode(&input.txid).expect("Invalid txid hex"));
            prevouts.extend(input.vout.to_le_bytes());
        }
        let hash_prevouts = Self::double_sha(&prevouts);
        preimage.extend(hash_prevouts);

        // 3. hashSequence
        let mut sequences = Vec::new();
        for input in &self.vin {
            sequences.extend(input.sequence.to_le_bytes());
        }
        let hash_sequence = Self::double_sha(&sequences);
        preimage.extend(hash_sequence);

        // 4. outpoint
        preimage.extend(hex::decode(&self.vin[input_index].txid).expect("Invalid txid hex"));
        preimage.extend(self.vin[input_index].vout.to_le_bytes());

        // 5. scriptCode (for P2WPKH, equivalent P2PKH script)
        let script_code = if script_pubkey.starts_with("0014") {
            format!("76a914{}88ac", &script_pubkey[4..])
        } else {
            script_pubkey.to_string()
        };
        let script_bytes = hex::decode(&script_code).expect("Invalid script code hex");
        preimage.extend((script_bytes.len() as u8).to_le_bytes());
        preimage.extend(script_bytes);

        // 6. value
        preimage.extend(input_value.to_le_bytes());

        // 7. nSequence
        preimage.extend(self.vin[input_index].sequence.to_le_bytes());

        // 8. hashOutputs
        let mut outputs = Vec::new();
        for output in &self.vout {
            outputs.extend(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            outputs.extend((script_bytes.len() as u8).to_le_bytes());
            outputs.extend(script_bytes);
        }
        let hash_outputs = Self::double_sha(&outputs);
        preimage.extend(hash_outputs);

        // 9. nLocktime
        preimage.extend(self.locktime.to_le_bytes());

        // 10. sighash_type
        preimage.extend(sighash_type.to_le_bytes());

        Self::double_sha(&preimage)
    }

    pub fn compute_sighash(
        &self,
        input_index: usize,
        script_pubkey: &str,
        _input_value: u64, // Not needed for non-SegWit sighash
        sighash_type: u32,
    ) -> Vec<u8> {
        // Create modified vin for sighash
        let mut sig_vin = self.vin.clone();
        for j in 0..sig_vin.len() {
            sig_vin[j].script_sig = if j == input_index {
                script_pubkey.to_string()
            } else {
                "".to_string()
            };
        }

        // Create modified transaction for hash
        let mod_tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version: self.version,
            vin: sig_vin,
            vout: self.vout.clone(),
            witnesses: None,
            locktime: self.locktime,
        };

        let mut serialized = mod_tx.serialize_non_witness();
        serialized.extend(sighash_type.to_le_bytes());

        Self::double_sha(&serialized)
    }

    pub fn create_new_transaction(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        pool: &TransactionPool,
        value: u64,
        fee: u64,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1u32;
        let locktime = 0u32;
        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Collect UTXOs from both main UTXO set and mempool UTXO set
        let mut owned_utxos: Vec<((String, u32), Utxo)> = utxo_set
            .utxos
            .iter()
            .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(
                pool.utxo_set
                    .utxos
                    .iter()
                    .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
                    .map(|(k, v)| (k.clone(), v.clone())),
            )
            .filter(|(k, _)| pool.get_transactions_spending_utxo(&k.0, k.1).is_empty())
            .collect();

        if owned_utxos.is_empty() {
            return Err("No unspent UTXOs found for wallet".to_string());
        }

        owned_utxos.sort_by(|a, b| b.1.out.value.cmp(&a.1.out.value));

        let mut selected: Vec<(String, u32, Utxo)> = Vec::new();
        let mut total_input = 0u64;

        // Select UTXOs to cover value + fee
        for (key, utxo) in owned_utxos {
            selected.push((key.0, key.1, utxo));
            total_input += selected.last().unwrap().2.out.value;
            if total_input >= value + fee {
                break;
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create vin
        let vin: Vec<TxInput> = selected
            .clone()
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid: txid.clone(),
                vout: vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();

        // Create vout
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(to_address)
            .map_err(|_| "Invalid recipient address".to_string())?;
        let script_pubkey = Self::create_p2pkh_script(&pubkey_hash);
        let mut final_vout = vec![TxOutput {
            value,
            script_pubkey: script_pubkey.clone(),
        }];

        // Calculate change
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            final_vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script.clone(),
            });
        }

        // Create unsigned tx template
        let mut tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version,
            vin,
            vout: final_vout,
            witnesses: None,
            locktime,
        };

        // Sign each input
        for i in 0..tx.vin.len() {
            let mut sig_vin = tx.vin.clone();
            for j in 0..sig_vin.len() {
                sig_vin[j].script_sig = if j == i {
                    selected[i].2.out.script_pubkey.clone()
                } else {
                    "".to_string()
                };
            }

            let mod_tx = Transaction {
                txid: "".to_string(),
                hash: "".to_string(),
                version: tx.version,
                vin: sig_vin,
                vout: tx.vout.clone(),
                witnesses: None,
                locktime: tx.locktime,
            };

            let mut serialized = mod_tx.serialize_non_witness();
            serialized.extend(SIGHASH_ALL.to_le_bytes());
            let sighash_bytes = Self::double_sha(&serialized);
            let sig_hex = wallet.sign_data(&sighash_bytes);
            let script_sig = format!("{}{}", sig_hex, wallet.public_key);
            tx.vin[i].script_sig = script_sig;
        }

        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();

        Ok(tx)
    }

    pub fn create_new_multisig_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        pool: &TransactionPool,
        value: u64,
        fee: u64,
        m: u8,
        pubkeys_hex: Vec<String>,
    ) -> Result<Self, String> {
        let version = 1u32;
        let locktime = 0u32;
        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Validate inputs
        if m == 0 || m as usize > pubkeys_hex.len() {
            return Err("Invalid m value for multisig".to_string());
        }
        for pubkey in &pubkeys_hex {
            if hex::decode(pubkey).is_err() {
                return Err(format!("Invalid pubkey hex: {}", pubkey));
            }
        }

        // Collect UTXOs from both main UTXO set and mempool UTXO set
        let mut owned_utxos: Vec<((String, u32), Utxo)> = utxo_set
            .utxos
            .iter()
            .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(
                pool.utxo_set
                    .utxos
                    .iter()
                    .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
                    .map(|(k, v)| (k.clone(), v.clone())),
            )
            .filter(|(k, _)| pool.get_transactions_spending_utxo(&k.0, k.1).is_empty())
            .collect();

        if owned_utxos.is_empty() {
            return Err("No unspent UTXOs found for wallet".to_string());
        }

        owned_utxos.sort_by(|a, b| b.1.out.value.cmp(&a.1.out.value));

        let mut selected: Vec<(String, u32, Utxo)> = Vec::new();
        let mut total_input = 0u64;

        // Select UTXOs to cover value + fee
        for (key, utxo) in owned_utxos {
            selected.push((key.0, key.1, utxo));
            total_input += selected.last().unwrap().2.out.value;
            if total_input >= value + fee {
                break;
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds for value and fee: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let mut vin: Vec<TxInput> = selected
            .clone()
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid: txid.clone(),
                vout: vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();

        // Create vout
        let redeem_script = Self::create_multisig_redeem_script(m, &pubkeys_hex);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut final_vout = vec![TxOutput {
            value,
            script_pubkey: script_pubkey.clone(),
        }];

        // Adjust change based on fee
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            final_vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script.clone(),
            });
        }

        // Create unsigned tx template
        let mut tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version,
            vin,
            vout: final_vout,
            witnesses: None,
            locktime,
        };

        // Sign each input
        for i in 0..tx.vin.len() {
            let mut sig_vin = tx.vin.clone();
            for j in 0..sig_vin.len() {
                sig_vin[j].script_sig = if j == i {
                    selected[i].2.out.script_pubkey.clone()
                } else {
                    "".to_string()
                };
            }

            let mod_tx = Transaction {
                txid: "".to_string(),
                hash: "".to_string(),
                version: tx.version,
                vin: sig_vin,
                vout: tx.vout.clone(),
                witnesses: None,
                locktime: tx.locktime,
            };

            let mut serialized = mod_tx.serialize_non_witness();
            serialized.extend(SIGHASH_ALL.to_le_bytes());
            let sighash_bytes = Self::double_sha(&serialized);
            let sig_hex = wallet.sign_data(&sighash_bytes);
            let script_sig = format!("{}{}", sig_hex, wallet.public_key);
            tx.vin[i].script_sig = script_sig;
        }

        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();

        Ok(tx)
    }

    pub fn create_new_timelocked_cltv_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        pool: &TransactionPool,
        value: u64,
        fee: u64,
        cltv_lock_time: u32,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1u32;
        let locktime = cltv_lock_time;
        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Collect UTXOs from both main UTXO set and mempool UTXO set
        let mut owned_utxos: Vec<((String, u32), Utxo)> = utxo_set
            .utxos
            .iter()
            .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(
                pool.utxo_set
                    .utxos
                    .iter()
                    .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
                    .map(|(k, v)| (k.clone(), v.clone())),
            )
            .filter(|(k, _)| pool.get_transactions_spending_utxo(&k.0, k.1).is_empty())
            .collect();

        if owned_utxos.is_empty() {
            return Err("No unspent UTXOs found for wallet".to_string());
        }

        owned_utxos.sort_by(|a, b| b.1.out.value.cmp(&a.1.out.value));

        let mut selected: Vec<(String, u32, Utxo)> = Vec::new();
        let mut total_input = 0u64;

        // Select UTXOs to cover value + fee
        for (key, utxo) in owned_utxos {
            selected.push((key.0, key.1, utxo));
            total_input += selected.last().unwrap().2.out.value;
            if total_input >= value + fee {
                break;
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds for value and fee: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let mut vin: Vec<TxInput> = selected
            .clone()
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid: txid.clone(),
                vout: vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();

        // Create vout
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(to_address)
            .map_err(|_| "Invalid recipient address".to_string())?;
        let redeem_script = Self::create_cltv_redeem_script(cltv_lock_time, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut final_vout = vec![TxOutput {
            value,
            script_pubkey: script_pubkey.clone(),
        }];

        // Adjust change based on fee
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            final_vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script.clone(),
            });
        }

        // Create unsigned tx template
        let mut tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version,
            vin,
            vout: final_vout,
            witnesses: None,
            locktime,
        };

        // Sign each input
        for i in 0..tx.vin.len() {
            let mut sig_vin = tx.vin.clone();
            for j in 0..sig_vin.len() {
                sig_vin[j].script_sig = if j == i {
                    selected[i].2.out.script_pubkey.clone()
                } else {
                    "".to_string()
                };
            }

            let mod_tx = Transaction {
                txid: "".to_string(),
                hash: "".to_string(),
                version: tx.version,
                vin: sig_vin,
                vout: tx.vout.clone(),
                witnesses: None,
                locktime: tx.locktime,
            };

            let mut serialized = mod_tx.serialize_non_witness();
            serialized.extend(SIGHASH_ALL.to_le_bytes());
            let sighash_bytes = Self::double_sha(&serialized);
            let sig_hex = wallet.sign_data(&sighash_bytes);
            let script_sig = format!("{}{}", sig_hex, wallet.public_key);
            tx.vin[i].script_sig = script_sig;
        }

        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();

        Ok(tx)
    }

    pub fn create_new_timelocked_csv_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        pool: &TransactionPool,
        value: u64,
        fee: u64,
        csv_lock_blocks: u32,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1u32;
        let locktime = 0u32;
        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Collect UTXOs from both main UTXO set and mempool UTXO set
        let mut owned_utxos: Vec<((String, u32), Utxo)> = utxo_set
            .utxos
            .iter()
            .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(
                pool.utxo_set
                    .utxos
                    .iter()
                    .filter(|(_, utxo)| utxo.out.script_pubkey == sender_script)
                    .map(|(k, v)| (k.clone(), v.clone())),
            )
            .filter(|(k, _)| pool.get_transactions_spending_utxo(&k.0, k.1).is_empty())
            .collect();

        if owned_utxos.is_empty() {
            return Err("No unspent UTXOs found for wallet".to_string());
        }

        owned_utxos.sort_by(|a, b| b.1.out.value.cmp(&a.1.out.value));

        let mut selected: Vec<(String, u32, Utxo)> = Vec::new();
        let mut total_input = 0u64;

        // Select UTXOs to cover value + fee
        for (key, utxo) in owned_utxos {
            selected.push((key.0, key.1, utxo));
            total_input += selected.last().unwrap().2.out.value;
            if total_input >= value + fee {
                break;
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds for value and fee: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let mut vin: Vec<TxInput> = selected
            .clone()
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid: txid.clone(),
                vout: vout,
                script_sig: "".to_string(),
                sequence: csv_lock_blocks,
            })
            .collect();

        // Create vout
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(to_address)
            .map_err(|_| "Invalid recipient address".to_string())?;
        let redeem_script = Self::create_csv_redeem_script(csv_lock_blocks, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut final_vout = vec![TxOutput {
            value,
            script_pubkey: script_pubkey.clone(),
        }];

        // Adjust change based on fee
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            final_vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script.clone(),
            });
        }

        // Create unsigned tx template
        let mut tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version,
            vin,
            vout: final_vout,
            witnesses: None,
            locktime,
        };

        // Sign each input
        for i in 0..tx.vin.len() {
            let mut sig_vin = tx.vin.clone();
            for j in 0..sig_vin.len() {
                sig_vin[j].script_sig = if j == i {
                    selected[i].2.out.script_pubkey.clone()
                } else {
                    "".to_string()
                };
            }

            let mod_tx = Transaction {
                txid: "".to_string(),
                hash: "".to_string(),
                version: tx.version,
                vin: sig_vin,
                vout: tx.vout.clone(),
                witnesses: None,
                locktime: tx.locktime,
            };

            let mut serialized = mod_tx.serialize_non_witness();
            serialized.extend(SIGHASH_ALL.to_le_bytes());
            let sighash_bytes = Self::double_sha(&serialized);
            let sig_hex = wallet.sign_data(&sighash_bytes);
            let script_sig = format!("{}{}", sig_hex, wallet.public_key);
            tx.vin[i].script_sig = script_sig;
        }

        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();

        Ok(tx)
    }
    pub fn modify_txn_to_add_segwit(
        &self,
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        output_redeem_scripts: Option<Vec<Option<String>>>,
    ) -> Result<Self, String> {
        if self.witnesses.is_some() {
            return Err("Transaction already contains witness data".to_string());
        }

        // Verify inputs are owned by wallet and get input values
        let sender_p2pkh_script = Self::create_p2pkh_script(&wallet.public_key_hash);
        let sender_p2wpkh_script = Self::create_p2wpkh_script(&wallet.public_key_hash);

        let mut input_values = Vec::new();
        for input in &self.vin {
            let utxo = utxo_set.get_utxo(&input.txid, input.vout).ok_or_else(|| {
                format!(
                    "UTXO not found for txid: {}, vout: {}",
                    input.txid, input.vout
                )
            })?;
            if utxo.out.script_pubkey != sender_p2pkh_script
                && utxo.out.script_pubkey != sender_p2wpkh_script
            {
                return Err("Input not owned by wallet".to_string());
            }
            input_values.push(utxo.out.value);
        }

        // Convert outputs to SegWit equivalents
        let mut new_vout = Vec::new();
        for (index, output) in self.vout.iter().enumerate() {
            if output.script_pubkey.starts_with("76a914") {
                let pubkey_hash = &output.script_pubkey[6..46];
                new_vout.push(TxOutput {
                    value: output.value,
                    script_pubkey: Self::create_p2wpkh_script(pubkey_hash),
                });
            } else if output.script_pubkey.starts_with("a914") {
                if let Some(rs_vec) = &output_redeem_scripts {
                    if rs_vec.len() != self.vout.len() {
                        return Err("Redeem scripts length does not match outputs".to_string());
                    }
                    if let Some(Some(rs)) = rs_vec.get(index) {
                        let rs_bytes =
                            hex::decode(rs).map_err(|_| "Invalid redeem script hex".to_string())?;
                        let calculated_hash = ChainUtil::hash160(&rs_bytes);
                        let expected_hash = &output.script_pubkey[4..44];
                        if calculated_hash != expected_hash {
                            return Err("Redeem script does not match script hash".to_string());
                        }
                        let new_script = Self::create_p2wsh_script(rs);
                        new_vout.push(TxOutput {
                            value: output.value,
                            script_pubkey: new_script,
                        });
                        continue;
                    }
                }
                new_vout.push(output.clone());
            } else {
                new_vout.push(output.clone());
            }
        }

        // Create new vin with empty script_sig
        let new_vin: Vec<TxInput> = self
            .vin
            .iter()
            .map(|input| TxInput {
                txid: input.txid.clone(),
                vout: input.vout,
                script_sig: "".to_string(),
                sequence: input.sequence,
            })
            .collect();

        // Create new transaction
        let mut new_tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version: self.version,
            vin: new_vin,
            vout: new_vout,
            witnesses: Some(vec![vec![]; self.vin.len()]),
            locktime: self.locktime,
        };

        // Sign each input with SegWit sighash
        let mut witnesses = vec![vec![]; self.vin.len()];
        for i in 0..new_tx.vin.len() {
            let utxo = utxo_set
                .get_utxo(&new_tx.vin[i].txid, new_tx.vin[i].vout)
                .unwrap();
            let sighash_bytes = new_tx.compute_segwit_sighash(
                i,
                input_values[i],
                &utxo.out.script_pubkey,
                SIGHASH_ALL,
            );
            let sig_hex = wallet.sign_data(&sighash_bytes);
            witnesses[i] = vec![sig_hex, wallet.public_key.clone()];
        }

        new_tx.witnesses = Some(witnesses);

        // Compute final txid and hash
        new_tx.txid = new_tx.compute_non_witness_txid();
        new_tx.hash = new_tx.compute_hash();

        Ok(new_tx)
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction {{ txid: {}, hash: {}, version: {}, locktime: {}, inputs: {}, outputs: {}, witnesses: {} }}",
            self.txid,
            self.hash,
            self.version,
            self.locktime,
            self.vin.len(),
            self.vout.len(),
            if self.witnesses.is_some() {
                "present"
            } else {
                "none"
            }
        )
    }
}
