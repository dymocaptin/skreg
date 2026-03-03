use skreg_crypto::revocation::{InMemoryRevocationStore, RevocationStore};

#[test]
fn in_memory_store_starts_empty() {
    let store = InMemoryRevocationStore::new();
    assert!(!store.is_revoked(1).unwrap());
}

#[test]
fn in_memory_store_tracks_revoked_serials() {
    let mut store = InMemoryRevocationStore::new();
    store.revoke(42);
    assert!(store.is_revoked(42).unwrap());
    assert!(!store.is_revoked(1).unwrap());
}

// ---- RsaPkcs1Verifier tests ----

use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rsa::signature::hazmat::PrehashSigner;
use rsa::signature::SignatureEncoding;
use sha2::Sha256;
use skreg_core::types::Sha256Digest;
use skreg_crypto::verifier::{RsaPkcs1Verifier, SignatureVerifier};

/// Generate a self-signed CA cert with an RSA-2048 key, returning (ca_cert_pem, private_key_pkcs8_pem).
fn make_test_ca() -> (String, String) {
    let mut rng = rand::thread_rng();
    let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let key_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap()
        .to_string();

    let key_pair = rcgen::KeyPair::from_pem_and_sign_algo(&key_pem, &rcgen::PKCS_RSA_SHA256)
        .expect("failed to load RSA key into rcgen");

    let mut params = rcgen::CertificateParams::new(vec!["test-ca".to_owned()]);
    params.key_pair = Some(key_pair);
    params.alg = &rcgen::PKCS_RSA_SHA256;

    let cert = rcgen::Certificate::from_params(params).unwrap();
    let ca_pem = cert.serialize_pem().unwrap();

    (ca_pem, key_pem)
}

/// Sign `digest_hex`'s decoded bytes with an RSA private key from PKCS8 PEM.
///
/// Uses `sign_prehash` so the bytes are treated as a pre-computed hash without
/// re-hashing, matching the behavior of `skreg_worker::stages::signing::sign_bytes`.
fn sign_digest(private_key_pem: &str, digest_hex: &str) -> Vec<u8> {
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(private_key_pem).unwrap();
    let signing_key = SigningKey::<Sha256>::new(private_key);
    let digest_bytes = hex::decode(digest_hex).unwrap();
    signing_key
        .sign_prehash(&digest_bytes)
        .expect("sign_prehash failed")
        .to_bytes()
        .to_vec()
}

#[test]
fn verifier_accepts_valid_signature() {
    let (ca_pem, key_pem) = make_test_ca();
    let digest_hex = "a".repeat(64);
    let signature = sign_digest(&key_pem, &digest_hex);
    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();

    let verifier = RsaPkcs1Verifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[]);
    assert!(result.is_ok(), "expected Ok, got {result:?}");
}

#[test]
fn verifier_rejects_wrong_signature() {
    let (ca_pem, key_pem) = make_test_ca();
    let digest_hex = "b".repeat(64);
    let mut signature = sign_digest(&key_pem, &digest_hex);
    signature[0] ^= 0xff; // corrupt first byte

    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();
    let verifier = RsaPkcs1Verifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[]);
    assert!(
        matches!(result, Err(skreg_crypto::error::VerifyError::SignatureMismatch)),
        "expected SignatureMismatch, got {result:?}"
    );
}

#[test]
fn verifier_rejects_nonempty_cert_chain() {
    let (ca_pem, _) = make_test_ca();
    let digest = Sha256Digest::from_hex(&"a".repeat(64)).unwrap();
    let verifier = RsaPkcs1Verifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &[], &["fake-cert".to_owned()]);
    assert!(
        matches!(result, Err(skreg_crypto::error::VerifyError::InvalidCertChain(_))),
        "expected InvalidCertChain, got {result:?}"
    );
}

#[test]
fn verifier_rejects_signature_from_different_key() {
    let (ca_pem, _) = make_test_ca();
    let (_, other_key_pem) = make_test_ca(); // different key

    let digest_hex = "c".repeat(64);
    let signature = sign_digest(&other_key_pem, &digest_hex); // signed with wrong key

    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();
    let verifier = RsaPkcs1Verifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[]);
    assert!(
        matches!(result, Err(skreg_crypto::error::VerifyError::SignatureMismatch)),
        "expected SignatureMismatch, got {result:?}"
    );
}
