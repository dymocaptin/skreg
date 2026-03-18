//! Stage 5: sign tarball sha256 using the provided CA private key PEM, write .sig to S3.

use anyhow::{Context, Result};
use aws_sdk_s3::Client as S3Client;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::hazmat::PrehashSigner;
use rsa::signature::SignatureEncoding;
use rsa::RsaPrivateKey;
use sha2::Sha256;

/// Sign `data` (a pre-computed hash) with `signing_key` using RSA PKCS#1v1.5 + SHA-256.
///
/// `data` must be the raw hash bytes — the signing key applies PKCS#1v1.5 padding
/// with the SHA-256 OID without re-hashing.
///
/// # Panics
///
/// Panics if `data` is not exactly 32 bytes (a valid SHA-256 hash length).
#[must_use]
pub fn sign_bytes(signing_key: &SigningKey<Sha256>, data: &[u8]) -> Vec<u8> {
    signing_key
        .sign_prehash(data)
        .expect("sign_prehash failed: invalid key or hash length")
        .to_bytes()
        .to_vec()
}

/// Sign the tarball sha256 using the provided CA private key PEM,
/// upload the `.sig` file to S3, and return the S3 key of the signature.
///
/// # Errors
///
/// Returns an error if the key cannot be parsed or the S3 upload fails.
pub async fn run_signing(
    tarball_sha256: &str,
    storage_path: &str,
    s3: &S3Client,
    bucket: &str,
    registry_ca_key_pem: &str,
) -> Result<String> {
    // 1. Parse RSA private key (PKCS#1 PEM)
    let private_key = RsaPrivateKey::from_pkcs1_pem(registry_ca_key_pem)
        .context("parsing RSA private key PEM")?;
    let signing_key = SigningKey::<Sha256>::new(private_key);

    // 2. Sign the sha256 digest bytes
    let digest_bytes = hex::decode(tarball_sha256).context("decoding sha256 hex")?;
    let signature = sign_bytes(&signing_key, &digest_bytes);

    // 3. Write .sig to S3
    let sig_path = storage_path.replace(".skill", ".sig");
    s3.put_object()
        .bucket(bucket)
        .key(&sig_path)
        .body(aws_sdk_s3::primitives::ByteStream::from(signature))
        .send()
        .await
        .context("uploading .sig to S3")?;

    Ok(sig_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::signature::hazmat::PrehashVerifier;
    use rsa::{
        pkcs1v15::{SigningKey, VerifyingKey},
        RsaPrivateKey,
    };

    #[test]
    fn sign_and_verify_roundtrip() {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let signing_key = SigningKey::<Sha256>::new(private_key.clone());
        let verifying_key = VerifyingKey::<Sha256>::new(private_key.to_public_key());

        // data is treated as a pre-computed hash (no internal re-hashing)
        let data = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 64 bytes
        let sig = sign_bytes(&signing_key, data);
        let sig_obj = rsa::pkcs1v15::Signature::try_from(sig.as_slice()).unwrap();
        assert!(verifying_key.verify_prehash(data, &sig_obj).is_ok());
    }
}
