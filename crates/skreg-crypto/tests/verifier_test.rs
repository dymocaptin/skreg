use skreg_crypto::error::VerifyError;
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

// ---- RsaPssVerifier tests ----

use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rsa::pss::BlindedSigningKey;
use rsa::signature::hazmat::RandomizedPrehashSigner;
use rsa::signature::SignatureEncoding;
use sha2::Sha256;
use skreg_core::types::Sha256Digest;
use skreg_crypto::verifier::{RsaPssVerifier, SignatureVerifier};

/// Generate a self-signed cert with RSA-2048 (PKCS#1 signing), returning (ca_cert_pem, key_pem).
///
/// rcgen 0.12 exposes PKCS_RSA_SHA256 publicly; the PSS constant is pub(crate).
/// For testing the package-signature path, the cert signing algorithm doesn't matter —
/// we only use the cert to carry the public key.
fn make_test_ca(cn: &str) -> (String, String) {
    let mut rng = rand::thread_rng();
    let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let key_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap()
        .to_string();

    let key_pair = rcgen::KeyPair::from_pem_and_sign_algo(&key_pem, &rcgen::PKCS_RSA_SHA256)
        .expect("failed to load RSA key into rcgen");

    let mut params = rcgen::CertificateParams::new(vec![cn.to_owned()]);
    params.key_pair = Some(key_pair);
    params.alg = &rcgen::PKCS_RSA_SHA256;

    let cert = rcgen::Certificate::from_params(params).unwrap();
    (cert.serialize_pem().unwrap(), key_pem)
}

/// Sign pre-hashed digest bytes with RSA-PSS.
fn pss_sign(private_key_pem: &str, digest_hex: &str) -> Vec<u8> {
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(private_key_pem).unwrap();
    let signing_key = BlindedSigningKey::<Sha256>::new(private_key);
    let digest_bytes = hex::decode(digest_hex).unwrap();
    signing_key
        .sign_prehash_with_rng(&mut rand::thread_rng(), &digest_bytes)
        .expect("sign_prehash_with_rng failed")
        .to_bytes()
        .to_vec()
}

#[test]
fn pss_verifier_accepts_valid_self_signed_sig() {
    let (ca_pem, key_pem) = make_test_ca("test-ca");
    let digest_hex = "a".repeat(64);
    let signature = pss_sign(&key_pem, &digest_hex);
    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();

    let verifier = RsaPssVerifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, std::slice::from_ref(&ca_pem));
    assert!(result.is_ok(), "expected Ok, got {result:?}");
    assert!(!result.unwrap().ca_verified);
}

#[test]
fn pss_verifier_rejects_wrong_signature() {
    let (ca_pem, key_pem) = make_test_ca("test-ca");
    let digest_hex = "b".repeat(64);
    let mut signature = pss_sign(&key_pem, &digest_hex);
    signature[0] ^= 0xff;

    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();
    let verifier = RsaPssVerifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[ca_pem]);
    assert!(
        matches!(
            result,
            Err(skreg_crypto::error::VerifyError::SignatureMismatch)
        ),
        "expected SignatureMismatch, got {result:?}"
    );
}

#[test]
fn pss_verifier_rejects_empty_chain() {
    let (ca_pem, key_pem) = make_test_ca("test-ca");
    let digest_hex = "c".repeat(64);
    let signature = pss_sign(&key_pem, &digest_hex);
    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();

    let verifier = RsaPssVerifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[]);
    assert!(
        matches!(
            result,
            Err(skreg_crypto::error::VerifyError::InvalidCertChain(_))
        ),
        "expected InvalidCertChain, got {result:?}"
    );
}

#[test]
fn pss_verifier_rejects_chain_length_3() {
    let (ca_pem, _) = make_test_ca("test-ca");
    let digest = Sha256Digest::from_hex(&"d".repeat(64)).unwrap();
    let verifier = RsaPssVerifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &[], &[ca_pem.clone(), ca_pem.clone(), ca_pem]);
    assert!(
        matches!(
            result,
            Err(skreg_crypto::error::VerifyError::InvalidCertChain(_))
        ),
        "expected InvalidCertChain, got {result:?}"
    );
}

#[test]
fn pss_verifier_rejects_signature_from_different_key() {
    let (ca_pem, _) = make_test_ca("test-ca");
    let (_, other_key_pem) = make_test_ca("other-ca");

    let digest_hex = "e".repeat(64);
    let signature = pss_sign(&other_key_pem, &digest_hex);

    let digest = Sha256Digest::from_hex(&digest_hex).unwrap();
    let verifier = RsaPssVerifier::new_with_root_pem(ca_pem.as_bytes());
    let result = verifier.verify(&digest, &signature, &[ca_pem]);
    assert!(
        matches!(
            result,
            Err(skreg_crypto::error::VerifyError::SignatureMismatch)
        ),
        "expected SignatureMismatch, got {result:?}"
    );
}

#[test]
fn verify_error_displays_cert_expired() {
    let e = VerifyError::CertExpired("2025-01-01".to_string());
    assert!(e.to_string().contains("expired"));
}

#[test]
fn verify_error_displays_cn_mismatch() {
    let e = VerifyError::CnMismatch {
        expected: "acme".to_string(),
        got: "other".to_string(),
    };
    assert!(e.to_string().contains("acme"));
}
