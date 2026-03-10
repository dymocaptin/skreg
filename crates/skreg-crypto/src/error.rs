//! Error types for cryptographic operations.

use thiserror::Error;

/// Errors that can occur during signature verification.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// The certificate chain could not be validated up to the root CA.
    #[error("certificate chain validation failed: {0}")]
    InvalidCertChain(String),
    /// The signature does not match the digest.
    #[error("signature mismatch")]
    SignatureMismatch,
    /// The signing certificate has been revoked.
    #[error("certificate {serial} has been revoked")]
    Revoked {
        /// The revoked certificate serial number.
        serial: u64,
    },
    /// A DER/ASN.1 parsing error.
    #[error("DER parsing error: {0}")]
    Der(String),
    /// The certificate's validity period has passed.
    #[error("certificate expired on {0}")]
    CertExpired(String),
    /// The certificate is not yet valid.
    #[error("certificate not yet valid until {0}")]
    CertNotYetValid(String),
    /// The certificate common name does not match the expected namespace.
    #[error("certificate CN mismatch: expected {expected}, got {got}")]
    CnMismatch {
        /// The expected common name (namespace slug).
        expected: String,
        /// The actual common name found in the certificate.
        got: String,
    },
    /// The self-signed publisher key has been revoked by the registry.
    #[error("publisher key has been revoked by the registry")]
    SelfSignedKeyRevoked,
}

/// Errors that can occur when checking or refreshing revocation state.
#[derive(Debug, Error)]
pub enum RevocationError {
    /// Network error fetching the CRL.
    #[error("failed to fetch CRL: {0}")]
    Network(String),
    /// The CRL response was not parseable.
    #[error("failed to parse CRL: {0}")]
    Parse(String),
}
