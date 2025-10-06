use crate::chain_util::ChainUtil;
use crate::config::{CHILDREN, SIGHASH_ALL, USER_TXN_FREERATE};
use crate::treechain;
use crate::treechain::treechain::TreeChain;
use crate::wallet::transaction::{Transaction, TxInput, TxOutput};
use crate::wallet::utxo::{Utxo, UtxoSet};
use crate::wallet::wallet::Wallet;
use hex;
use k256::ecdsa::signature::Verifier;
use k256::ecdsa::{Signature, VerifyingKey};
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_MEMPOOL_SIZE: usize = 100 * 1024 * 1024; // 100 MB in bytes
pub const DEFAULT_MIN_FEE_RATE: u64 = USER_TXN_FREERATE; // satoshis per vB
pub const MAX_TX_SIZE: usize = 100_000; // Bitcoin's max tx size (vB)

#[derive(Debug)]
pub enum Op {
    Push(Vec<u8>),
    Code(u8),
}

impl Op {
    fn as_push(&self) -> Option<&Vec<u8>> {
        if let Op::Push(data) = self {
            Some(data)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum ScriptInfo {
    P2pkh { pubkey_hash: String },
    Multisig { m: u8, pubkeys: Vec<String>, n: u8 },
    Cltv { lock: u32, pubkey_hash: String },
    Csv { lock: u32, pubkey_hash: String },
}

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

    // pub fn validate_transaction(
    //     &self,
    //     tx: &Transaction,
    //     min_fee_rate: f64,
    //     utxt_set: &UtxoSet,
    // ) -> Result<(u64, usize, f64, HashSet<String>), String> {
    //     //check txid and hash
    //     if !Transaction::check_txid_and_hash(tx) {
    //         return Err(format!(
    //             "Transaction txid and hash dosnt match calculation: {}",
    //             tx.txid
    //         ));
    //     }

    //     let vsize = self.calculate_vsize(tx);
    //     if vsize > MAX_TX_SIZE {
    //         return Err(format!("Transaction too large: {} vB", vsize));
    //     }

    //     // Check locktime first
    //     let current_time = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap_or_default()
    //         .as_secs() as u32;
    //     if tx.locktime > current_time && tx.locktime < 500_000_000 {
    //         return Err(format!(
    //             "Transaction locktime {} not yet reached (current: {})",
    //             tx.locktime, current_time
    //         ));
    //     }
    //     println!("validate transaction here 1");
    //     // Verify inputs are unspent and calculate total input value
    //     let mut input_value = 0;
    //     let mut depends = HashSet::new();
    //     let mut utxos = vec![];
    //     for input in &tx.vin {
    //         let utxo = utxt_set
    //             .get_utxo(&input.txid, input.vout)
    //             .or_else(|| self.utxo_set.get_utxo(&input.txid, input.vout))
    //             .ok_or(format!("UTXO not found: {}:{}", input.txid, input.vout))?;
    //         input_value += utxo.out.value;
    //         utxos.push(utxo.clone());

    //         if self.pool.contains_key(&input.txid) {
    //             depends.insert(input.txid.clone());
    //         }
    //     }
    //     println!("validate transaction here 2");

    //     // Verify signatures and scripts for all inputs
    //     let witnesses = tx.witnesses.as_ref();
    //     for (i, input) in tx.vin.iter().enumerate() {
    //         let utxo = &utxos[i];
    //         println!("validate transaction here 3");
    //         Self::verify_input(
    //             tx,
    //             i,
    //             input,
    //             utxo,
    //             utxo.out.value,
    //             current_time,
    //             witnesses.and_then(|w| w.get(i)),
    //         )?;
    //     }

    //     // Check outputs
    //     let mut output_value = 0;
    //     for output in &tx.vout {
    //         if output.value == 0 {
    //             return Err("Output value must be positive".to_string());
    //         }
    //         output_value += output.value;
    //     }

    //     if output_value > input_value {
    //         return Err(format!(
    //             "Outputs {} exceed inputs {}",
    //             output_value, input_value
    //         ));
    //     }

    //     let fee = input_value - output_value;
    //     let fee_rate = fee as f64 / vsize as f64;
    //     if fee_rate < min_fee_rate {
    //         return Err(format!(
    //             "Fee rate {} below minimum {}",
    //             fee_rate, min_fee_rate
    //         ));
    //     }

    //     Ok((fee, vsize, fee_rate, depends))
    // }

    pub fn validate_transaction(
        &self,
        tx: &Transaction,
        min_fee_rate: f64,
        utxo_set: &UtxoSet,
        treechain: &TreeChain,
    ) -> Result<(u64, usize, f64, HashSet<String>), String> {
        // Check txid and hash
        if !Transaction::check_txid_and_hash(tx) {
            return Err(format!(
                "Transaction txid and hash doesn't match calculation: {}",
                tx.txid
            ));
        }

        let vsize = self.calculate_vsize(tx);
        if vsize > MAX_TX_SIZE {
            return Err(format!("Transaction too large: {} vB", vsize));
        }

        // Check locktime first (global txn locktime)
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
        println!("validate transaction here 1");

        // Verify inputs are unspent and calculate total input value
        let mut input_value = 0u64;
        let mut depends = HashSet::new();
        let mut utxos = vec![];
        for input in &tx.vin {
            let utxo = utxo_set
                .get_utxo(&input.txid, input.vout)
                .or_else(|| self.utxo_set.get_utxo(&input.txid, input.vout))
                .ok_or(format!("UTXO not found: {}:{}", input.txid, input.vout))?;
            input_value += utxo.out.value;
            utxos.push(utxo.clone());

            if self.pool.contains_key(&input.txid) {
                depends.insert(input.txid.clone());
            }
        }
        println!("validate transaction here 2");

        // Current height for CSV checks
        let current_height = treechain.get_max_queue_index();

        // Verify signatures, scripts, and timelocks for all inputs
        let witnesses = tx.witnesses.as_ref();
        for (i, input) in tx.vin.iter().enumerate() {
            let utxo = &utxos[i];
            println!("validate transaction here 3");

            let script_pubkey = &utxo.out.script_pubkey;

            // Timelock checks ONLY for CLTV/CSV P2SH inputs (skip for multisig or other P2SH)
            if script_pubkey.starts_with("a914") && script_pubkey.ends_with("87") {
                let redeem_opt = Transaction::extract_redeem_from_scriptsig(&input.script_sig);
                let redeem = match redeem_opt {
                    Some(r) => r,
                    None => {
                        return Err(format!(
                            "Missing redeem in script_sig for P2SH input {}:{}",
                            input.txid, input.vout
                        ));
                    }
                };
                let redeem_bytes = match hex::decode(&redeem) {
                    Ok(b) => b,
                    Err(_) => {
                        return Err(format!(
                            "Invalid redeem hex for input {}:{}",
                            input.txid, input.vout
                        ));
                    }
                };
                let ops = Self::parse_script_ops(&redeem_bytes);
                if let Ok(info) = Self::get_script_info(&ops) {
                    match info {
                        ScriptInfo::Cltv { lock, .. } => {
                            println!("info {:?}", info);
                            if tx.locktime < lock as u32 {
                                return Err(format!(
                                    "CLTV not met for input {}:{}: locktime {} < lock {}",
                                    input.txid, input.vout, tx.locktime, lock
                                ));
                            }
                            if tx.version < 2 || input.sequence >= 0xFFFFFFFE {
                                return Err(format!(
                                    "Invalid version/sequence for CLTV input {}:{}",
                                    input.txid, input.vout
                                ));
                            }
                        }
                        ScriptInfo::Csv { lock, .. } => {
                            println!("info {:?}", info);
                            let blocks_since =
                                current_height.saturating_sub(utxo.queue_index as u64);
                            if blocks_since < lock as u64 {
                                return Err(format!(
                                    "CSV not met for input {}:{}: {} blocks < lock {} (created at {})",
                                    input.txid, input.vout, blocks_since, lock, utxo.queue_index
                                ));
                            }
                            if tx.version < 2 {
                                return Err(format!(
                                    "Invalid version for CSV input {}:{}",
                                    input.txid, input.vout
                                ));
                            }
                        }
                        ScriptInfo::Multisig { .. } => {
                            println!("info {:?}", info);

                            // No additional timelock checks for multisig; proceed to full verification
                        }
                        _ => {
                            println!("info {:?}", info);
                        } // Other scripts OK, no timelock
                    }
                }
            }

            // Full script and signature verification for ALL inputs (handles P2PKH, P2SH multisig, timelock, etc.)
            Self::verify_input(
                tx,
                i,
                input,
                utxo,
                utxo.out.value,
                current_time,
                witnesses.and_then(|w| w.get(i)),
                treechain,
            )?;
        }

        // Check outputs
        let mut output_value = 0u64;
        for output in &tx.vout {
            if output.value == 0 {
                return Err("Output value must be positive".to_string());
            }
            output_value += output.value;
        }

        if output_value > input_value {
            return Err(format!(
                "Outputs {} exceed inputs {}",
                output_value, input_value
            ));
        }

        let fee = input_value - output_value;
        let fee_rate = fee as f64 / vsize as f64;
        if fee_rate < min_fee_rate {
            return Err(format!(
                "Fee rate {} below minimum {}",
                fee_rate, min_fee_rate
            ));
        }

        // Double-spend check in mempool
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

    fn verify_input(
        tx: &Transaction,
        input_index: usize,
        input: &TxInput,
        utxo: &Utxo,
        input_value: u64,
        current_time: u32,
        witness: Option<&Vec<String>>,
        treechain: &TreeChain,
    ) -> Result<(), String> {
        println!("called verify input");
        let script_pubkey = &utxo.out.script_pubkey;
        let script_pubkey_bytes =
            hex::decode(script_pubkey).map_err(|e| format!("Invalid script_pubkey hex: {}", e))?;
        println!("verify input here 1");

        let current_height = treechain
            .blocks
            .values()
            .map(|b| b.pqp_entry.queue_index as u64)
            .max()
            .unwrap_or(0);

        if utxo.f_coinbase && current_height < (utxo.queue_index as u64 + 50) {
            return Err("Coinbase UTXO not mature (requires 50 confirmations)".to_string());
        }

        let is_segwit = witness.is_some();
        let stack: Vec<String>;

        if is_segwit {
            stack = witness.unwrap().clone();
        } else {
            let script_sig_bytes = hex::decode(&input.script_sig)
                .map_err(|e| format!("Invalid script_sig hex: {}", e))?;
            stack = Self::parse_stack(&script_sig_bytes)?;
        }
        println!("verify input here 2");

        if script_pubkey_bytes.len() == 25
            && script_pubkey_bytes[0] == 0x76
            && script_pubkey_bytes[1] == 0xa9
            && script_pubkey_bytes[2] == 0x14
            && script_pubkey_bytes[23] == 0x88
            && script_pubkey_bytes[24] == 0xac
        {
            // Legacy P2PKH
            if stack.len() != 2 {
                return Err("Invalid stack for P2PKH".to_string());
            }
            let sig_hex = &stack[0];
            let pubkey_hex = &stack[1];
            let pubkey_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
            let expected_hash = hex::encode(&script_pubkey_bytes[3..23]);
            if pubkey_hash != expected_hash {
                return Err("Pubkey hash mismatch for P2PKH".to_string());
            }
            let script_code = script_pubkey.clone();
            let sighash = tx.compute_sighash(input_index, &script_code, input_value, SIGHASH_ALL);
            if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                return Err("Signature verification failed for P2PKH".to_string());
            }
        } else if script_pubkey_bytes.len() == 23
            && script_pubkey_bytes[0] == 0xa9
            && script_pubkey_bytes[1] == 0x14
            && script_pubkey_bytes[22] == 0x87
        {
            println!("verify input here 3");
            // Legacy P2SH
            if stack.is_empty() {
                return Err("Empty stack for P2SH".to_string());
            }
            let redeem = stack.last().unwrap().clone();

            // Ownership and maturity check for timelock P2SH
            if stack.len() != 3 {
                // Fall back to general P2SH (e.g., multisig)
                let input_stack = &stack[0..stack.len() - 1];
                println!("verify input here 5");
                Self::verify_script(
                    tx,
                    input_index,
                    input,
                    input_value,
                    &redeem,
                    input_stack,
                    current_time,
                    false,
                    utxo,
                    treechain,
                )?;
                return Ok(());
            }
            let sig_hex = stack[0].clone();
            let pubkey_hex = stack[1].clone();
            let pubkey_hash = match ChainUtil::pubkey_hash_from_address(&pubkey_hex) {
                Ok(hash) => hash,
                Err(_) => {
                    println!("❌ Invalid pubkey_hex: {}", pubkey_hex);
                    return Err("Invalid pubkey_hex".to_string());
                }
            };
            let extracted_script_hash = hex::encode(&script_pubkey_bytes[2..22]);
            let current_time_check = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u64;
            // Find the originating block by queue_index
            if let Some((_, originating_block_opt)) = treechain
                .blocks
                .get_index(utxo.queue_index.try_into().unwrap())
            {
                let originating_block = originating_block_opt;
                // Find the originating transaction by txid
                let originating_tx_opt = originating_block.tx.iter().find(|t| t.txid == input.txid);
                if let Some(originating_tx) = originating_tx_opt {
                    // Verify the output at vout matches the script_pubkey
                    if let Some(output) = originating_tx.vout.get(input.vout as usize) {
                        if output.script_pubkey != *script_pubkey {
                            println!(
                                "⚠️ Script mismatch for UTXO {}:{} in block {}",
                                input.txid, input.vout, originating_block.hash
                            );
                            return Err("Script mismatch for P2SH".to_string());
                        }
                        let locktime = originating_tx.locktime;
                        let is_cltv = locktime != 0;
                        let mut lock: u32 = 0;
                        let mut is_csv_detected = false;
                        if is_cltv {
                            lock = locktime;
                        } else {
                            // For CSV, find max relative lock from inputs' sequences
                            let mut max_relative = 0u32;
                            for inp in &originating_tx.vin {
                                if inp.sequence != 4294967295u32 {
                                    is_csv_detected = true;
                                    let relative = inp.sequence & 0x0000FFFF;
                                    if relative > max_relative {
                                        max_relative = relative;
                                    }
                                }
                            }
                            if is_csv_detected {
                                lock = max_relative;
                            } else {
                                // Not a timelock P2SH, fall back
                                let input_stack = &stack[0..stack.len() - 1];
                                Self::verify_script(
                                    tx,
                                    input_index,
                                    input,
                                    input_value,
                                    &redeem,
                                    input_stack,
                                    current_time,
                                    false,
                                    utxo,
                                    treechain,
                                )?;
                                return Ok(());
                            }
                        }

                        // Construct p2pkh_script for this pubkey
                        let p2pkh_script = format!("76a914{}88ac", pubkey_hash);
                        // Lock as 4 LE bytes hex
                        let lock_bytes = lock.to_le_bytes();
                        let lock_hex = format!(
                            "{:02x}{:02x}{:02x}{:02x}",
                            lock_bytes[0], lock_bytes[1], lock_bytes[2], lock_bytes[3]
                        );
                        let opcode = if is_cltv { "b1" } else { "b2" };
                        let expected_redeem = format!("04{}{}75{}", lock_hex, opcode, p2pkh_script);
                        let expected_redeem_bytes = match hex::decode(&expected_redeem) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                println!(
                                    "⚠️ Failed to decode expected redeem script '{}': {}",
                                    expected_redeem, e
                                );
                                return Err("Invalid expected redeem script".to_string());
                            }
                        };
                        // Compute expected_hash = hash160(expected_redeem_bytes)
                        let expected_hash = ChainUtil::hash160(&expected_redeem_bytes);
                        if expected_hash != extracted_script_hash {
                            println!(
                                "⚠️ P2SH hash mismatch for UTXO {}:{} (expected: {}, got: {})",
                                input.txid, input.vout, expected_hash, extracted_script_hash
                            );
                            return Err("P2SH ownership mismatch".to_string());
                        }
                        // Check maturity (policy)
                        let lock_u64 = lock as u64;
                        let is_mature = if is_cltv {
                            if lock_u64 >= 500_000_000 {
                                current_time_check >= lock_u64
                            } else {
                                current_height >= lock_u64
                            }
                        } else {
                            let blocks_since =
                                current_height.saturating_sub(utxo.queue_index as u64);
                            blocks_since >= lock_u64
                        };
                        if !is_mature {
                            println!(
                                "🔒 Immature P2SH UTXO {}:{} (is_cltv={}, lock={}, blocks_since={})",
                                input.txid,
                                input.vout,
                                is_cltv,
                                lock,
                                current_height.saturating_sub(utxo.queue_index as u64)
                            );
                            return Err("P2SH timelock not mature".to_string());
                        }
                        // Verify signature directly (inner P2PKH)
                        let script_code = redeem.clone();
                        let sighash =
                            tx.compute_sighash(input_index, &script_code, input_value, SIGHASH_ALL);
                        if !Self::verify_signature(&sighash, &sig_hex, &pubkey_hex) {
                            return Err(
                                "Signature verification failed for P2SH timelock".to_string()
                            );
                        }
                        println!(
                            "✅ P2SH timelock verified for input {}:{}",
                            input.txid, input.vout
                        );
                        return Ok(());
                    } else {
                        println!(
                            "⚠️ Invalid vout {} for txid {} in block {}",
                            input.vout, input.txid, originating_block.hash
                        );
                        return Err("Invalid vout for P2SH".to_string());
                    }
                } else {
                    println!(
                        "⚠️ Originating tx {} not found in block for queue_index {}",
                        input.txid, utxo.queue_index
                    );
                    return Err("Originating tx not found".to_string());
                }
            } else {
                println!(
                    "⚠️ Originating block not found for queue_index {}",
                    utxo.queue_index
                );
                return Err("Originating block not found".to_string());
            }
        } else if script_pubkey_bytes.len() == 22
            && script_pubkey_bytes[0] == 0x00
            && script_pubkey_bytes[1] == 0x14
        {
            // P2WPKH
            if stack.len() != 2 {
                return Err("Invalid witness for P2WPKH".to_string());
            }
            let sig_hex = &stack[0];
            let pubkey_hex = &stack[1];
            let pubkey_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
            let expected_hash = hex::encode(&script_pubkey_bytes[2..22]);
            if pubkey_hash != expected_hash {
                return Err("Pubkey hash mismatch for P2WPKH".to_string());
            }
            let script_code = Transaction::create_p2pkh_script(&pubkey_hash);
            let sighash =
                tx.compute_segwit_sighash(input_index, input_value, &script_code, SIGHASH_ALL);
            if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                return Err("Signature verification failed for P2WPKH".to_string());
            }
        } else if script_pubkey_bytes.len() == 34
            && script_pubkey_bytes[0] == 0x00
            && script_pubkey_bytes[1] == 0x20
        {
            // P2WSH (similar to P2SH but with witness and SHA256)
            if stack.is_empty() {
                return Err("Empty witness for P2WSH".to_string());
            }
            let witness_script = stack.last().unwrap().clone();
            let witness_script_bytes = hex::decode(&witness_script)
                .map_err(|e| format!("Invalid witness_script hex: {}", e))?;
            let mut hasher = Sha256::new();
            hasher.update(&witness_script_bytes);
            let ws_hash = hex::encode(hasher.finalize());
            let expected_hash = hex::encode(&script_pubkey_bytes[2..34]);
            if ws_hash != expected_hash {
                return Err("Witness script hash mismatch for P2WSH".to_string());
            }

            // Ownership and maturity check for timelock P2WSH
            if stack.len() != 3 {
                // Fall back to general P2WSH (e.g., multisig)
                let input_stack = &stack[0..stack.len() - 1];
                Self::verify_script(
                    tx,
                    input_index,
                    input,
                    input_value,
                    &witness_script,
                    input_stack,
                    current_time,
                    true,
                    utxo,
                    treechain,
                )?;
                return Ok(());
            }
            let sig_hex = stack[0].clone();
            let pubkey_hex = stack[1].clone();
            let pubkey_hash = match ChainUtil::pubkey_hash_from_address(&pubkey_hex) {
                Ok(hash) => hash,
                Err(_) => {
                    println!("❌ Invalid pubkey_hex: {}", pubkey_hex);
                    return Err("Invalid pubkey_hex".to_string());
                }
            };
            let extracted_script_hash = hex::encode(&script_pubkey_bytes[2..34]);
            let current_time_check = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u64;
            // Find the originating block by queue_index
            if let Some((_, originating_block_opt)) = treechain
                .blocks
                .get_index(utxo.queue_index.try_into().unwrap())
            {
                let originating_block = originating_block_opt;
                // Find the originating transaction by txid
                let originating_tx_opt = originating_block.tx.iter().find(|t| t.txid == input.txid);
                if let Some(originating_tx) = originating_tx_opt {
                    // Verify the output at vout matches the script_pubkey
                    if let Some(output) = originating_tx.vout.get(input.vout as usize) {
                        if output.script_pubkey != *script_pubkey {
                            println!(
                                "⚠️ Script mismatch for UTXO {}:{} in block {}",
                                input.txid, input.vout, originating_block.hash
                            );
                            return Err("Script mismatch for P2WSH".to_string());
                        }
                        let locktime = originating_tx.locktime;
                        let is_cltv = locktime != 0;
                        let mut lock: u32 = 0;
                        let mut is_csv_detected = false;
                        if is_cltv {
                            lock = locktime;
                        } else {
                            // For CSV, find max relative lock from inputs' sequences
                            let mut max_relative = 0u32;
                            for inp in &originating_tx.vin {
                                if inp.sequence != 4294967295u32 {
                                    is_csv_detected = true;
                                    let relative = inp.sequence & 0x0000FFFF;
                                    if relative > max_relative {
                                        max_relative = relative;
                                    }
                                }
                            }
                            if is_csv_detected {
                                lock = max_relative;
                            } else {
                                // Not a timelock P2WSH, fall back
                                let input_stack = &stack[0..stack.len() - 1];
                                Self::verify_script(
                                    tx,
                                    input_index,
                                    input,
                                    input_value,
                                    &witness_script,
                                    input_stack,
                                    current_time,
                                    true,
                                    utxo,
                                    treechain,
                                )?;
                                return Ok(());
                            }
                        }

                        // Construct p2pkh_script for this pubkey
                        let p2pkh_script = format!("76a914{}88ac", pubkey_hash);
                        // Lock as 4 LE bytes hex
                        let lock_bytes = lock.to_le_bytes();
                        let lock_hex = format!(
                            "{:02x}{:02x}{:02x}{:02x}",
                            lock_bytes[0], lock_bytes[1], lock_bytes[2], lock_bytes[3]
                        );
                        let opcode = if is_cltv { "b1" } else { "b2" };
                        let expected_redeem = format!("04{}{}75{}", lock_hex, opcode, p2pkh_script);
                        let expected_redeem_bytes = match hex::decode(&expected_redeem) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                println!(
                                    "⚠️ Failed to decode expected redeem script '{}': {}",
                                    expected_redeem, e
                                );
                                return Err("Invalid expected redeem script".to_string());
                            }
                        };
                        // Compute expected_hash = SHA256(expected_redeem_bytes)
                        let mut hasher = Sha256::new();
                        hasher.update(&expected_redeem_bytes);
                        let expected_hash = hex::encode(hasher.finalize());
                        if expected_hash != extracted_script_hash {
                            println!(
                                "⚠️ P2WSH hash mismatch for UTXO {}:{} (expected: {}, got: {})",
                                input.txid, input.vout, expected_hash, extracted_script_hash
                            );
                            return Err("P2WSH ownership mismatch".to_string());
                        }
                        // Check maturity (policy)
                        let lock_u64 = lock as u64;
                        let is_mature = if is_cltv {
                            if lock_u64 >= 500_000_000 {
                                current_time_check >= lock_u64
                            } else {
                                current_height >= lock_u64
                            }
                        } else {
                            let blocks_since =
                                current_height.saturating_sub(utxo.queue_index as u64);
                            blocks_since >= lock_u64
                        };
                        if !is_mature {
                            println!(
                                "🔒 Immature P2WSH UTXO {}:{} (is_cltv={}, lock={}, blocks_since={})",
                                input.txid,
                                input.vout,
                                is_cltv,
                                lock,
                                current_height.saturating_sub(utxo.queue_index as u64)
                            );
                            return Err("P2WSH timelock not mature".to_string());
                        }
                        // Verify signature directly (inner P2PKH, SegWit sighash)
                        let script_code = witness_script.clone();
                        let sighash = tx.compute_segwit_sighash(
                            input_index,
                            input_value,
                            &script_code,
                            SIGHASH_ALL,
                        );
                        if !Self::verify_signature(&sighash, &sig_hex, &pubkey_hex) {
                            return Err(
                                "Signature verification failed for P2WSH timelock".to_string()
                            );
                        }
                        println!(
                            "✅ P2WSH timelock verified for input {}:{}",
                            input.txid, input.vout
                        );
                        return Ok(());
                    } else {
                        println!(
                            "⚠️ Invalid vout {} for txid {} in block {}",
                            input.vout, input.txid, originating_block.hash
                        );
                        return Err("Invalid vout for P2WSH".to_string());
                    }
                } else {
                    println!(
                        "⚠️ Originating tx {} not found in block for queue_index {}",
                        input.txid, utxo.queue_index
                    );
                    return Err("Originating tx not found".to_string());
                }
            } else {
                println!(
                    "⚠️ Originating block not found for queue_index {}",
                    utxo.queue_index
                );
                return Err("Originating block not found".to_string());
            }
        } else {
            return Err("Unsupported script type".to_string());
        }

        Ok(())
    }

    fn verify_script(
        tx: &Transaction,
        input_index: usize,
        input: &TxInput,
        input_value: u64,
        script_hex: &str,
        stack: &[String],
        current_time: u32,
        is_segwit: bool,
        utxo: &Utxo,
        treechain: &TreeChain,
    ) -> Result<(), String> {
        println!("called verify script");
        let script_bytes =
            hex::decode(script_hex).map_err(|e| format!("Invalid script hex: {}", e))?;
        println!("verify script here 0.1");
        let ops = Self::parse_script_ops(&script_bytes);
        println!("verify script here 0.2");
        let script_info = Self::get_script_info(&ops)?;
        println!("verify script here 0.3");

        let sighash = if is_segwit {
            tx.compute_segwit_sighash(input_index, input_value, script_hex, SIGHASH_ALL)
        } else {
            tx.compute_sighash(input_index, script_hex, input_value, SIGHASH_ALL)
        };

        match script_info {
            ScriptInfo::P2pkh { pubkey_hash } => {
                if stack.len() != 2 {
                    return Err("Invalid stack for P2PKH script".to_string());
                }
                let sig_hex = &stack[0];
                let pubkey_hex = &stack[1];
                let computed_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
                if computed_hash != pubkey_hash {
                    return Err("Pubkey hash mismatch in P2PKH script".to_string());
                }
                if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                    return Err("Signature verification failed in P2PKH script".to_string());
                }
            }
            ScriptInfo::Multisig { m, pubkeys, n: _ } => {
                println!("verify script called here 1");
                if stack.len() < (m as usize + 1) || !stack[0].is_empty() {
                    println!("stack: {:?}", stack);
                    // Expect empty for OP_0
                    return Err("Invalid stack for multisig".to_string());
                }
                let sigs = &stack[1..];
                if sigs.len() < m as usize {
                    return Err("Incorrect number of signatures for multisig".to_string());
                }
                let mut valid_sigs = 0;
                for sig in &sigs[0..m as usize] {
                    let mut matched = false;
                    for pubk in &pubkeys {
                        println!("verify script called here 2");
                        if Self::verify_signature(&sighash, sig, pubk) {
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        valid_sigs += 1;
                    }
                }
                if valid_sigs < m as usize {
                    return Err("Insufficient valid signatures for multisig".to_string());
                }
            }
            ScriptInfo::Cltv { lock, pubkey_hash } => {
                if tx.version < 2 {
                    return Err("Transaction version must be at least 2 for CLTV".to_string());
                }
                if tx.locktime < lock as u32 {
                    return Err(format!(
                        "CLTV locktime {} < required lock {}",
                        tx.locktime, lock
                    ));
                }
                if input.sequence >= 0xFFFFFFFEu32 {
                    return Err(
                        "Input sequence must be less than 0xFFFFFFFE to enable CLTV enforcement"
                            .to_string(),
                    );
                }
                if stack.len() != 2 {
                    return Err("Invalid stack for CLTV".to_string());
                }
                let sig_hex = &stack[0];
                let pubkey_hex = &stack[1];
                let computed_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
                if computed_hash != pubkey_hash {
                    return Err("Pubkey hash mismatch in CLTV".to_string());
                }
                if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                    return Err("Signature verification failed in CLTV".to_string());
                }
            }
            ScriptInfo::Csv { lock, pubkey_hash } => {
                if tx.version < 2 {
                    return Err("Transaction version must be at least 2 for CSV".to_string());
                }
                let sequence_relative = (input.sequence & 0x0000FFFFu32) as u64;
                let sequence_is_time = (input.sequence & 0x00010000) != 0;
                let lock_is_time = lock >= 500_000_000u32;
                if sequence_is_time != lock_is_time {
                    return Err("CSV locktime type mismatch (time vs block)".to_string());
                }
                if sequence_relative < lock as u64 {
                    return Err(format!(
                        "CSV sequence relative lock {} < required {}",
                        sequence_relative, lock
                    ));
                }
                if stack.len() != 2 {
                    return Err("Invalid stack for CSV".to_string());
                }
                let sig_hex = &stack[0];
                let pubkey_hex = &stack[1];
                let computed_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
                if computed_hash != pubkey_hash {
                    return Err("Pubkey hash mismatch in CSV".to_string());
                }
                if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                    return Err("Signature verification failed in CSV".to_string());
                }
            }
        }
        Ok(())
    }
    fn parse_stack(bytes: &[u8]) -> Result<Vec<String>, String> {
        let mut items = vec![];
        let mut pos = 0;
        while pos < bytes.len() {
            let len = bytes[pos] as usize;
            pos += 1;
            if pos + len > bytes.len() {
                return Err("Invalid stack length".to_string());
            }
            let item = if len == 0 {
                "".to_string()
            } else {
                hex::encode(&bytes[pos..pos + len])
            };
            items.push(item);
            pos += len;
        }
        Ok(items)
    }

    pub fn parse_script_ops(bytes: &[u8]) -> Vec<Op> {
        println!("called parse script ops");
        let mut ops = vec![];
        let mut pos = 0;
        while pos < bytes.len() {
            let op = bytes[pos];
            pos += 1;
            if op == 0 {
                ops.push(Op::Push(vec![]));
            } else if (1..=75).contains(&op) {
                if pos + op as usize > bytes.len() {
                    break;
                }
                ops.push(Op::Push(bytes[pos..pos + op as usize].to_vec()));
                pos += op as usize;
            } else {
                ops.push(Op::Code(op));
            }
        }
        ops
    }
    pub fn get_script_info(ops: &[Op]) -> Result<ScriptInfo, String> {
        println!("ops at get_script_info: {:?}", ops);
        println!("called get script info");
        // P2PKH: OP_DUP OP_HASH160 <20-byte hash> OP_EQUALVERIFY OP_CHECKSIG
        if ops.len() == 5
        && matches!(ops[0], Op::Code(0x76))
        && matches!(ops[1], Op::Code(0xa6))  // Fixed: OP_HASH160 (0xa6), not OP_EQUAL (0xa9)
        && matches!(ops[2], Op::Push(ref data) if data.len() == 20)
        && matches!(ops[3], Op::Code(0x88))
        && matches!(ops[4], Op::Code(0xac))
        {
            return Ok(ScriptInfo::P2pkh {
                pubkey_hash: hex::encode(&ops[2].as_push().unwrap()),
            });
        }
        // Multisig: <m> <pubkey>* <n> OP_CHECKMULTISIG (unchanged)
        if ops.len() >= 4 {
            let m_op = &ops[0];
            let n_op = &ops[ops.len() - 2];
            let checkmultisig_op = &ops[ops.len() - 1];
            if let (Op::Code(m), Op::Code(n), Op::Code(0xae)) = (m_op, n_op, checkmultisig_op) {
                if *m >= 0x51 && *m <= 0x60 && *n >= 0x51 && *n <= 0x60 && *n >= *m {
                    let m = m - 0x50;
                    let n = n - 0x50;
                    let pubkeys: Vec<String> = ops[1..ops.len() - 2]
                        .iter()
                        .map(|op| {
                            if let Op::Push(data) = op {
                                hex::encode(data)
                            } else {
                                "".to_string()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                    if pubkeys.len() == n as usize {
                        return Ok(ScriptInfo::Multisig { m, pubkeys, n });
                    }
                }
            }
        }
        // CLTV: <lock-push> OP_CLTV OP_DROP OP_DUP OP_HASH160 <20-byte hash> OP_EQUALVERIFY OP_CHECKSIG
        if ops.len() == 8
        && matches!(ops[0], Op::Push(ref data) if data.len() <= 4)
        && matches!(ops[1], Op::Code(0xb1))
        && matches!(ops[2], Op::Code(0x75))  // Fixed: OP_DROP
        && matches!(ops[3], Op::Code(0x76))  // OP_DUP
        && matches!(ops[4], Op::Code(0xa6))  // Fixed: OP_HASH160
        && matches!(ops[5], Op::Push(ref data) if data.len() == 20)
        && matches!(ops[6], Op::Code(0x88))
        && matches!(ops[7], Op::Code(0xac))
        {
            let lock_bytes = ops[0].as_push().unwrap();
            let mut bytes_arr = [0u8; 4];
            let copy_len = lock_bytes.len().min(4);
            bytes_arr[0..copy_len].copy_from_slice(&lock_bytes[0..copy_len]);
            let lock = u32::from_le_bytes(bytes_arr);
            return Ok(ScriptInfo::Cltv {
                lock,
                pubkey_hash: hex::encode(&ops[5].as_push().unwrap()), // Fixed index
            });
        }
        // CSV: <lock-push> OP_CSV OP_DROP OP_DUP OP_HASH160 <20-byte hash> OP_EQUALVERIFY OP_CHECKSIG
        if ops.len() == 8
        && matches!(ops[0], Op::Push(ref data) if data.len() <= 4)
        && matches!(ops[1], Op::Code(0xb2))
        && matches!(ops[2], Op::Code(0x75))  // Fixed: OP_DROP
        && matches!(ops[3], Op::Code(0x76))  // OP_DUP
        && matches!(ops[4], Op::Code(0xa6))  // Fixed: OP_HASH160
        && matches!(ops[5], Op::Push(ref data) if data.len() == 20)
        && matches!(ops[6], Op::Code(0x88))
        && matches!(ops[7], Op::Code(0xac))
        {
            let lock_bytes = ops[0].as_push().unwrap();
            let mut bytes_arr = [0u8; 4];
            let copy_len = lock_bytes.len().min(4);
            bytes_arr[0..copy_len].copy_from_slice(&lock_bytes[0..copy_len]);
            let lock = u32::from_le_bytes(bytes_arr);
            return Ok(ScriptInfo::Csv {
                lock,
                pubkey_hash: hex::encode(&ops[5].as_push().unwrap()), // Fixed index
            });
        }
        Err("Unsupported script info".to_string())
    }
    fn bytes_to_u32(bytes: &[u8]) -> u32 {
        let mut arr = [0u8; 4];
        let len = bytes.len().min(4);
        arr[0..len].copy_from_slice(&bytes[0..len]);
        u32::from_le_bytes(arr)
    }

    // pub fn add_transaction(&mut self, tx: Transaction, utxo_set: &UtxoSet) -> Result<(), String> {
    //     println!("Adding transaction to mempool: {}", tx.txid);
    //     // Validate transaction
    //     let (fee, vsize, fee_rate, depends) =
    //         self.validate_transaction(&tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)?;

    //     // Check mempool size limit
    //     if self.total_size + vsize > MAX_MEMPOOL_SIZE {
    //         self.evict_low_fee_transactions(vsize)?;
    //     }

    //     // Check dependencies
    //     for parent_txid in &depends {
    //         if let Some(parent) = self.pool.get_mut(parent_txid) {
    //             parent.children.insert(tx.txid.clone());
    //         } else {
    //             return Err("Parent not in pool".to_string());
    //         }
    //     }

    //     // Remove spent mempool UTXOs
    //     for input in &tx.vin {
    //         if depends.contains(&input.txid) {
    //             self.utxo_set.remove_utxo(&input.txid, input.vout);
    //         }
    //     }

    //     // Add new UTXOs
    //     for (i, _) in tx.vout.iter().enumerate() {
    //         let utxo = Utxo::extract_utxo(&tx, i as u32, 0).unwrap();
    //         self.utxo_set.add_utxo(tx.txid.clone(), i as u32, utxo);
    //     }
    //     // Add to mempool
    //     let txid = tx.txid.clone();
    //     let entry = MempoolEntry {
    //         tx: tx.clone(),
    //         fee,
    //         vsize,
    //         fee_rate,
    //         added_time: SystemTime::now()
    //             .duration_since(UNIX_EPOCH)
    //             .unwrap_or_default()
    //             .as_millis() as u64,
    //         depends,
    //         children: HashSet::new(),
    //     };
    //     self.pool.insert(txid.clone(), entry.clone());
    //     self.total_size += vsize;

    //     Ok(())
    // }

    pub fn add_transaction(
        &mut self,
        tx: Transaction,
        utxo_set: &UtxoSet,
        treechain: &TreeChain,
    ) -> Result<(), String> {
        println!("add transaction called");

        let (fee, vsize, fee_rate, depends) =
            self.validate_transaction(&tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set, treechain)?;
        println!("validated txn");

        let current_height = treechain.get_max_queue_index(); // Assume this method exists
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        // Check if this is a normal/CSV txn (locktime=0, all sequences=max) or CLTV funding (locktime>0)
        let is_normal_or_csv =
            tx.locktime == 0 && tx.vin.iter().all(|input| input.sequence == 0xffffffff);
        if !is_normal_or_csv {
            // Assume CLTV funding: enforce maturity on txn locktime
            let is_time_based = tx.locktime >= 500_000_000;
            let current = if is_time_based {
                current_time
            } else {
                current_height as u32
            };
            if current < tx.locktime {
                return Err(format!(
                    "⛔ CLTV funding txn not mature: locktime {} > current {}",
                    tx.locktime, current
                ));
            }
            println!(
                "✅ CLTV funding txn mature: locktime {} <= current {}",
                tx.locktime, current
            );
        } else {
            println!("✅ Normal/CSV txn: no maturity check needed");
        }

        let entry_size = tx.serialize_non_witness().len();
        if self.total_size + entry_size > MAX_MEMPOOL_SIZE {
            return Err("Mempool full".to_string());
        }

        // Remove spent UTXOs from mempool UTXO set
        for input in &tx.vin {
            self.utxo_set.remove_utxo(&input.txid, input.vout);
        }

        // Add new outputs to mempool UTXO set
        for (i, out) in tx.vout.iter().enumerate() {
            self.utxo_set
                .add_utxo(tx.txid.clone(), i as u32, Utxo::new(out.clone(), 0, false));
        }

        // Create entry
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

        // Update parents' children
        for parent_txid in &entry.depends {
            if let Some(parent) = self.pool.get_mut(parent_txid) {
                parent.children.insert(tx.txid.clone());
            }
        }

        self.pool.insert(tx.txid.clone(), entry);
        self.total_size += entry_size;

        Ok(())
    }

    pub fn remove_transaction(&mut self, txid: &str) {
        if let Some(entry) = self.pool.remove(txid) {
            let tx_size = entry.tx.serialize_non_witness().len();
            self.total_size = self.total_size.saturating_sub(tx_size);

            // Remove its outputs from mempool UTXO set
            for i in 0..entry.tx.vout.len() as u32 {
                self.utxo_set.remove_utxo(txid, i);
            }

            // Restore spent UTXOs if they were from mempool
            for input in &entry.tx.vin {
                if let Some(parent_entry) = self.pool.get(&input.txid) {
                    let utxo =
                        Utxo::new(parent_entry.tx.vout[input.vout as usize].clone(), 0, false);
                    self.utxo_set.add_utxo(input.txid.clone(), input.vout, utxo);
                }
            }

            // Update parents' children
            for parent_txid in &entry.depends {
                if let Some(parent) = self.pool.get_mut(parent_txid) {
                    parent.children.remove(txid);
                }
            }

            // Recursively remove children if they become orphaned
            let children: Vec<String> = entry.children.into_iter().collect();
            for child_txid in children {
                self.remove_transaction(&child_txid);
            }
        }
    }

    pub fn replace_by_fee(
        &mut self,
        new_tx: Transaction,
        utxo_set: &UtxoSet,
        treechain: &TreeChain,
    ) -> Result<(), String> {
        // Validate new transaction first
        let (new_fee, _, new_fee_rate, _) =
            self.validate_transaction(&new_tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set, treechain)?;

        // Find conflicting transactions (same inputs)
        let mut replaced_txids = HashSet::new();
        let mut total_replaced_fee = 0;
        for input in &new_tx.vin {
            let spending_txs = self.get_transactions_spending_utxo(&input.txid, input.vout);
            for tx in spending_txs {
                let entry = self.pool.get(&tx.txid).unwrap();
                replaced_txids.insert(tx.txid.clone());
                total_replaced_fee += entry.fee;
            }
        }

        if !replaced_txids.is_empty() {
            // Check RBF rules (simplified)
            if new_fee_rate
                <= self
                    .pool
                    .values()
                    .map(|e| e.fee_rate)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0)
            {
                return Err(format!(
                    "New transaction fee rate {} not higher than minimum in pool",
                    new_fee_rate
                ));
            }
            for txid in &replaced_txids {
                let entry = self.pool.get(txid).unwrap();
                if new_fee_rate <= entry.fee_rate {
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
        self.add_transaction(new_tx, utxo_set, treechain)
    }

    // pub fn remove_transaction(&mut self, txid: &str) {
    //     if let Some(entry) = self.pool.remove(txid) {
    //         self.total_size -= entry.vsize;

    //         // Recursively remove children first
    //         let children = entry.children.clone();
    //         for child in children {
    //             self.remove_transaction(&child);
    //         }

    //         // Remove its UTXOs
    //         for i in 0..entry.tx.vout.len() as u32 {
    //             self.utxo_set.remove_utxo(&entry.tx.txid, i);
    //         }

    //         // Restore spent mempool UTXOs
    //         for input in &entry.tx.vin {
    //             if entry.depends.contains(&input.txid) {
    //                 if let Some(parent) = self.pool.get(&input.txid) {
    //                     let utxo = Utxo::extract_utxo(&parent.tx, input.vout, 0).unwrap();
    //                     self.utxo_set.add_utxo(input.txid.clone(), input.vout, utxo);
    //                 }
    //             }
    //         }

    //         // Remove from parents' children
    //         for parent_txid in entry.depends {
    //             if let Some(parent) = self.pool.get_mut(&parent_txid) {
    //                 parent.children.remove(txid);
    //             }
    //         }
    //     }
    // }

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
        treechain: &TreeChain,
    ) -> Result<(), String> {
        println!("replace_transaction called for txid: {}", new_tx.txid);
        // Validate new transaction
        let (new_fee, new_vsize, new_fee_rate, new_depends) = self
            .validate_transaction(&new_tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set, treechain)
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
        self.add_transaction(new_tx, utxo_set, treechain)
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

    pub fn reorg(&mut self, utxo_set: &UtxoSet, treechain: &TreeChain) {
        let mut invalid_txids = Vec::new();
        for (txid, entry) in &self.pool {
            if let Err(_) = self.validate_transaction(
                &entry.tx,
                DEFAULT_MIN_FEE_RATE as f64,
                utxo_set,
                treechain,
            ) {
                invalid_txids.push(txid.clone());
            }
        }
        for txid in invalid_txids {
            self.remove_transaction(&txid);
        }
    }
}
