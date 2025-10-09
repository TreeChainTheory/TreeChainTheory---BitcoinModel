use bs58;
use k256::ecdsa::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use ripemd::{Digest as RipemdDigest, Ripemd160};
use sha2::Sha256;
pub struct ChainUtil;

impl ChainUtil {
    pub fn gen_key_pair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        (signing_key, verifying_key)
    }

    pub fn hash160(data: &Vec<u8>) -> String {
        // SHA256
        let sha256 = Sha256::digest(&data);

        // RIPEMD160
        let ripemd = Ripemd160::digest(&sha256);

        hex::encode(ripemd)
    }

    pub fn pubkey_hash_from_pubkey(pubkey_hex: &str) -> String {
        let pubkey_bytes = hex::decode(pubkey_hex).expect("Invalid pubkey hex");

        // SHA256
        let sha256 = Sha256::digest(&pubkey_bytes);

        // RIPEMD160
        let ripemd = Ripemd160::digest(&sha256);

        hex::encode(ripemd)
    }

    pub fn pubkey_hash_from_address(address: &str) -> Result<String, String> {
        // Decode Base58Check address
        let decoded = bs58::decode(address)
            .into_vec()
            .map_err(|e| format!("Base58 decode error: {}", e))?;

        // Check length (25 bytes for P2PKH: 1 version byte + 20 hash bytes + 4 checksum bytes)
        if decoded.len() != 25 {
            return Err("Invalid address length".to_string());
        }

        // Verify version byte (0x00 for P2PKH mainnet)
        if decoded[0] != 0x00 {
            return Err("Invalid address version (only P2PKH supported)".to_string());
        }

        // Verify checksum
        let payload = &decoded[0..21]; // version + pubkey hash
        let checksum = &decoded[21..25];
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        if checksum != &hash2[0..4] {
            return Err("Invalid address checksum".to_string());
        }

        // Extract 20-byte pubkey hash
        let pubkey_hash = &decoded[1..21];
        Ok(hex::encode(pubkey_hash))
    }

    pub fn address_from_pubkey_hash(pubkey_hash: &str) -> Result<String, String> {
        let hash_bytes =
            hex::decode(pubkey_hash).map_err(|e| format!("Invalid pubkey hash hex: {}", e))?;
        if hash_bytes.len() != 20 {
            return Err("Pubkey hash must be 20 bytes".to_string());
        }

        // Create payload: version byte (0x00 for P2PKH) + pubkey hash
        let mut payload = vec![0x00];
        payload.extend_from_slice(&hash_bytes);

        // Compute checksum: first 4 bytes of SHA256(SHA256(payload))
        let mut hasher = Sha256::new();
        hasher.update(&payload);
        let hash1 = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
        payload.extend_from_slice(&hash2[0..4]);

        // Encode to Base58Check
        Ok(bs58::encode(payload).into_string())
    }
}
