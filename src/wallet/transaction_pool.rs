use crate::chain_util::ChainUtil;
use crate::config::{CHILDREN, SIGHASH_ALL, USER_TXN_FREERATE};
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
enum Op {
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
enum ScriptInfo {
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

    //     // Verify inputs are unspent and calculate total input value
    //     let mut input_value = 0;
    //     let mut depends = HashSet::new();
    //     for input in &tx.vin {
    //         let utxo = utxt_set
    //             .get_utxo(&input.txid, input.vout)
    //             .or_else(|| self.utxo_set.get_utxo(&input.txid, input.vout))
    //             .ok_or(format!("UTXO not found: {}:{}", input.txid, input.vout))?;
    //         input_value += utxo.out.value;

    //         if self.pool.contains_key(&input.txid) {
    //             depends.insert(input.txid.clone());
    //         }

    //         let input_index = tx.vin.iter().position(|x| x == input).unwrap();
    //         if tx.witnesses.is_none() {
    //             let sighash = tx.compute_sighash(
    //                 input_index,
    //                 &utxo.out.script_pubkey,
    //                 utxo.out.value,
    //                 SIGHASH_ALL,
    //             );
    //             let (sig_hex, pubkey_hex) =
    //                 Self::parse_script_sig(&input.script_sig).map_err(|e| {
    //                     format!("Script_sig error for {}:{}: {}", input.txid, input.vout, e)
    //                 })?;
    //             if !Self::verify_signature(&sighash, &sig_hex, &pubkey_hex) {
    //                 return Err(format!(
    //                     "Invalid signature for input {}:{}",
    //                     input.txid, input.vout
    //                 ));
    //             }
    //         } else {
    //             let witnesses = tx.witnesses.as_ref().unwrap();
    //             let witness = witnesses.get(input_index).ok_or(format!(
    //                 "Missing witness data for {}:{}",
    //                 input.txid, input.vout
    //             ))?;
    //             if witness.len() < 2 {
    //                 return Err(format!(
    //                     "Invalid witness format for {}:{}",
    //                     input.txid, input.vout
    //                 ));
    //             }
    //             let sighash = tx.compute_segwit_sighash(
    //                 input_index,
    //                 utxo.out.value,
    //                 &utxo.out.script_pubkey,
    //                 SIGHASH_ALL,
    //             );
    //             if !Self::verify_signature(&sighash, &witness[0], &witness[1]) {
    //                 return Err(format!(
    //                     "Invalid witness signature for input {}:{}",
    //                     input.txid, input.vout
    //                 ));
    //             }
    //         }
    //     }

    //     let output_value: u64 = tx.vout.iter().map(|out| out.value).sum();
    //     if output_value > input_value {
    //         return Err(format!(
    //             "Output value {} exceeds input value {}",
    //             output_value, input_value
    //         ));
    //     }
    //     let fee = input_value - output_value;

    //     let fee_rate = fee as f64 / vsize as f64;
    //     if fee_rate < min_fee_rate {
    //         return Err(format!(
    //             "Fee rate {} sat/vB below minimum {}",
    //             fee_rate, min_fee_rate
    //         ));
    //     }
    //     // println!("self.pool values: {:?}", self.pool.values());
    //     for input in &tx.vin {
    //         if self.pool.values().any(|entry| {
    //             entry
    //                 .tx
    //                 .vin
    //                 .iter()
    //                 .any(|in_| in_.txid == input.txid && in_.vout == input.vout)
    //         }) {
    //             return Err(format!(
    //                 "Double-spend detected for {}:{}",
    //                 input.txid, input.vout
    //             ));
    //         }
    //     }

    //     Ok((fee, vsize, fee_rate, depends))
    // }

    pub fn validate_transaction(
        &self,
        tx: &Transaction,
        min_fee_rate: f64,
        utxt_set: &UtxoSet,
    ) -> Result<(u64, usize, f64, HashSet<String>), String> {
        //check txid and hash
        if !Transaction::check_txid_and_hash(tx) {
            return Err(format!(
                "Transaction txid and hash dosnt match calculation: {}",
                tx.txid
            ));
        }

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
        println!("validate transaction here 1");
        // Verify inputs are unspent and calculate total input value
        let mut input_value = 0;
        let mut depends = HashSet::new();
        let mut utxos = vec![];
        for input in &tx.vin {
            let utxo = utxt_set
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

        // Verify signatures and scripts for all inputs
        let witnesses = tx.witnesses.as_ref();
        for (i, input) in tx.vin.iter().enumerate() {
            let utxo = &utxos[i];
            println!("validate transaction here 3");
            Self::verify_input(
                tx,
                i,
                input,
                utxo,
                utxo.out.value,
                current_time,
                witnesses.and_then(|w| w.get(i)),
            )?;
        }

        // Check outputs
        let mut output_value = 0;
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
    ) -> Result<(), String> {
        println!("called verify input");
        let script_pubkey = &utxo.out.script_pubkey;
        let script_pubkey_bytes =
            hex::decode(script_pubkey).map_err(|e| format!("Invalid script_pubkey hex: {}", e))?;
        println!("verify input here 1");
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
            let redeem_bytes =
                hex::decode(&redeem).map_err(|e| format!("Invalid redeem hex: {}", e))?;
            let mut hasher = Sha256::new();
            hasher.update(&redeem_bytes);
            let hash1 = hasher.finalize();
            let ripemd_hash = Ripemd160::digest(hash1);
            let expected_hash = hex::encode(&script_pubkey_bytes[2..22]);
            // let redeem_bytes = hex::decode(redeem).expect("Invalid redeem script hex");
            // let script_hash = ChainUtil::hash160(&redeem_bytes);
            if hex::encode(ripemd_hash) != expected_hash {
                println!("verify input here 4");
                return Err("Redeem script hash mismatch for P2SH".to_string());
            }
            let script_code = redeem.clone();
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
            )?;
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
            // P2WSH
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
            let script_code = witness_script.clone();
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
            )?;
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
    ) -> Result<(), String> {
        println!("called verify script");
        let script_bytes =
            hex::decode(script_hex).map_err(|e| format!("Invalid script hex: {}", e))?;
        println!("verify script here 0.1");
        let ops = Self::parse_script_ops(&script_bytes);
        println!("verify script here 0.2");
        // let err = Self::get_script_info(&ops).unwrap_err();
        // println!("Error: {}", err);
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
            ScriptInfo::Multisig { m, n, pubkeys } => {
                println!("verify script called here 1");
                if stack.len() != (m as usize + 1) || !stack[0].is_empty() {
                    // Expect empty for OP_0
                    return Err("Invalid stack for multisig".to_string());
                }
                let sigs = &stack[1..];
                if sigs.len() < m as usize {
                    return Err("Incorrect number of signatures for multisig".to_string());
                }
                let mut valid_sigs = 0;
                for sig in sigs {
                    for pubk in &pubkeys {
                        println!("verify script called here 2");
                        if Self::verify_signature(&sighash, sig, pubk) {
                            valid_sigs += 1;
                        }
                    }
                }
                if valid_sigs < m as usize {
                    return Err("Insufficient valid signatures for multisig".to_string());
                }
            }
            ScriptInfo::Cltv { lock, pubkey_hash } => {
                if stack.len() != 2 {
                    return Err("Invalid stack for CLTV".to_string());
                }
                let sig_hex = &stack[0];
                let pubkey_hex = &stack[1];
                let computed_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
                if computed_hash != pubkey_hash {
                    return Err("Pubkey hash mismatch in CLTV".to_string());
                }
                if tx.locktime < lock {
                    return Err(format!(
                        "CLTV lock not satisfied: {} < {}",
                        tx.locktime, lock
                    ));
                }
                if !Self::verify_signature(&sighash, sig_hex, pubkey_hex) {
                    return Err("Signature verification failed in CLTV".to_string());
                }
            }
            ScriptInfo::Csv { lock, pubkey_hash } => {
                if stack.len() != 2 {
                    return Err("Invalid stack for CSV".to_string());
                }
                let sig_hex = &stack[0];
                let pubkey_hex = &stack[1];
                let computed_hash = ChainUtil::pubkey_hash_from_pubkey(pubkey_hex);
                if computed_hash != pubkey_hash {
                    return Err("Pubkey hash mismatch in CSV".to_string());
                }
                if input.sequence < lock {
                    return Err(format!(
                        "CSV lock not satisfied: {} < {}",
                        input.sequence, lock
                    ));
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

    fn parse_script_ops(bytes: &[u8]) -> Vec<Op> {
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

    fn get_script_info(ops: &[Op]) -> Result<ScriptInfo, String> {
        if ops.len() == 5
            && matches!(ops[0], Op::Code(0x76))
            && matches!(ops[1], Op::Code(0xa9))
            && matches!(ops[2], Op::Push(ref data) if data.len() == 20)
            && matches!(ops[3], Op::Code(0x88))
            && matches!(ops[4], Op::Code(0xac))
        {
            return Ok(ScriptInfo::P2pkh {
                pubkey_hash: hex::encode(&ops[2].as_push().unwrap()),
            });
        }
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
        if ops.len() == 6
            && matches!(ops[0], Op::Push(_))
            && matches!(ops[1], Op::Code(0xb1))
            && matches!(ops[2], Op::Code(0x76))
            && matches!(ops[3], Op::Code(0xa9))
            && matches!(ops[4], Op::Push(ref data) if data.len() == 20)
            && matches!(ops[5], Op::Code(0x88))
        {
            let lock_bytes = ops[0].as_push().unwrap();
            let lock = if lock_bytes.len() <= 4 {
                let mut bytes = [0u8; 4];
                bytes[..lock_bytes.len()].copy_from_slice(lock_bytes);
                u32::from_le_bytes(bytes)
            } else {
                return Err("Invalid CLTV locktime length".to_string());
            };
            return Ok(ScriptInfo::Cltv {
                lock,
                pubkey_hash: hex::encode(&ops[4].as_push().unwrap()),
            });
        }
        if ops.len() == 6
            && matches!(ops[0], Op::Push(_))
            && matches!(ops[1], Op::Code(0xb2))
            && matches!(ops[2], Op::Code(0x76))
            && matches!(ops[3], Op::Code(0xa9))
            && matches!(ops[4], Op::Push(ref data) if data.len() == 20)
            && matches!(ops[5], Op::Code(0x88))
        {
            let lock_bytes = ops[0].as_push().unwrap();
            let lock = if lock_bytes.len() <= 4 {
                let mut bytes = [0u8; 4];
                bytes[..lock_bytes.len()].copy_from_slice(lock_bytes);
                u32::from_le_bytes(bytes)
            } else {
                return Err("Invalid CSV locktime length".to_string());
            };
            return Ok(ScriptInfo::Csv {
                lock,
                pubkey_hash: hex::encode(&ops[4].as_push().unwrap()),
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

    pub fn add_transaction(&mut self, tx: Transaction, utxo_set: &UtxoSet) -> Result<(), String> {
        println!("add transaction called");
        let (fee, vsize, fee_rate, depends) =
            self.validate_transaction(&tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)?;
        println!("validated txn");

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
    ) -> Result<(), String> {
        // Validate new transaction first
        let (new_fee, _, new_fee_rate, _) =
            self.validate_transaction(&new_tx, DEFAULT_MIN_FEE_RATE as f64, utxo_set)?;

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
        self.add_transaction(new_tx, utxo_set)
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
