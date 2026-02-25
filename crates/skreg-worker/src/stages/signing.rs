//! Stage 4: load CA key from Secrets Manager, sign tarball sha256, write .sig to S3.

use anyhow::{Context, Result};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SmClient;
use rand::CryptoRng;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::RsaPrivateKey;
use sha2::Sha256;

/// Sign `data` with `signing_key` using RSA PKCS#1v1.5 + SHA-256.
pub fn sign_bytes<R: rand::RngCore + CryptoRng>(
    signing_key: &SigningKey<Sha256>,
    data: &[u8],
    rng: &mut R,
) -> Vec<u8> {
    signing_key.sign_with_rng(rng, data).to_bytes().to_vec()
}

/// Load the CA private key from Secrets Manager, sign the tarball sha256,
/// upload the `.sig` file to S3, and return the S3 key of the signature.
///
/// # Errors
///
/// Returns an error if the secret cannot be fetched, the key cannot be parsed,
/// or the S3 upload fails.
pub async fn run_signing(
    tarball_sha256: &str,
    storage_path: &str,
    s3: &S3Client,
    sm: &SmClient,
    bucket: &str,
    ca_secret_arn: &str,
) -> Result<String> {
    // 1. Load CA private key from Secrets Manager
    let secret = sm
        .get_secret_value()
        .secret_id(ca_secret_arn)
        .send()
        .await
        .context("fetching CA private key from Secrets Manager")?;

    let secret_str = secret.secret_string()
        .context("CA secret has no string value")?;
    let secret_json: serde_json::Value = serde_json::from_str(secret_str)
        .context("parsing CA secret JSON")?;
    let pem = secret_json["private_key"].as_str()
        .context("CA secret missing 'private_key' field")?;

    // 2. Parse RSA private key
    let private_key = RsaPrivateKey::from_pkcs8_pem(pem)
        .context("parsing RSA private key PEM")?;
    let signing_key = SigningKey::<Sha256>::new(private_key);

    // 3. Sign the sha256 digest bytes (rng dropped before any await)
    let digest_bytes = hex::decode(tarball_sha256)
        .context("decoding sha256 hex")?;
    let signature = {
        let mut rng = rand::thread_rng();
        sign_bytes(&signing_key, &digest_bytes, &mut rng)
    };

    // 4. Write .sig to S3
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
    use rsa::{RsaPrivateKey, pkcs1v15::{SigningKey, VerifyingKey}};
    use rsa::signature::Verifier;

    #[test]
    fn sign_and_verify_roundtrip() {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let signing_key = SigningKey::<Sha256>::new(private_key.clone());
        let verifying_key = VerifyingKey::<Sha256>::new(private_key.to_public_key());

        let data = b"test digest";
        let sig = sign_bytes(&signing_key, data, &mut rng);
        assert!(verifying_key.verify(data, &rsa::pkcs1v15::Signature::try_from(sig.as_slice()).unwrap()).is_ok());
    }
}
