use crate::chain_util::ChainUtil;
use crate::config::SIGHASH_ALL;
use crate::treechain::treechain::TreeChain;
use crate::wallet::transaction::Transaction;
use crate::wallet::utxo::Utxo;
use crate::wallet::utxo::UtxoSet;
use k256::EncodedPoint;
use k256::ecdsa::Signature;
use k256::ecdsa::signature::Signer;
use k256::ecdsa::signature::Verifier;
use k256::ecdsa::{SigningKey, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint;

#[derive(Debug, PartialEq, Clone)]
pub struct Wallet {
    pub key_pair: SigningKey,
    pub public_key: String,
    pub public_key_hash: String,
    pub address: String,
}

impl Wallet {
    pub fn new() -> Self {
        let (key_pair, verifying_key) = ChainUtil::gen_key_pair();
        let public_key_point: EncodedPoint = verifying_key.to_encoded_point(true);
        let public_key_hex = hex::encode(public_key_point.as_bytes());
        let public_key_hash = ChainUtil::pubkey_hash_from_pubkey(&public_key_hex);
        let address = ChainUtil::address_from_pubkey_hash(&public_key_hash)
            .expect("failed to get the addresss");

        Self {
            key_pair,
            public_key: public_key_hex,
            public_key_hash: public_key_hash,
            address: address,
        }
    }

    pub fn sign_data(&self, data: &[u8]) -> String {
        let signature: Signature = self.key_pair.sign(data);
        hex::encode(signature.to_der().to_bytes())
    }

    pub fn verify_data_signature(data: &[u8], signature_hex: &str, pubkey_hex: &str) -> bool {
        // Decode the hex-encoded public key
        let pubkey_bytes = match hex::decode(pubkey_hex) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let encoded_point = match EncodedPoint::from_bytes(&pubkey_bytes) {
            Ok(point) => point,
            Err(_) => return false,
        };

        let verifying_key = match VerifyingKey::from_encoded_point(&encoded_point) {
            Ok(key) => key,
            Err(_) => return false,
        };

        // Decode signature
        let signature_bytes = match hex::decode(signature_hex) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let signature = match Signature::from_der(&signature_bytes) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        // Verify
        verifying_key.verify(data, &signature).is_ok()
    }

    pub fn get_balance(address: &str, utxo_set: &UtxoSet) -> u64 {
        // Get the public key hash from the address
        let pubkey_hash = match ChainUtil::pubkey_hash_from_address(address) {
            Ok(hash) => hash,
            Err(_) => {
                println!("❌ Invalid address: {}", address);
                return 0;
            }
        };

        // Calculate the expected P2PKH script_pubkey for this address
        let expected_script = Transaction::create_p2pkh_script(&pubkey_hash);

        // Sum the value of all UTXOs with matching script_pubkey
        let balance = utxo_set
            .utxos
            .values()
            .filter(|utxo| utxo.out.script_pubkey == expected_script)
            .map(|utxo| utxo.out.value)
            .sum::<u64>();

        println!(
            "✅ Balance for address {} (pkhash: {}): {} satoshis",
            address, pubkey_hash, balance
        );

        balance
    }

    // pub fn get_balance(
    //     &self,
    //     address: &str,
    //     utxo_set: &UtxoSet,
    //     treechain: &TreeChain,
    // ) -> (u64, u64, u64) {
    //     let pubkey_hash = match ChainUtil::pubkey_hash_from_address(address) {
    //         Ok(hash) => hash,
    //         Err(_) => {
    //             println!("❌ Invalid address: {}", address);
    //             return (0, 0, 0);
    //         }
    //     };

    //     let expected_p2pkh_script = Transaction::create_p2pkh_script(&pubkey_hash);

    //     let mut total_balance = 0u64;
    //     let mut unlocked_balance = 0u64;
    //     let mut locked_balance = 0u64;

    //     // Compute current height as max queue_index
    //     let current_height = treechain
    //         .blocks
    //         .values()
    //         .map(|b| b.pqp_entry.queue_index as u64)
    //         .max()
    //         .unwrap_or(0);

    //     // Compute current time as unix seconds
    //     let current_time = std::time::SystemTime::now()
    //         .duration_since(std::time::UNIX_EPOCH)
    //         .unwrap_or_default()
    //         .as_secs() as u64;
    //     for ((txid, vout), utxo) in &utxo_set.utxos {
    //         let script = &utxo.out.script_pubkey;
    //         if *script == expected_p2pkh_script {
    //             unlocked_balance += utxo.out.value;
    //             continue;
    //         } else if script.starts_with("a914") && script.ends_with("87") {
    //             println!("🧐 utxo script: {}", script);
    //             // P2SH: Extract script_hash (hex positions 4 to 44)
    //             let extracted_script_hash = &script[4..44];
    //             // Find the originating block by queue_index
    //             if let Some((originating_block_hash, originating_block_opt)) = treechain
    //                 .blocks
    //                 .get_index(utxo.queue_index.try_into().unwrap())
    //             {
    //                 let originating_block = originating_block_opt;
    //                 let originating_tx_opt = originating_block.tx.iter().find(|t| t.txid == *txid);
    //                 if let Some(originating_tx) = originating_tx_opt {
    //                     // Verify the output at vout matches the script_pubkey
    //                     if let Some(output) = originating_tx.vout.get(*vout as usize) {
    //                         if output.script_pubkey != *script {
    //                             println!(
    //                                 "⚠️ Script mismatch for UTXO {}:{} in block {}",
    //                                 txid, vout, originating_block.hash
    //                             );
    //                             continue;
    //                         }
    //                         let locktime = originating_tx.locktime;
    //                         let is_cltv = locktime != 0;
    //                         let mut lock: u32 = 0;
    //                         let mut is_csv = false;
    //                         if is_cltv {
    //                             lock = locktime;
    //                         } else {
    //                             // For CSV, find max relative lock from inputs' sequences
    //                             let mut max_relative = 0u32;
    //                             for input in &originating_tx.vin {
    //                                 if input.sequence != 4294967295u32 {
    //                                     is_csv = true;
    //                                     let relative = input.sequence & 0x0000FFFF;
    //                                     if relative > max_relative {
    //                                         max_relative = relative;
    //                                     }
    //                                 }
    //                             }
    //                             if is_csv {
    //                                 lock = max_relative;
    //                             } else {
    //                                 continue;
    //                             }
    //                         }
    //                         // Construct p2pkh_script for this wallet
    //                         let p2pkh_script = format!("76a914{}88ac", self.public_key_hash);
    //                         // Lock as 4 LE bytes hex
    //                         let lock_bytes = lock.to_le_bytes();
    //                         let lock_hex = format!(
    //                             "{:02x}{:02x}{:02x}{:02x}",
    //                             lock_bytes[0], lock_bytes[1], lock_bytes[2], lock_bytes[3]
    //                         );
    //                         let opcode = if is_cltv { "b1" } else { "b2" };
    //                         let redeem_script =
    //                             format!("04{}{}75{}", lock_hex, opcode, p2pkh_script);
    //                         let redeem_bytes = match hex::decode(&redeem_script) {
    //                             Ok(bytes) => bytes,
    //                             Err(e) => {
    //                                 println!(
    //                                     "⚠️ Failed to decode redeem script '{}': {}",
    //                                     redeem_script, e
    //                                 );
    //                                 continue;
    //                             }
    //                         };
    //                         // Compute redeem_hash = hash160(redeem_bytes)
    //                         let redeem_hash = ChainUtil::hash160(&redeem_bytes);
    //                         let expected_script_hash = &redeem_hash;
    //                         if expected_script_hash != extracted_script_hash {
    //                             println!(
    //                                 "⚠️ Script hash mismatch for UTXO {}:{} (expected: {}, got: {})",
    //                                 txid, vout, expected_script_hash, extracted_script_hash
    //                             );
    //                             continue;
    //                         }
    //                         // Check maturity
    //                         let lock_u64 = lock as u64;
    //                         let mut is_mature = true;
    //                         if is_cltv {
    //                             is_mature = if lock_u64 >= 500_000_000 {
    //                                 // Time-based CLTV
    //                                 current_time >= lock_u64
    //                             } else {
    //                                 // Height-based CLTV
    //                                 current_height >= lock_u64
    //                             };
    //                         } else if is_csv {
    //                             // CSV: blocks since confirmation >= lock
    //                             let blocks_since =
    //                                 current_height.saturating_sub(utxo.queue_index as u64);
    //                             is_mature = blocks_since >= lock_u64;
    //                         }
    //                         if is_mature {
    //                             unlocked_balance += utxo.out.value;
    //                         } else {
    //                             locked_balance += utxo.out.value;
    //                         }
    //                         println!(
    //                             "🔓/🔒 Mature: {}, UTXO: script_hash={}, value={}, is_cltv={}, lock={}, blocks_since={} (current_height={})",
    //                             if is_mature { "Yes" } else { "No" },
    //                             extracted_script_hash,
    //                             utxo.out.value,
    //                             is_cltv,
    //                             lock,
    //                             current_height.saturating_sub(utxo.queue_index as u64),
    //                             current_height
    //                         );
    //                     } else {
    //                         println!(
    //                             "⚠️ Invalid vout {} for txid {} in block {}",
    //                             vout, txid, originating_block.hash
    //                         );
    //                     }
    //                 } else {
    //                     println!(
    //                         "⚠️ Originating tx {} not found in block for queue_index {}",
    //                         txid, utxo.queue_index
    //                     );
    //                 }
    //             } else {
    //                 println!(
    //                     "⚠️ Originating block not found for queue_index {}",
    //                     utxo.queue_index
    //                 );
    //                 continue;
    //             }
    //         }
    //     }
    //     total_balance = unlocked_balance + locked_balance;

    //     println!(
    //         "✅ Balance for address {} (pkhash: {}): total {} satoshis (unlocked: {}, locked: {})",
    //         address, pubkey_hash, total_balance, unlocked_balance, locked_balance
    //     );

    //     (total_balance, unlocked_balance, locked_balance)
    // }

    pub fn get_utxos_for_address<'a>(
        address: &'a str,
        utxo_set: &'a UtxoSet,
        _treechain: &'a TreeChain,
    ) -> Vec<(String, u32, &'a Utxo)> {
        let pubkey_hash = match ChainUtil::pubkey_hash_from_address(address) {
            Ok(hash) => hash,
            Err(_) => {
                println!("❌ Invalid address: {}", address);
                return Vec::new();
            }
        };

        let expected_script = Transaction::create_p2pkh_script(&pubkey_hash);

        let mut utxos = Vec::new();

        for ((txid, vout), utxo) in &utxo_set.utxos {
            let script = &utxo.out.script_pubkey;
            if *script == expected_script {
                utxos.push((txid.clone(), *vout, utxo));
            }
        }
        utxos.sort_by_key(|(_, _, utxo)| utxo.queue_index);
        println!(
            "✅ Found {} mature UTXOs for address {}",
            utxos.len(),
            address
        );

        utxos
    }

    pub fn find_multisig_utxos(
        &self,
        utxo_set: &UtxoSet,
        pubkeys: &Vec<String>,
        m: u8,
    ) -> Vec<(String, u32, Utxo)> {
        if m as usize > pubkeys.len() || m == 0 {
            return vec![];
        }
        let redeem = Transaction::create_multisig_redeem_script(m, pubkeys);
        let script_pubkey = Transaction::create_p2sh_script(&redeem);
        utxo_set
            .utxos
            .iter()
            .filter(|((_, _), utxo)| utxo.out.script_pubkey == script_pubkey)
            .map(|((txid, vout), utxo)| (txid.clone(), *vout, utxo.clone()))
            .collect()
    }

    pub fn sign_multisig(
        &self,
        utxo_set: &UtxoSet,
        txid: &str,
        vout: u32,
        spending_tx: &Transaction,
        pubkeys: &Vec<String>,
        m: u8,
    ) -> Result<String, String> {
        // Verify UTXO exists
        let utxo = utxo_set
            .get_utxo(txid, vout)
            .ok_or_else(|| format!("UTXO not found: {}:{}", txid, vout))?;

        // Check if this wallet can sign the multisig UTXO
        let multisig_utxos = self.find_multisig_utxos(utxo_set, pubkeys, m);
        if !multisig_utxos
            .iter()
            .any(|(t, v, _)| t == txid && *v == vout)
        {
            return Err(
                "UTXO is not a multisig output or wallet is not part of the multisig".to_string(),
            );
        }

        // Reconstruct redeem script
        let redeem = Transaction::create_multisig_redeem_script(m, pubkeys);

        // Verify the UTXO's script_pubkey matches the expected P2SH script
        let expected_script = Transaction::create_p2sh_script(&redeem);
        if utxo.out.script_pubkey != expected_script {
            return Err("Script mismatch for P2SH multisig output".to_string());
        }

        // Compute sighash for the input
        let input_index = spending_tx
            .vin
            .iter()
            .position(|input| input.txid == txid && input.vout == vout)
            .ok_or_else(|| "Input not found in spending transaction".to_string())?;
        let sighash =
            spending_tx.compute_sighash(input_index, &redeem, utxo.out.value, SIGHASH_ALL);

        // Sign the sighash
        let signature = self.sign_data(&sighash);

        Ok(signature)
    }
}
