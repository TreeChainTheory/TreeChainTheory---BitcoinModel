use crate::chain_util::ChainUtil;
use crate::config::{CHILDREN, USER_TXN_FREERATE};
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;
use k256::ecdsa::signature::Verifier;
use k256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SIGHASH_ALL: u32 = 0x01;
pub const MAX_MEMPOOL_SIZE: usize = 100 * 1024 * 1024; // 100 MB in bytes
pub const DEFAULT_MIN_FEE_RATE: u64 = USER_TXN_FREERATE; // satoshis per vB
pub const MAX_TX_SIZE: usize = 100_000; // Bitcoin's max tx size (vB)

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolEntry {
    pub tx: Transaction,
    pub fee: u64,                  // Total fee in satoshis
    pub vsize: usize,              // Virtual size in vB
    pub fee_rate: f64,             // Fee rate in sat/vB
    pub added_time: u64,           // Unix timestamp (ms) when added
    pub depends: HashSet<String>,  // Parent txids this tx depends on
    pub children: HashSet<String>, // Child txids spending this tx's outputs
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionPool {
    pub pool: HashMap<String, MempoolEntry>, // txid -> MempoolEntry
    pub utxo_set: UtxoSet,                   // Temporary UTXO set for mempool
    pub total_size: usize,                   // Total size in bytes
}

impl TransactionPool {
    pub fn new() -> Self {
        TransactionPool {
            pool: HashMap::new(),
            utxo_set: UtxoSet::new(),
            total_size: 0,
        }
    }

    pub fn calculate_vsize(&self, tx: &Transaction) -> usize {
        let (size, vsize, _) = tx.get_size_vsize_weight();
        vsize
    }

    pub fn validate_transaction(
        &self,
        tx: &Transaction,
        utxo_set: &UtxoSet,
        min_fee_rate: f64,
    ) -> Result<(u64, usize, f64, HashSet<String>), String> {
        let vsize = self.calculate_vsize(tx);
        if vsize > MAX_TX_SIZE {
            return Err(format!("Transaction too large: {} vB", vsize));
        }

        // Check locktime
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        if tx.locktime > current_time && tx.locktime < 500_000_000 {
            return Err(format!(
                "Transaction locktime {} not yet reached",
                tx.locktime
            ));
        }

        // Verify inputs are unspent and calculate total input value
        let mut input_value = 0;
        let mut depends = HashSet::new();
        for input in &tx.vin {
            // Check if input is in main UTXO set or mempool UTXO set
            let utxo = match utxo_set.get_utxo(&input.txid, input.vout) {
                Some(u) => u,
                None => match self.utxo_set.get_utxo(&input.txid, input.vout) {
                    Some(u) => u,
                    None => return Err(format!("UTXO not found: {}:{}", input.txid, input.vout)),
                },
            };
            input_value += utxo.out.value;

            // Track dependencies (parent txids in mempool)
            if self.pool.contains_key(&input.txid) {
                depends.insert(input.txid.clone());
            }

            // Verify script_sig (for non-SegWit) or witness (for SegWit)
            let input_index = tx.vin.iter().position(|x| x == input).unwrap();
            if tx.witnesses.is_none() {
                let sighash =
                    tx.compute_sighash(input_index, &utxo.out.script_pubkey, 0, SIGHASH_ALL);
                let (sig_hex, pubkey_hex) = Self::parse_script_sig(&input.script_sig)?;
                if !Self::verify_signature(&sighash, &sig_hex, &pubkey_hex) {
                    return Err(format!(
                        "Invalid signature for input {}:{}",
                        input.txid, input.vout
                    ));
                }
            } else {
                let witnesses = tx.witnesses.as_ref().unwrap();
                let witness = witnesses.get(input_index).ok_or("Missing witness data")?;
                if witness.len() < 2 {
                    return Err("Invalid witness format".to_string());
                }
                let sighash = tx.compute_segwit_sighash(
                    input_index,
                    utxo.out.value,
                    &utxo.out.script_pubkey,
                    SIGHASH_ALL,
                );
                if !Self::verify_signature(&sighash, &witness[0], &witness[1]) {
                    return Err(format!(
                        "Invalid witness signature for input {}:{}",
                        input.txid, input.vout
                    ));
                }
            }
        }

        // Calculate output value and fee
        let output_value: u64 = tx.vout.iter().map(|out| out.value).sum();
        if output_value > input_value {
            return Err("Output value exceeds input value".to_string());
        }
        let fee = input_value - output_value;

        // Check fee rate
        let fee_rate = fee as f64 / vsize as f64;
        if fee_rate < min_fee_rate {
            return Err(format!(
                "Fee rate {} sat/vB below minimum {}",
                fee_rate, min_fee_rate
            ));
        }

        // Check for double-spends
        for input in &tx.vin {
            if self.pool.values().any(|entry| {
                entry
                    .tx
                    .vin
                    .iter()
                    .any(|in_| in_.txid == input.txid && in_.vout == input.vout)
            }) {
                return Err(format!(
                    "Double-spend detected for {}:{}",
                    input.txid, input.vout
                ));
            }
        }

        Ok((fee, vsize, fee_rate, depends))
    }

    pub fn add_transaction(&mut self, tx: Transaction, utxo_set: &UtxoSet) -> Result<(), String> {
        // Validate transaction
        let (fee, vsize, fee_rate, depends) =
            self.validate_transaction(&tx, utxo_set, DEFAULT_MIN_FEE_RATE as f64)?;

        // Check mempool size limit
        if self.total_size + vsize > MAX_MEMPOOL_SIZE {
            self.evict_low_fee_transactions(vsize)?;
        }

        // Check dependencies
        for parent_txid in &depends {
            if !self.pool.contains_key(parent_txid) && !utxo_set.has_utxo(parent_txid, 0) {
                return Err(format!("Parent transaction {} not found", parent_txid));
            }
        }

        // Add to mempool
        let txid = tx.txid.clone();
        let entry = MempoolEntry {
            tx: tx.clone(),
            fee,
            vsize,
            fee_rate,
            added_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            depends,
            children: HashSet::new(),
        };
        self.pool.insert(txid.clone(), entry.clone());
        self.total_size += vsize;

        // Update children of parent transactions
        for parent_txid in &entry.depends {
            if let Some(parent_entry) = self.pool.get_mut(parent_txid) {
                parent_entry.children.insert(txid.clone());
            }
        }

        // Add transaction outputs to mempool UTXO set
        for (vout_idx, out) in tx.vout.iter().enumerate() {
            let utxo = Utxo::new(out.clone(), 0, false); // queue_index 0 for mempool
            self.utxo_set.add_utxo(txid.clone(), vout_idx as u32, utxo);
        }

        Ok(())
    }

    pub fn remove_transaction(&mut self, txid: &str) -> Option<Vec<Transaction>> {
        let entry = self.pool.remove(txid)?;
        self.total_size -= entry.vsize;

        // Remove from mempool UTXO set
        for vout_idx in 0..entry.tx.vout.len() as u32 {
            self.utxo_set.remove_utxo(txid, vout_idx);
        }

        // Collect descendants to remove
        let mut removed_txs = vec![entry.tx.clone()];
        let children = entry.children.clone();
        for child_txid in children {
            if let Some(child_txs) = self.remove_transaction(&child_txid) {
                removed_txs.extend(child_txs);
            }
        }

        // Update parent transactions
        for parent_txid in entry.depends {
            if let Some(parent_entry) = self.pool.get_mut(&parent_txid) {
                parent_entry.children.remove(txid);
            }
        }

        Some(removed_txs)
    }

    pub fn evict_low_fee_transactions(&mut self, required_space: usize) -> Result<(), String> {
        let mut sorted_entries: Vec<(&String, &MempoolEntry)> = self
            .pool
            .iter()
            .filter(|(_, entry)| entry.depends.is_empty())
            .collect();
        sorted_entries.sort_by(|a, b| a.1.fee_rate.partial_cmp(&b.1.fee_rate).unwrap());

        let mut freed_space = 0;
        let mut txids_to_remove = Vec::new();
        for (txid, entry) in sorted_entries {
            if freed_space >= required_space {
                break;
            }
            txids_to_remove.push(txid.clone());
            freed_space += entry.vsize;
        }

        if freed_space < required_space {
            return Err("Cannot free enough space in mempool".to_string());
        }

        for txid in txids_to_remove {
            self.remove_transaction(&txid);
        }

        Ok(())
    }

    pub fn select_transactions(&self, max_block_size: usize, align: u8) -> Vec<Transaction> {
        let mut sorted_entries: Vec<&MempoolEntry> = self.pool.values().collect();
        sorted_entries.sort_by(|a, b| b.fee_rate.partial_cmp(&a.fee_rate).unwrap());

        let mut selected_txs = Vec::new();
        let mut total_size = 0;

        for entry in sorted_entries {
            if !self.tx_suitable_for_align(&entry.tx, align) {
                continue;
            }
            if total_size + entry.vsize > max_block_size {
                continue;
            }

            // Check if all dependencies are included
            let all_deps_included = entry.depends.iter().all(|dep_txid| {
                selected_txs
                    .iter()
                    .any(|tx: &Transaction| tx.txid == *dep_txid)
                    || self.pool.get(dep_txid).is_none()
            });
            if !all_deps_included {
                continue;
            }

            selected_txs.push(entry.tx.clone());
            total_size += entry.vsize;
        }

        selected_txs
    }

    pub fn tx_suitable_for_align(&self, tx: &Transaction, align: u8) -> bool {
        let txid = &tx.txid;
        if txid.is_empty() {
            return false;
        }
        let last_char = txid.chars().last().unwrap();
        let last_digit = u32::from_str_radix(&last_char.to_string(), 16).unwrap_or(0);
        ((last_digit % CHILDREN as u32) + 1) == align as u32
    }

    pub fn replace_transaction(
        &mut self,
        new_tx: Transaction,
        utxo_set: &UtxoSet,
    ) -> Result<(), String> {
        // Validate new transaction
        let (new_fee, new_vsize, new_fee_rate, new_depends) =
            self.validate_transaction(&new_tx, utxo_set, DEFAULT_MIN_FEE_RATE as f64)?;

        // Check if it replaces an existing transaction
        let mut replaced_txids = HashSet::new();
        for input in &new_tx.vin {
            for (txid, entry) in &self.pool {
                if entry
                    .tx
                    .vin
                    .iter()
                    .any(|in_| in_.txid == input.txid && in_.vout == input.vout)
                {
                    replaced_txids.insert(txid.clone());
                }
            }
        }

        // Check RBF conditions (BIP-125)
        if !replaced_txids.is_empty() {
            let mut total_replaced_fee = 0;
            let mut total_replaced_size = 0;
            for txid in &replaced_txids {
                let entry = self
                    .pool
                    .get(txid)
                    .ok_or("Replaced transaction not found")?;
                total_replaced_fee += entry.fee;
                total_replaced_size += entry.vsize;
                if entry.fee_rate >= new_fee_rate {
                    return Err("New transaction fee rate not higher than replaced".to_string());
                }
            }
            if new_fee <= total_replaced_fee {
                return Err("New transaction fee not higher than replaced".to_string());
            }

            // Remove replaced transactions
            for txid in replaced_txids {
                self.remove_transaction(&txid);
            }
        }

        // Add new transaction
        self.add_transaction(new_tx, utxo_set)
    }

    pub fn parse_script_sig(script_sig: &str) -> Result<(String, String), String> {
        let bytes =
            hex::decode(script_sig).map_err(|e| format!("Invalid script_sig hex: {}", e))?;
        if bytes.len() < 2 {
            return Err("Invalid script_sig format".to_string());
        }
        let sig_len = bytes[0] as usize;
        if bytes.len() < 1 + sig_len + 1 {
            return Err("Invalid script_sig length".to_string());
        }
        let sig = hex::encode(&bytes[1..1 + sig_len]);
        let pubkey_len = bytes[1 + sig_len] as usize;
        if bytes.len() < 1 + sig_len + 1 + pubkey_len {
            return Err("Invalid script_sig length".to_string());
        }
        let pubkey = hex::encode(&bytes[2 + sig_len..2 + sig_len + pubkey_len]);
        Ok((sig, pubkey))
    }

    pub fn verify_signature(sighash: &[u8], sig_hex: &str, pubkey_hex: &str) -> bool {
        let sig_bytes = match hex::decode(sig_hex) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let pubkey_bytes = match hex::decode(pubkey_hex) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let signature = match Signature::from_der(&sig_bytes) {
            Ok(sig) => sig,
            Err(_) => return false,
        };
        let verifying_key = match VerifyingKey::from_sec1_bytes(&pubkey_bytes) {
            Ok(key) => key,
            Err(_) => return false,
        };
        verifying_key.verify(sighash, &signature).is_ok()
    }

    pub fn clear(&mut self) {
        self.pool.clear();
        self.utxo_set = UtxoSet::new();
        self.total_size = 0;
    }

    pub fn remove_confirmed_transactions(&mut self, block_txs: &[Transaction]) {
        for tx in block_txs {
            self.remove_transaction(&tx.txid);
        }
    }

    pub fn get_transaction(&self, txid: &str) -> Option<&Transaction> {
        self.pool.get(txid).map(|entry| &entry.tx)
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    pub fn get_transactions_spending_utxo(&self, txid: &str, vout: u32) -> Vec<&Transaction> {
        self.pool
            .values()
            .filter(|entry| {
                entry
                    .tx
                    .vin
                    .iter()
                    .any(|input| input.txid == txid && input.vout == vout)
            })
            .map(|entry| &entry.tx)
            .collect()
    }

    pub fn reorg(&mut self, utxo_set: &UtxoSet) {
        let mut invalid_txids = Vec::new();
        for (txid, entry) in &self.pool {
            if let Err(_) =
                self.validate_transaction(&entry.tx, utxo_set, DEFAULT_MIN_FEE_RATE as f64)
            {
                invalid_txids.push(txid.clone());
            }
        }
        for txid in invalid_txids {
            self.remove_transaction(&txid);
        }
    }
}
