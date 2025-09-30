use crate::chain_util::ChainUtil;
use crate::config::SIGHASH_ALL;
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

    pub fn get_utxos_for_address<'a>(
        address: &'a str,
        utxo_set: &'a UtxoSet,
    ) -> Vec<(String, u32, &'a Utxo)> {
        let pubkey_hash = match ChainUtil::pubkey_hash_from_address(address) {
            Ok(hash) => hash,
            Err(_) => {
                println!("❌ Invalid address: {}", address);
                return Vec::new();
            }
        };

        let expected_script = Transaction::create_p2pkh_script(&pubkey_hash);

        let utxos = utxo_set
            .utxos
            .iter()
            .filter(|((_, _), utxo)| utxo.out.script_pubkey == expected_script)
            .map(|((txid, vout), utxo)| (txid.clone(), *vout, utxo))
            .collect::<Vec<_>>();

        println!("✅ Found {} UTXOs for address {}", utxos.len(), address);

        utxos
    }
    pub fn find_multisig_utxos(
        &self,
        utxo_set: &UtxoSet,
        pubkeys: &Vec<String>,
        m: u8,
    ) -> Vec<(String, u32, Utxo)> {
        if !pubkeys.contains(&self.public_key) {
            return vec![];
        }
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
