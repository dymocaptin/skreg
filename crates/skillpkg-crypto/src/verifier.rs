//! Signature verification against the embedded root CA.

use skillpkg_core::types::Sha256Digest;

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
