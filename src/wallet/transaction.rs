use crate::chain_util::ChainUtil;
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
        println!("Redeem script: {}, Hash: {}", redeem_script, script_hash);
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
        script.extend(lock_bytes);
        script.push(0xb1); // OP_CHECKLOCKTIMEVERIFY
        script.push(0x75); // OP_DROP
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        let pk_hash_bytes = hex::decode(pubkey_hash).expect("Invalid pubkey hash hex");
        script.push(pk_hash_bytes.len() as u8);
        script.extend(pk_hash_bytes);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        hex::encode(script)
    }

    pub fn create_csv_redeem_script(csv_lock_blocks: u32, pubkey_hash: &str) -> String {
        let mut script: Vec<u8> = vec![];
        let lock_bytes = csv_lock_blocks.to_le_bytes();
        script.push(lock_bytes.len() as u8);
        script.extend(lock_bytes);
        script.push(0xb2); // OP_CHECKSEQUENCEVERIFY
        script.push(0x75); // OP_DROP
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        let pk_hash_bytes = hex::decode(pubkey_hash).expect("Invalid pubkey hash hex");
        script.push(pk_hash_bytes.len() as u8);
        script.extend(pk_hash_bytes);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        hex::encode(script)
    }

    pub fn sign_transaction(
        self,
        wallet: &Wallet,
        tx: &mut Transaction,
        utxo_set: &UtxoSet,
    ) -> Transaction {
        for i in 0..tx.vin.len() {
            let (txid, vout) = {
                let input = &tx.vin[i];
                (input.txid.clone(), input.vout)
            };
            let utxo = utxo_set.get_utxo(&txid, vout).expect("UTXO not found");
            if tx.witnesses.is_some() {
                // SegWit signing path
                let sighash = tx.compute_segwit_sighash(
                    i,
                    utxo.out.value,
                    &utxo.out.script_pubkey,
                    SIGHASH_ALL,
                );
                let sig = wallet.sign_data(&sighash);
                tx.witnesses.as_mut().unwrap()[i] = vec![sig, wallet.public_key.clone()];
                tx.vin[i].script_sig = "".to_string(); // MUST stay empty for native segwit
            } else {
                // Legacy signing path
                let sighash =
                    tx.compute_sighash(i, &utxo.out.script_pubkey, utxo.out.value, SIGHASH_ALL);
                let sig = wallet.sign_data(&sighash);
                let sig_bytes = hex::decode(&sig).expect("Invalid signature hex");
                let pubkey_bytes = hex::decode(&wallet.public_key).expect("Invalid pubkey hex");
                let mut script_sig = vec![];
                script_sig.push(sig_bytes.len() as u8);
                script_sig.extend_from_slice(&sig_bytes);
                script_sig.push(pubkey_bytes.len() as u8);
                script_sig.extend_from_slice(&pubkey_bytes);
                tx.vin[i].script_sig = hex::encode(script_sig);
            }
        }
        tx.txid = tx.compute_non_witness_txid();
        tx.hash = tx.compute_hash();

        return tx.clone();
    }

    pub fn create_new_transaction(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        value: u64,
        fee: u64,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1;
        let locktime = 0;
        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Select UTXOs
        let mut selected = vec![];
        let mut total_input = 0;
        for ((txid, vout), utxo) in utxo_set.utxos.iter() {
            if utxo.out.script_pubkey == sender_script {
                selected.push((txid.clone(), *vout, utxo.out.value));
                total_input += utxo.out.value;
                if total_input > value + fee {
                    break;
                }
            }
        }

        if total_input < value + fee {
            return Err("Insufficient funds".to_string());
        }

        let vin: Vec<TxInput> = selected
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid,
                vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();
        let vout = vec![
            TxOutput {
                value,
                script_pubkey: Self::create_p2pkh_script(&ChainUtil::pubkey_hash_from_address(
                    to_address,
                )?),
            },
            TxOutput {
                value: total_input - value - fee,
                script_pubkey: sender_script,
            },
        ];

        let tx = Self::new(version, locktime, vin, vout, None);
        Ok(tx)
    }

    pub fn create_new_multisig_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        m: u8,
        pubkeys_hex: Vec<String>,
        value: u64,
        fee: u64,
    ) -> Result<Self, String> {
        let version = 1;
        let locktime = 0;

        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Select UTXOs
        let mut selected = vec![];
        let mut total_input = 0;

        for ((txid, vout), utxo) in utxo_set.utxos.iter() {
            if utxo.out.script_pubkey == sender_script {
                selected.push((txid.clone(), *vout, utxo));
                total_input += utxo.out.value;
                if total_input >= value + fee {
                    break;
                }
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let vin: Vec<TxInput> = selected
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid,
                vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();

        // Create vout
        let redeem_script = Self::create_multisig_redeem_script(m, &pubkeys_hex);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut vout = vec![TxOutput {
            value,
            script_pubkey,
        }];

        // Add change output if necessary
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script,
            });
        }

        Ok(Self::new(version, locktime, vin, vout, None))
    }

    pub fn create_new_timelocked_cltv_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        value: u64,
        fee: u64,
        cltv_lock_time: u32,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1;
        let locktime = cltv_lock_time;

        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Select UTXOs
        let mut selected = vec![];
        let mut total_input = 0;

        for ((txid, vout), utxo) in utxo_set.utxos.iter() {
            if utxo.out.script_pubkey == sender_script {
                selected.push((txid.clone(), *vout, utxo));
                total_input += utxo.out.value;
                if total_input >= value + fee {
                    break;
                }
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let vin: Vec<TxInput> = selected
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid,
                vout,
                script_sig: "".to_string(),
                sequence: 0xffffffff,
            })
            .collect();

        // Create vout
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(to_address)
            .map_err(|_| "Invalid recipient address".to_string())?;
        let redeem_script = Self::create_cltv_redeem_script(cltv_lock_time, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut vout = vec![TxOutput {
            value,
            script_pubkey,
        }];

        // Add change output if necessary
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script,
            });
        }

        Ok(Self::new(version, locktime, vin, vout, None))
    }

    pub fn create_new_timelocked_csv_txn(
        wallet: &Wallet,
        utxo_set: &UtxoSet,
        value: u64,
        fee: u64,
        csv_lock_blocks: u32,
        to_address: &str,
    ) -> Result<Self, String> {
        let version = 1;
        let locktime = 0;

        let sender_script = Self::create_p2pkh_script(&wallet.public_key_hash);

        // Select UTXOs
        let mut selected = vec![];
        let mut total_input = 0;

        for ((txid, vout), utxo) in utxo_set.utxos.iter() {
            if utxo.out.script_pubkey == sender_script {
                selected.push((txid.clone(), *vout, utxo));
                total_input += utxo.out.value;
                if total_input >= value + fee {
                    break;
                }
            }
        }

        if total_input < value + fee {
            return Err(format!(
                "Insufficient funds: need {}, have {}",
                value + fee,
                total_input
            ));
        }

        // Create final vin
        let vin: Vec<TxInput> = selected
            .into_iter()
            .map(|(txid, vout, _)| TxInput {
                txid,
                vout,
                script_sig: "".to_string(),
                sequence: csv_lock_blocks,
            })
            .collect();

        // Create vout
        let pubkey_hash = ChainUtil::pubkey_hash_from_address(to_address)
            .map_err(|_| "Invalid recipient address".to_string())?;
        let redeem_script = Self::create_csv_redeem_script(csv_lock_blocks, &pubkey_hash);
        let script_pubkey = Self::create_p2sh_script(&redeem_script);
        let mut vout = vec![TxOutput {
            value,
            script_pubkey,
        }];

        // Add change output if necessary
        let change = total_input - value - fee;
        const DUST_THRESHOLD: u64 = 546;
        if change > DUST_THRESHOLD {
            vout.push(TxOutput {
                value: change,
                script_pubkey: sender_script,
            });
        }

        Ok(Self::new(version, locktime, vin, vout, None))
    }

    pub fn compute_sighash(
        &self,
        input_index: usize,
        script_pubkey: &str,
        value: u64,
        sighash_type: u32,
    ) -> Vec<u8> {
        let mut vin = self.vin.clone();
        for i in 0..vin.len() {
            vin[i].script_sig = if i == input_index {
                script_pubkey.to_string()
            } else {
                "".to_string()
            };
        }

        let mod_tx = Transaction {
            txid: "".to_string(),
            hash: "".to_string(),
            version: self.version,
            vin,
            vout: self.vout.clone(),
            witnesses: None,
            locktime: self.locktime,
        };

        let mut serialized = mod_tx.serialize_non_witness();
        serialized.extend(sighash_type.to_le_bytes());
        Self::double_sha(&serialized)
    }

    pub fn compute_segwit_sighash(
        &self,
        input_index: usize,
        input_value: u64,
        script_pubkey: &str,
        sighash_type: u32,
    ) -> Vec<u8> {
        let mut hasher = Sha256::new();

        hasher.update(self.version.to_le_bytes());

        // Hash prevouts
        let mut prevouts = Sha256::new();
        for input in &self.vin {
            prevouts.update(hex::decode(&input.txid).expect("Invalid txid hex"));
            prevouts.update(input.vout.to_le_bytes());
        }
        hasher.update(Self::double_sha(&prevouts.finalize()));

        // Hash sequences
        let mut sequences = Sha256::new();
        for input in &self.vin {
            sequences.update(input.sequence.to_le_bytes());
        }
        hasher.update(Self::double_sha(&sequences.finalize()));

        // Outpoint
        let input = &self.vin[input_index];
        hasher.update(hex::decode(&input.txid).expect("Invalid txid hex"));
        hasher.update(input.vout.to_le_bytes());

        // ScriptCode (for P2WPKH)
        let script_bytes = hex::decode(script_pubkey).expect("Invalid scriptPubKey hex");
        hasher.update((script_bytes.len() as u8).to_le_bytes());
        hasher.update(&script_bytes);

        // Value
        hasher.update(input_value.to_le_bytes());

        // Sequence
        hasher.update(input.sequence.to_le_bytes());

        // Hash outputs
        let mut outputs = Sha256::new();
        for output in &self.vout {
            outputs.update(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            outputs.update((script_bytes.len() as u8).to_le_bytes());
            outputs.update(&script_bytes);
        }
        hasher.update(Self::double_sha(&outputs.finalize()));

        hasher.update(self.locktime.to_le_bytes());
        hasher.update(sighash_type.to_le_bytes());

        Self::double_sha(&hasher.finalize())
    }

    pub fn serialize_non_witness(&self) -> Vec<u8> {
        let mut result = vec![];

        result.extend(self.version.to_le_bytes());
        result.push(self.vin.len() as u8);
        for input in &self.vin {
            result.extend(hex::decode(&input.txid).expect("Invalid txid hex"));
            result.extend(input.vout.to_le_bytes());
            let script_bytes = hex::decode(&input.script_sig).expect("Invalid scriptSig hex");
            result.push(script_bytes.len() as u8);
            result.extend(&script_bytes);
            result.extend(input.sequence.to_le_bytes());
        }

        result.push(self.vout.len() as u8);
        for output in &self.vout {
            result.extend(output.value.to_le_bytes());
            let script_bytes =
                hex::decode(&output.script_pubkey).expect("Invalid scriptPubKey hex");
            result.push(script_bytes.len() as u8);
            result.extend(&script_bytes);
        }

        result.extend(self.locktime.to_le_bytes());
        result
    }

    pub fn get_size_vsize_weight(&self) -> (usize, usize, u64) {
        let base_size = self.serialize_non_witness().len();
        let witness_size = self
            .witnesses
            .as_ref()
            .map(|witnesses| {
                let mut size = 0;
                for wit_stack in witnesses {
                    size += 1; // Stack size byte
                    for item in wit_stack {
                        let item_bytes = hex::decode(item).expect("Invalid witness item hex");
                        size += 1 + item_bytes.len(); // Length byte + data
                    }
                }
                if !witnesses.is_empty() {
                    size += 2; // Marker and flag bytes
                }
                size
            })
            .unwrap_or(0);
        let total_size = base_size + witness_size;
        let weight = (base_size * 4) as u64 + witness_size as u64;
        let vsize = (weight + 3) / 4; // Ceiling division

        (total_size, vsize as usize, weight)
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

    // Helper function to create a sample transaction
    pub fn create_sample_transaction() -> Transaction {
        let input = TxInput {
            txid: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vout: 0,
            script_sig: "4832".repeat(16).to_string(),
            sequence: 0xffffffff,
        };

        let output = TxOutput {
            value: 50000000, // 0.5 BTC in satoshis
            script_pubkey: "76a9141a1b2c3d4e5f6789012345678901234567890abc88ac".to_string(), // P2PKH script
        };

        Transaction::new(
            1, // version
            0, // locktime
            vec![input],
            vec![output],
            None, // no witnesses
        )
    }

    // Helper function to create a second sample transaction with different output
    pub fn create_sample_transaction_with_different_output() -> Transaction {
        let input = TxInput {
            txid: "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            vout: 0,
            script_sig: "4831".repeat(16).to_string(),
            sequence: 0xffffffff,
        };

        let output = TxOutput {
            value: 25000000, // 0.25 BTC in satoshis
            script_pubkey: "76a9141a1b2c3d4e5f6789012345678901234567890abc88ac".to_string(), // Different P2PKH script
        };

        Transaction::new(
            1, // version
            0, // locktime
            vec![input],
            vec![output],
            None, // no witnesses
        )
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Transaction {{\n  txid: {},\n  hash: {},\n  version: {},\n  locktime: {},",
            self.txid, self.hash, self.version, self.locktime
        )?;

        writeln!(f, "  inputs: [")?;
        for input in &self.vin {
            writeln!(
                f,
                "    TxInput {{ txid: {}, vout: {}, script_sig: {}, sequence: {} }},",
                input.txid, input.vout, input.script_sig, input.sequence
            )?;
        }
        writeln!(f, "  ],")?;

        writeln!(f, "  outputs: [")?;
        for output in &self.vout {
            writeln!(
                f,
                "    TxOutput {{ value: {}, script_pubkey: {} }},",
                output.value, output.script_pubkey
            )?;
        }
        writeln!(f, "  ],")?;

        writeln!(
            f,
            "  witnesses: {}\n}}",
            if let Some(w) = &self.witnesses {
                format!("{:?}", w)
            } else {
                "none".to_string()
            }
        )
    }
}
