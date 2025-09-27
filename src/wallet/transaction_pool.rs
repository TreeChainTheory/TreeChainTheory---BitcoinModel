use crate::chain_util::ChainUtil;
use crate::config::{CHILDREN, SIGHASH_ALL, USER_TXN_FREERATE};
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;
use k256::ecdsa::signature::Verifier;
use k256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

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
        min_fee_rate: f64,
        utxt_set: &UtxoSet,
    ) -> Result<(u64, usize, f64, HashSet<String>), String> {
        let vsize = self.calculate_vsize(tx);
        if vsize > MAX_TX_SIZE {
            return Err(format!("Transaction too large: {} vB", vsize));
        }

        // Check locktime first
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        if tx.locktime > current_time && tx.locktime < 500_000_000 {
            return Err(format!(
                "Transaction locktime {} not yet reached (current: {})",
                tx.locktime, current_time
            ));
        }

        // Verify inputs are unspent and calculate total input value
        let mut input_value = 0;
        let mut depends = HashSet::new();
        for input in &tx.vin {
            let utxo = utxt_set
                .get_utxo(&input.txid, input.vout)
                .or_else(|| self.utxo_set.get_utxo(&input.txid, input.vout))
                .ok_or(format!("UTXO not found: {}:{}", input.txid, input.vout))?;
            input_value += utxo.out.value;

            if self.pool.contains_key(&input.txid) {
                depends.insert(input.txid.clone());
            }

            let input_index = tx.vin.iter().position(|x| x == input).unwrap();
            if tx.witnesses.is_none() {
                let sighash = tx.compute_sighash(
                    input_index,
                    &utxo.out.script_pubkey,
                    utxo.out.value,
                    SIGHASH_ALL,
                );
                let (sig_hex, pubkey_hex) =
                    Self::parse_script_sig(&input.script_sig).map_err(|e| {
                        format!("Script_sig error for {}:{}: {}", input.txid, input.vout, e)
                    })?;
                if !Self::verify_signature(&sighash, &sig_hex, &pubkey_hex) {
                    return Err(format!(
                        "Invalid signature for input {}:{}",
                        input.txid, input.vout
                    ));
                }
            } else {
                let witnesses = tx.witnesses.as_ref().unwrap();
                let witness = witnesses.get(input_index).ok_or(format!(
                    "Missing witness data for {}:{}",
                    input.txid, input.vout
                ))?;
                if witness.len() < 2 {
                    return Err(format!(
                        "Invalid witness format for {}:{}",
                        input.txid, input.vout
                    ));
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

        let output_value: u64 = tx.vout.iter().map(|out| out.value).sum();
        if output_value > input_value {
            return Err(format!(
                "Output value {} exceeds input value {}",
                output_value, input_value
            ));
        }
        let fee = input_value - output_value;

        let fee_rate = fee as f64 / vsize as f64;
        if fee_rate < min_fee_rate {
            return Err(format!(
                "Fee rate {} sat/vB below minimum {}",
                fee_rate, min_fee_rate
            ));
        }
        // println!("self.pool values: {:?}", self.pool.values());
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
        println!("Adding transaction to mempool: {}", tx.txid);
        // Validate transaction
        let (fee, vsize, fee_rate, depends) =
            self.validate_transaction(&tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)?;

        // Check mempool size limit
        if self.total_size + vsize > MAX_MEMPOOL_SIZE {
            self.evict_low_fee_transactions(vsize)?;
        }

        // Check dependencies
        for parent_txid in &depends {
            if let Some(parent) = self.pool.get_mut(parent_txid) {
                parent.children.insert(tx.txid.clone());
            } else {
                return Err("Parent not in pool".to_string());
            }
        }

        // Remove spent mempool UTXOs
        for input in &tx.vin {
            if depends.contains(&input.txid) {
                self.utxo_set.remove_utxo(&input.txid, input.vout);
            }
        }

        // Add new UTXOs
        for (i, _) in tx.vout.iter().enumerate() {
            let utxo = Utxo::extract_utxo(&tx, i as u32, 0).unwrap();
            self.utxo_set.add_utxo(tx.txid.clone(), i as u32, utxo);
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

        Ok(())
    }

    pub fn remove_transaction(&mut self, txid: &str) {
        if let Some(entry) = self.pool.remove(txid) {
            self.total_size -= entry.vsize;

            // Recursively remove children first
            let children = entry.children.clone();
            for child in children {
                self.remove_transaction(&child);
            }

            // Remove its UTXOs
            for i in 0..entry.tx.vout.len() as u32 {
                self.utxo_set.remove_utxo(&entry.tx.txid, i);
            }

            // Restore spent mempool UTXOs
            for input in &entry.tx.vin {
                if entry.depends.contains(&input.txid) {
                    if let Some(parent) = self.pool.get(&input.txid) {
                        let utxo = Utxo::extract_utxo(&parent.tx, input.vout, 0).unwrap();
                        self.utxo_set.add_utxo(input.txid.clone(), input.vout, utxo);
                    }
                }
            }

            // Remove from parents' children
            for parent_txid in entry.depends {
                if let Some(parent) = self.pool.get_mut(&parent_txid) {
                    parent.children.remove(txid);
                }
            }
        }
    }

    pub fn remove_confirmed_txn(&mut self, txid: &str) {
        if let Some(entry) = self.pool.remove(txid) {
            self.total_size -= entry.vsize;

            // Remove its UTXOs from mempool UTXO set (do NOT restore spent UTXOs)
            for i in 0..entry.tx.vout.len() as u32 {
                self.utxo_set.remove_utxo(&entry.tx.txid, i);
            }

            // Remove from parent transactions' children lists
            for parent_txid in &entry.depends {
                if let Some(parent) = self.pool.get_mut(parent_txid) {
                    parent.children.remove(txid);
                }
            }

            println!(
                "✅ Removed confirmed transaction {} from mempool (size: {} vB)",
                txid, entry.vsize
            );
        }
    }

    pub fn evict_low_fee_transactions(&mut self, required_space: usize) -> Result<(), String> {
        let mut sorted_entries: Vec<(&String, &MempoolEntry)> = self.pool.iter().collect();
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

        for txid in txids_to_remove {
            self.remove_transaction(&txid);
        }

        Ok(())
    }

    pub fn select_transactions(&self, max_block_weight: usize, align: u8) -> Vec<Transaction> {
        let mut sorted_entries: Vec<&MempoolEntry> = self.pool.values().collect();
        sorted_entries.sort_by(|a, b| b.fee_rate.partial_cmp(&a.fee_rate).unwrap());

        let mut selected_txs = Vec::new();
        let mut total_weight = 0;

        for entry in sorted_entries {
            let tx_weight = entry.tx.get_size_vsize_weight().2;
            println!(
                "Evaluating txid={} (fee_rate={:.2}, weight={})",
                entry.tx.txid, entry.fee_rate, tx_weight
            );

            if !self.tx_suitable_for_align(&entry.tx, align) {
                println!("  Skipped: tx_suitable_for_align failed (align={})", align);
                continue;
            }

            if total_weight + tx_weight as usize > max_block_weight {
                println!(
                    "  Skipped: Weight limit exceeded (total_weight={} + tx_weight={} > max_block_weight={})",
                    total_weight, tx_weight, max_block_weight
                );
                continue;
            }

            let all_deps_included = entry.depends.iter().all(|dep_txid| {
                let included = selected_txs
                    .iter()
                    .any(|tx: &Transaction| tx.txid == *dep_txid)
                    || self.pool.get(dep_txid).is_none();
                if !included {
                    println!("  Dependency {} not included", dep_txid);
                }
                included
            });
            if !all_deps_included {
                println!("  Skipped: Dependencies not satisfied");
                continue;
            }

            println!("  Selected txid={}", entry.tx.txid);
            selected_txs.push(entry.tx.clone());
            total_weight += tx_weight as usize;
        }

        println!("Total selected transactions: {}", selected_txs.len());
        selected_txs
    }

    pub fn tx_suitable_for_align(&self, tx: &Transaction, align: u8) -> bool {
        if align == 0 {
            return true; // this is only for testing purposes
        }
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
        println!("replace_transaction called for txid: {}", new_tx.txid);
        // Validate new transaction
        let (new_fee, new_vsize, new_fee_rate, new_depends) = self
            .validate_transaction(&new_tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)
            .map_err(|e| format!("Validation failed for txid {}: {}", new_tx.txid, e))?;

        // Check if it replaces an existing transaction
        let mut replaced_txids = HashSet::new();
        println!("Checking for replaced transactions");
        for input in &new_tx.vin {
            println!("Checking input {}:{}", input.txid, input.vout);
            for (txid, entry) in &self.pool {
                if entry.tx.vin.iter().any(|in_| {
                    let matches = in_.txid == input.txid && in_.vout == input.vout;
                    if matches {
                        println!(
                            "Found matching input in txid {}: {}:{}",
                            txid, in_.txid, in_.vout
                        );
                    }
                    matches
                }) {
                    println!("Adding txid {} to replaced_txids", txid);
                    replaced_txids.insert(txid.clone());
                }
            }
        }
        println!("replaced_txids: {:?}", replaced_txids);

        // Check RBF conditions (BIP-125)
        if !replaced_txids.is_empty() {
            let mut total_replaced_fee = 0;
            let mut total_replaced_size = 0;
            for txid in &replaced_txids {
                let entry = self
                    .pool
                    .get(txid)
                    .ok_or(format!("Replaced transaction {} not found", txid))?;
                total_replaced_fee += entry.fee;
                total_replaced_size += entry.vsize;
                if entry.fee_rate >= new_fee_rate {
                    return Err(format!(
                        "New transaction fee rate {} not higher than replaced {} for txid {}",
                        new_fee_rate, entry.fee_rate, txid
                    ));
                }
            }
            if new_fee <= total_replaced_fee {
                return Err(format!(
                    "New transaction fee {} not higher than replaced total {} for txids {:?}",
                    new_fee, total_replaced_fee, replaced_txids
                ));
            }

            // Remove replaced transactions
            for txid in replaced_txids {
                println!("Removing replaced transaction {}", txid);
                self.remove_transaction(&txid);
                println!(
                    "After removal, is tx {} present: {}",
                    txid,
                    self.pool.contains_key(&txid)
                );
            }
        } else {
            println!("No transactions to replace, proceeding to add new transaction");
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
                self.validate_transaction(&entry.tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)
            {
                invalid_txids.push(txid.clone());
            }
        }
        for txid in invalid_txids {
            self.remove_transaction(&txid);
        }
    }
}
