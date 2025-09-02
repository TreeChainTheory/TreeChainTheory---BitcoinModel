use crate::chain_util::ChainUtil;
use crate::config::INITIAL_BALANCE;
use k256::EncodedPoint;
use k256::ecdsa::{SigningKey, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use std::fmt;

pub struct Wallet {
    pub key_pair: SigningKey,
    pub public_key: String,
}

impl Wallet {
    pub fn new() -> Self {
        let (key_pair, verifying_key) = ChainUtil::gen_key_pair();
        let public_key_point: EncodedPoint = verifying_key.to_encoded_point(false);
        let public_key_hex = hex::encode(public_key_point.as_bytes());

        Self {
            key_pair,
            public_key: public_key_hex,
        }
    }
}
