//! Stage 4: verify publisher signature and certificate chain.

use anyhow::{bail, Context, Result};
use aws_sdk_s3::Client as S3Client;
use skreg_core::types::Sha256Digest;
use skreg_crypto::{
    error::VerifyError,
    verifier::{RsaPssVerifier, SignatureVerifier},
};
use sqlx::PgPool;
use uuid::Uuid;

/// Kinds of publisher verification failure.
pub(crate) enum FailureKind {
    /// The signature bytes do not match the tarball digest.
    SignatureMismatch,
    /// The signing certificate expired on the given date.
    CertExpired(String),
    /// The certificate common name does not match the package namespace.
    CnMismatch,
    /// The certificate chain could not be validated.
    ChainInvalid,
    /// The CA-issued certificate with this serial number has been revoked.
    CertRevoked(i64),
    /// The self-signed publisher key has been revoked by the registry.
    SelfSignedKeyRevoked,
    /// The certificate chain has an unsupported length (not 1 or 2).
    InvalidChainLength,
}

/// Return an actionable user-facing message for a [`FailureKind`].
pub(crate) fn failure_message(kind: FailureKind) -> String {
    match kind {
        FailureKind::SignatureMismatch => "The package signature does not match its contents. \
             Re-pack the package with `skreg pack` and publish again."
            .to_owned(),
        FailureKind::CertExpired(date) => {
            format!(
                "Your signing certificate expired on {date}. \
                 Renew it with `skreg certify` and publish again."
            )
        }
        FailureKind::CnMismatch => {
            "The certificate common name does not match the package namespace. \
             Re-issue your certificate with `skreg certify` and ensure you \
             select the correct namespace."
                .to_owned()
        }
        FailureKind::ChainInvalid => {
            "The certificate chain could not be validated against the skreg root CA. \
             Ensure you are providing the complete chain from your leaf certificate \
             up to the Publisher CA."
                .to_owned()
        }
        FailureKind::CertRevoked(serial) => {
            format!(
                "Certificate with serial {serial} has been revoked. \
                 Obtain a new certificate with `skreg certify` and publish again."
            )
        }
        FailureKind::SelfSignedKeyRevoked => {
            "Your self-signed publisher key has been revoked by the registry. \
             Please contact support to resolve this issue."
                .to_owned()
        }
        FailureKind::InvalidChainLength => {
            "The certificate chain must contain exactly 1 or 2 certificates \
             (self-signed leaf, or leaf + Publisher CA)."
                .to_owned()
        }
    }
}

/// Run Stage 4: verify the publisher signature and update the `signer` column.
///
/// Loads the `.skill` tarball from S3, reads `manifest.json`, verifies the
/// RSA-PSS signature against the certificate chain, and writes the signer kind
/// (`"self_signed"` or `"publisher"`) back to the `versions` table.
///
/// # Errors
///
/// Returns an error if the S3 download, tarball unpacking, manifest parsing,
/// revocation lookup, or signature verification fails.
pub async fn run_verify_publisher(
    version_id: Uuid,
    sha256: &str,
    storage_path: &str,
    namespace: &str,
    pool: &PgPool,
    s3: &S3Client,
    bucket: &str,
) -> Result<()> {
    // 1. Download tarball from S3
    let obj = s3
        .get_object()
        .bucket(bucket)
        .key(storage_path)
        .send()
        .await
        .context("downloading tarball from S3")?;
    let bytes = obj
        .body
        .collect()
        .await
        .context("reading S3 object body")?
        .into_bytes();

    // 2. Unpack and read manifest.json
    let tmp = skreg_pack::unpack::unpack_to_tempdir(&bytes).context("unpacking tarball")?;
    let manifest_raw = std::fs::read_to_string(tmp.path().join("manifest.json"))
        .context("reading manifest.json")?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).context("parsing manifest.json")?;

    // 3. publisher_sig_hex must be present
    let sig_hex = manifest["publisher_sig_hex"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("{}", failure_message(FailureKind::SignatureMismatch)))?;

    let sig_bytes = hex::decode(sig_hex).context("decoding publisher_sig_hex")?;

    // 4. cert_chain must be 1 or 2 entries
    let cert_chain: Vec<String> = manifest["cert_chain"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("{}", failure_message(FailureKind::InvalidChainLength)))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();

    if cert_chain.is_empty() || cert_chain.len() > 2 {
        bail!("{}", failure_message(FailureKind::InvalidChainLength));
    }

    let chain_len = cert_chain.len();

    // 5. Load revoked serials from publisher_certs table
    let revoked_serials: Vec<i64> =
        sqlx::query_scalar("SELECT serial FROM publisher_certs WHERE revoked_at IS NOT NULL")
            .fetch_all(pool)
            .await
            .context("loading revoked publisher cert serials")?;

    // 6. For self-signed (chain len == 1): check revoked_self_signed_keys
    if chain_len == 1 {
        // Derive SPKI fingerprint from the PEM cert: SHA-256 of the DER-encoded SubjectPublicKeyInfo
        use sha2::Digest as _;
        use x509_cert::der::{DecodePem, Encode};
        use x509_cert::Certificate;

        let cert = Certificate::from_pem(cert_chain[0].as_bytes())
            .map_err(|e| anyhow::anyhow!("parsing self-signed cert: {e}"))?;
        let spki_der = cert
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|e| anyhow::anyhow!("encoding SPKI: {e}"))?;
        let fingerprint = hex::encode(sha2::Sha256::digest(&spki_der));

        let revoked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM revoked_self_signed_keys WHERE spki_fingerprint = $1)",
        )
        .bind(&fingerprint)
        .fetch_one(pool)
        .await
        .context("checking revoked_self_signed_keys")?;

        if revoked {
            bail!("{}", failure_message(FailureKind::SelfSignedKeyRevoked));
        }
    }

    // 7. Verify signature
    let digest =
        Sha256Digest::from_hex(sha256).map_err(|e| anyhow::anyhow!("invalid sha256 in DB: {e}"))?;

    let verifier = RsaPssVerifier::new();
    let signer = verifier
        .verify_with_namespace(&digest, &sig_bytes, &cert_chain, namespace)
        .map_err(|e| {
            let kind = map_verify_error(&e, &revoked_serials);
            anyhow::anyhow!("{}", failure_message(kind))
        })?;

    // 8. Check revoked CA-issued serials (after successful chain verification)
    if let Some(serial) = signer.cert_serial {
        #[allow(clippy::cast_possible_wrap)]
        let serial_i64 = serial as i64;
        if revoked_serials.contains(&serial_i64) {
            bail!("{}", failure_message(FailureKind::CertRevoked(serial_i64)));
        }
    }

    // 9. Update versions with signer kind
    let signer_kind = if signer.ca_verified {
        "publisher"
    } else {
        "self_signed"
    };

    sqlx::query("UPDATE versions SET signer = $1 WHERE id = $2")
        .bind(signer_kind)
        .bind(version_id)
        .execute(pool)
        .await
        .context("updating signer on versions")?;

    Ok(())
}

fn map_verify_error(e: &VerifyError, revoked_serials: &[i64]) -> FailureKind {
    match e {
        VerifyError::SignatureMismatch => FailureKind::SignatureMismatch,
        VerifyError::CertExpired(date) => FailureKind::CertExpired(date.clone()),
        VerifyError::CnMismatch { .. } => FailureKind::CnMismatch,
        VerifyError::InvalidCertChain(_)
        | VerifyError::Der(_)
        | VerifyError::CertNotYetValid(_) => FailureKind::ChainInvalid,
        VerifyError::Revoked { serial } => {
            #[allow(clippy::cast_possible_wrap)]
            let s = *serial as i64;
            if revoked_serials.contains(&s) {
                FailureKind::CertRevoked(s)
            } else {
                FailureKind::ChainInvalid
            }
        }
        VerifyError::SelfSignedKeyRevoked => FailureKind::SelfSignedKeyRevoked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_message_for_signature_mismatch() {
        let msg = failure_message(FailureKind::SignatureMismatch);
        assert!(msg.contains("skreg pack"));
    }

    #[test]
    fn failure_message_for_cert_expired() {
        let msg = failure_message(FailureKind::CertExpired("2025-01-01".to_string()));
        assert!(msg.contains("skreg certify"));
    }

    #[test]
    fn failure_message_for_cn_mismatch() {
        let msg = failure_message(FailureKind::CnMismatch);
        assert!(msg.contains("skreg certify"));
    }
}
