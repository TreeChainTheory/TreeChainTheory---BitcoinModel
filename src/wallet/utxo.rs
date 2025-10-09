use crate::wallet::transaction::Transaction;
use crate::wallet::transaction::TxOutput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Utxo {
    pub out: TxOutput,    // Transaction output (amount and script)
    pub queue_index: u32, // queue_index of that block
    pub f_coinbase: bool, // True if from a coinbase transaction
}

impl Utxo {
    pub fn new(out: TxOutput, queue_index: u32, f_coinbase: bool) -> Self {
        Utxo {
            out,
            queue_index,
            f_coinbase,
        }
    }

    pub fn extract_utxo(txn: &Transaction, vout: u32, queue_index: u32) -> Option<Self> {
        if vout as usize >= txn.vout.len() {
            return None;
        }

        // Determine if the transaction is a coinbase transaction
        let is_coinbase =
            txn.vin.len() == 1 && txn.vin[0].txid == "00".repeat(32) && txn.vin[0].vout == u32::MAX;

        // Extract the TxOutput at the specified vout index
        let out = txn.vout[vout as usize].clone();

        Some(Utxo {
            out,
            queue_index,
            f_coinbase: is_coinbase,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UtxoSet {
    pub utxos: HashMap<(String, u32), Utxo>, // Key: (txid, output index), Value: Utxo
}

impl UtxoSet {
    pub fn new() -> Self {
        UtxoSet {
            utxos: HashMap::new(),
        }
    }

    // Add a UTXO to the set
    pub fn add_utxo(&mut self, txid: String, vout: u32, utxo: Utxo) {
        self.utxos.insert((txid, vout), utxo);
    }

    // Remove a UTXO from the set (e.g., when spent)
    pub fn remove_utxo(&mut self, txid: &str, vout: u32) -> Option<Utxo> {
        self.utxos.remove(&(txid.to_string(), vout))
    }

    // Get a UTXO by txid and vout
    pub fn get_utxo(&self, txid: &str, vout: u32) -> Option<&Utxo> {
        self.utxos.get(&(txid.to_string(), vout))
    }

    // Check if a UTXO exists
    pub fn has_utxo(&self, txid: &str, vout: u32) -> bool {
        self.utxos.contains_key(&(txid.to_string(), vout))
    }

    // Get the total value of all UTXOs
    pub fn total_value(&self) -> u64 {
        self.utxos.values().map(|utxo| utxo.out.value).sum()
    }
}
