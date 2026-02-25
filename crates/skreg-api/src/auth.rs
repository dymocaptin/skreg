//! API key and OTP generation + hashing utilities.

use rand::Rng;
use sha2::{Digest, Sha256};

/// Generate a random API key with a `skreg_` prefix.
#[must_use]
pub fn generate_api_key() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen::<u8>()).collect();
    format!("skreg_{}", hex::encode(bytes))
}

/// Generate a 6-digit numeric OTP.
#[must_use]
pub fn generate_otp() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}

/// SHA-256 hex digest of `input`. Used for both API keys and OTPs.
#[must_use]
pub fn hash_secret(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_key_has_prefix() {
        let key = generate_api_key();
        assert!(key.starts_with("skreg_"));
        assert!(key.len() > 32);
    }

    #[test]
    fn hash_is_deterministic() {
        let h1 = hash_secret("abc");
        let h2 = hash_secret("abc");
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_inputs_differ() {
        assert_ne!(hash_secret("abc"), hash_secret("xyz"));
    }

    #[test]
    fn otp_is_six_digits() {
        let otp = generate_otp();
        assert_eq!(otp.len(), 6);
        assert!(otp.chars().all(|c: char| c.is_ascii_digit()));
    }
}
