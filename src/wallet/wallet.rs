use crate::chain_util::ChainUtil;
use k256::EncodedPoint;
use k256::ecdsa::Signature;
use k256::ecdsa::signature::Signer;
use k256::ecdsa::signature::SignerMut;
use k256::ecdsa::{SigningKey, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use std::fmt;

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
}
