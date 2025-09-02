use k256::EncodedPoint;
use k256::ecdsa::{SigningKey, VerifyingKey};
use rand_core::OsRng;

pub struct ChainUtil;

impl ChainUtil {
    pub fn gen_key_pair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        (signing_key, verifying_key)
    }
}
