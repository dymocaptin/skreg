//! Signature verification against the embedded root CA.

use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::hazmat::PrehashVerifier;
use rsa::RsaPublicKey;
use sha2::Sha256;
use skreg_core::types::Sha256Digest;
use x509_cert::der::{DecodePem, Encode};
use x509_cert::Certificate;

use crate::error::VerifyError;

/// The identity of a verified signer extracted from a certificate chain.
#[derive(Debug, Clone)]
pub struct VerifiedSigner {
    /// Certificate serial number, used to check revocation.
    pub cert_serial: Option<u64>,
    /// Human-readable subject common name.
    pub common_name: String,
}

/// Verifies a detached package signature against a certificate chain and root CA.
pub trait SignatureVerifier: Send + Sync {
    /// Verify a detached `signature` over the given `digest`.
    ///
    /// The `cert_chain_pem` is an ordered list of PEM-encoded certificates
    /// (leaf first, ending at an intermediate CA signed by the root CA embedded
    /// in the verifier implementation).
    ///
    /// # Errors
    ///
    /// Returns [`VerifyError`] if the chain is invalid, the signature does not
    /// match, or any certificate in the chain has been revoked.
    fn verify(
        &self,
        digest: &Sha256Digest,
        signature: &[u8],
        cert_chain_pem: &[String],
    ) -> Result<VerifiedSigner, VerifyError>;
}

/// Path to the bundled root CA certificate (relative to this file).
const ROOT_CA_PEM: &[u8] = include_bytes!("../../../certs/root-ca.pem");

/// RSA PKCS#1 v1.5 + SHA-256 verifier backed by a bundled root CA.
///
/// For registry-signed packages (`cert_chain_pem` is empty), verifies
/// the signature against the root CA's public key directly.
pub struct RsaPkcs1Verifier {
    root_ca_pem: Vec<u8>,
}

impl RsaPkcs1Verifier {
    /// Create a verifier using the bundled root CA.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root_ca_pem: ROOT_CA_PEM.to_vec(),
        }
    }

    /// Create a verifier with a custom root CA PEM.
    ///
    /// Intended for testing and self-hosted registries; production code
    /// should use [`RsaPkcs1Verifier::new`] to use the bundled root CA.
    #[must_use]
    pub fn new_with_root_pem(pem: &[u8]) -> Self {
        Self {
            root_ca_pem: pem.to_vec(),
        }
    }

    fn root_public_key(&self) -> Result<RsaPublicKey, VerifyError> {
        let cert = Certificate::from_pem(&self.root_ca_pem)
            .map_err(|e| VerifyError::Der(e.to_string()))?;
        let spki_der = cert
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|e| VerifyError::Der(e.to_string()))?;
        RsaPublicKey::from_public_key_der(&spki_der)
            .map_err(|e| VerifyError::Der(e.to_string()))
    }
}

impl Default for RsaPkcs1Verifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for RsaPkcs1Verifier {
    fn verify(
        &self,
        digest: &Sha256Digest,
        signature: &[u8],
        cert_chain_pem: &[String],
    ) -> Result<VerifiedSigner, VerifyError> {
        if !cert_chain_pem.is_empty() {
            return Err(VerifyError::InvalidCertChain(
                "non-empty cert chain not yet supported".into(),
            ));
        }

        let public_key = self.root_public_key()?;
        let verifying_key = VerifyingKey::<Sha256>::new(public_key);

        // Sha256Digest::from_hex validates hex at construction; this cannot fail.
        let digest_bytes = hex::decode(digest.as_hex())
            .expect("Sha256Digest always contains valid lowercase hex");

        let sig = RsaSignature::try_from(signature)
            .map_err(|_| VerifyError::SignatureMismatch)?;

        // verify_prehash treats digest_bytes as the pre-computed SHA-256 hash
        // and applies PKCS#1 v1.5 without re-hashing.
        verifying_key
            .verify_prehash(&digest_bytes, &sig)
            .map_err(|_| VerifyError::SignatureMismatch)?;

        Ok(VerifiedSigner {
            cert_serial: None,
            common_name: "registry".into(),
        })
    }
}
