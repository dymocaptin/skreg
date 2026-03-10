//! Signature verification against the embedded root CA.

use rsa::pkcs8::DecodePublicKey;
use rsa::pss::{Signature as PssSignature, VerifyingKey as PssVerifyingKey};
use rsa::signature::hazmat::PrehashVerifier;
use rsa::signature::Verifier;
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
    /// Whether the cert was verified by the skreg Publisher CA (`true`) or is self-signed (`false`).
    pub ca_verified: bool,
}

/// Verifies a detached package signature against a certificate chain and root CA.
pub trait SignatureVerifier: Send + Sync {
    /// Verify a detached `signature` over the given `digest`.
    ///
    /// `cert_chain_pem` must have exactly 1 entry (self-signed) or 2 entries
    /// (CA-verified: leaf first, then Publisher CA cert).
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

/// RSA-PSS + SHA-256 verifier backed by a bundled root CA.
///
/// `cert_chain_pem` must contain exactly 1 certificate (self-signed publisher)
/// or 2 certificates (leaf + Publisher CA, in that order).
pub struct RsaPssVerifier {
    root_ca_pem: Vec<u8>,
}

impl RsaPssVerifier {
    /// Create a verifier using the bundled root CA.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root_ca_pem: ROOT_CA_PEM.to_vec(),
        }
    }

    /// Create a verifier with a custom root CA PEM.
    ///
    /// Intended for testing; production code should use [`RsaPssVerifier::new`].
    #[must_use]
    pub fn new_with_root_pem(pem: &[u8]) -> Self {
        Self {
            root_ca_pem: pem.to_vec(),
        }
    }

    fn parse_cert(pem: &str) -> Result<Certificate, VerifyError> {
        Certificate::from_pem(pem.as_bytes()).map_err(|e| VerifyError::Der(e.to_string()))
    }

    fn extract_rsa_public_key(cert: &Certificate) -> Result<RsaPublicKey, VerifyError> {
        let spki_der = cert
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|e| VerifyError::Der(e.to_string()))?;
        RsaPublicKey::from_public_key_der(&spki_der).map_err(|e| VerifyError::Der(e.to_string()))
    }

    fn extract_common_name(cert: &Certificate) -> String {
        cert.tbs_certificate.subject.to_string()
    }

    fn extract_serial(cert: &Certificate) -> Option<u64> {
        cert.tbs_certificate
            .serial_number
            .as_bytes()
            .get(..8)
            .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
    }

    /// Verify that `cert` was signed by `signer` using RSA-PSS / SHA-256.
    fn verify_cert_signed_by(cert: &Certificate, signer: &Certificate) -> Result<(), VerifyError> {
        let tbs_der = cert
            .tbs_certificate
            .to_der()
            .map_err(|e| VerifyError::Der(e.to_string()))?;
        let sig_bytes = cert
            .signature
            .as_bytes()
            .ok_or_else(|| VerifyError::InvalidCertChain("no cert signature bytes".into()))?;
        let sig = PssSignature::try_from(sig_bytes)
            .map_err(|_| VerifyError::InvalidCertChain("cert signature not valid PSS".into()))?;
        let signer_key = Self::extract_rsa_public_key(signer)?;
        let verifying_key = PssVerifyingKey::<Sha256>::new(signer_key);
        verifying_key
            .verify(&tbs_der, &sig)
            .map_err(|_| VerifyError::InvalidCertChain("cert signature verification failed".into()))
    }

    /// Verify a package signature (RSA-PSS over prehashed digest).
    fn verify_package_sig(
        public_key: &RsaPublicKey,
        digest: &Sha256Digest,
        sig_bytes: &[u8],
    ) -> Result<(), VerifyError> {
        let digest_bytes =
            hex::decode(digest.as_hex()).expect("Sha256Digest always contains valid hex");
        let sig = PssSignature::try_from(sig_bytes).map_err(|_| VerifyError::SignatureMismatch)?;
        let verifying_key = PssVerifyingKey::<Sha256>::new(public_key.clone());
        verifying_key
            .verify_prehash(&digest_bytes, &sig)
            .map_err(|_| VerifyError::SignatureMismatch)
    }

    fn verify_self_signed(
        cert: &Certificate,
        digest: &Sha256Digest,
        sig_bytes: &[u8],
    ) -> Result<VerifiedSigner, VerifyError> {
        let public_key = Self::extract_rsa_public_key(cert)?;
        Self::verify_package_sig(&public_key, digest, sig_bytes)?;
        Ok(VerifiedSigner {
            cert_serial: Self::extract_serial(cert),
            common_name: Self::extract_common_name(cert),
            ca_verified: false,
        })
    }

    fn verify_ca_chain(
        &self,
        leaf_cert: &Certificate,
        intermediate_cert: &Certificate,
        digest: &Sha256Digest,
        sig_bytes: &[u8],
    ) -> Result<VerifiedSigner, VerifyError> {
        let root_cert = Certificate::from_pem(&self.root_ca_pem)
            .map_err(|e| VerifyError::Der(e.to_string()))?;
        Self::verify_cert_signed_by(intermediate_cert, &root_cert)?;
        Self::verify_cert_signed_by(leaf_cert, intermediate_cert)?;
        let leaf_key = Self::extract_rsa_public_key(leaf_cert)?;
        Self::verify_package_sig(&leaf_key, digest, sig_bytes)?;
        Ok(VerifiedSigner {
            cert_serial: Self::extract_serial(leaf_cert),
            common_name: Self::extract_common_name(leaf_cert),
            ca_verified: true,
        })
    }
}

impl Default for RsaPssVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for RsaPssVerifier {
    fn verify(
        &self,
        digest: &Sha256Digest,
        signature: &[u8],
        cert_chain_pem: &[String],
    ) -> Result<VerifiedSigner, VerifyError> {
        match cert_chain_pem.len() {
            1 => {
                let cert = Self::parse_cert(&cert_chain_pem[0])?;
                Self::verify_self_signed(&cert, digest, signature)
            }
            2 => {
                let leaf = Self::parse_cert(&cert_chain_pem[0])?;
                let intermediate = Self::parse_cert(&cert_chain_pem[1])?;
                self.verify_ca_chain(&leaf, &intermediate, digest, signature)
            }
            _ => Err(VerifyError::InvalidCertChain(format!(
                "expected 1 or 2 certs, got {}",
                cert_chain_pem.len()
            ))),
        }
    }
}
