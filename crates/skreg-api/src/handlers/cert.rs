//! POST /v1/namespaces/:ns/cert — issue a Publisher CA-signed leaf certificate.

use aws_sdk_secretsmanager::Client as SmClient;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use log::error;
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_RSA_SHA256};
use rsa::pkcs8::EncodePrivateKey;
use serde::Serialize;

use crate::middleware::{extract_bearer, resolve_namespace};
use crate::router::SharedState;

/// Maximum allowed CSR size in bytes (16 KiB).
pub(crate) const MAX_CSR_BYTES: usize = 16 * 1024;

/// Response body for `POST /v1/namespaces/:ns/cert`.
#[derive(Debug, Serialize)]
pub struct CertResponse {
    /// PEM-encoded leaf certificate signed by the Publisher CA.
    pub cert: String,
    /// PEM-encoded Publisher CA certificate.
    pub ca_cert: String,
}

/// Validate that a CSR PEM string does not exceed [`MAX_CSR_BYTES`].
///
/// # Errors
///
/// Returns `413 Payload Too Large` if the CSR exceeds the limit.
pub(crate) fn validate_csr_size(csr_pem: &str) -> Result<(), StatusCode> {
    if csr_pem.len() > MAX_CSR_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    Ok(())
}

/// Issue a leaf certificate from the Publisher CA for the given namespace.
///
/// Generates a fresh 2048-bit RSA leaf key, creates a cert with CN set to
/// `namespace` signed by the Publisher CA, and returns (`leaf_cert_pem`, serial).
///
/// # Errors
///
/// Returns an error if key generation or signing fails.
fn issue_leaf_cert(
    namespace: &str,
    pub_ca_key_pem: &str,
    pub_ca_cert_pem: &str,
) -> anyhow::Result<(String, i64)> {
    use anyhow::Context;

    // Parse the CA key
    let ca_key = KeyPair::from_pem(pub_ca_key_pem).context("parsing CA key PEM")?;

    // Build CA cert params from the existing CA certificate
    let ca_params = CertificateParams::from_ca_cert_pem(pub_ca_cert_pem, ca_key)
        .context("parsing CA cert PEM")?;
    let ca_cert = Certificate::from_params(ca_params).context("building CA Certificate")?;

    // Generate a fresh 2048-bit RSA leaf key using the `rsa` crate,
    // then load it into rcgen via PKCS#8 PEM (ring requires PKCS#8 for RSA).
    let mut rng = rand::thread_rng();
    let rsa_key = rsa::RsaPrivateKey::new(&mut rng, 2048).context("generating 2048-bit RSA key")?;
    let pkcs8_pem = rsa_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .context("encoding RSA key to PKCS#8 PEM")?;
    let leaf_key = KeyPair::from_pem(pkcs8_pem.as_str()).context("loading leaf key into rcgen")?;

    // Build leaf cert params
    let mut leaf_params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, namespace);
    leaf_params.distinguished_name = dn;
    leaf_params.alg = &PKCS_RSA_SHA256;
    leaf_params.key_pair = Some(leaf_key);

    // Generate a random positive serial using 7 bytes (stays in positive i64 range)
    let serial_bytes: [u8; 7] = rand::random();
    let mut serial_i64: i64 = 0;
    for b in serial_bytes {
        serial_i64 = (serial_i64 << 8) | i64::from(b);
    }
    leaf_params.serial_number = Some(rcgen::SerialNumber::from_slice(&serial_bytes));

    let leaf_cert = Certificate::from_params(leaf_params).context("generating leaf certificate")?;
    let leaf_pem = leaf_cert
        .serialize_pem_with_signer(&ca_cert)
        .context("signing leaf certificate with CA")?;

    Ok((leaf_pem, serial_i64))
}

/// Handle `POST /v1/namespaces/:ns/cert`.
///
/// Authenticates the caller via Bearer token, rate-limits to 5 issuances per
/// namespace per 24 h, checks for an existing active cert, fetches the
/// Publisher CA key from Secrets Manager, issues a leaf cert, persists it to
/// `publisher_certs`, writes an audit log entry, and returns the cert + CA PEM.
///
/// # Errors
///
/// - `401` — missing or invalid Bearer token
/// - `403` — token namespace does not match `:ns`
/// - `409` — an active certificate already exists for this namespace
/// - `413` — CSR body exceeds 16 KiB
/// - `422` — cert issuance failed
/// - `429` — rate limit exceeded (5 issuances per 24 h)
/// - `500` — database or Secrets Manager error
pub async fn cert_handler(
    State(state): State<SharedState>,
    Path(ns): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<CertResponse>, StatusCode> {
    // Auth
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let raw_key = extract_bearer(auth).ok_or(StatusCode::UNAUTHORIZED)?;
    let (ns_id, ns_slug) = resolve_namespace(&state.pool, &raw_key).await?;

    // Namespace ownership check
    if ns_slug != ns {
        return Err(StatusCode::FORBIDDEN);
    }

    // Parse CSR body (body is the raw CSR PEM)
    let csr_pem = std::str::from_utf8(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Validate CSR size
    validate_csr_size(csr_pem)?;

    // Rate limit: max 5 issuances per namespace per 24h
    let issuances_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM publisher_certs
         WHERE namespace_id = $1 AND issued_at > now() - INTERVAL '24 hours'",
    )
    .bind(ns_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db rate-limit check: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if issuances_24h >= 5 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Check for existing active cert (not revoked, not expired)
    let has_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM publisher_certs
            WHERE namespace_id = $1
              AND revoked_at IS NULL
              AND expires_at > now()
        )",
    )
    .bind(ns_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db active-cert check: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if has_active {
        return Err(StatusCode::CONFLICT);
    }

    // Fetch Publisher CA key from Secrets Manager
    let ca_key_pem = fetch_secret(&state.sm, &state.publisher_ca_key_secret_name).await?;
    let pub_ca_cert_pem = state.publisher_ca_cert_pem.clone();

    // Issue leaf cert
    let (leaf_pem, serial) =
        issue_leaf_cert(&ns_slug, &ca_key_pem, &pub_ca_cert_pem).map_err(|e| {
            error!("issue_leaf_cert: {e}");
            StatusCode::UNPROCESSABLE_ENTITY
        })?;

    // Compute expiry (90 days from now)
    let expires_at = chrono::Utc::now() + chrono::Duration::days(90);

    // Persist to publisher_certs
    sqlx::query(
        "INSERT INTO publisher_certs (namespace_id, serial, pem, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(ns_id)
    .bind(serial)
    .bind(&leaf_pem)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db insert publisher_cert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Write audit log
    sqlx::query(
        "INSERT INTO pki_audit_log (namespace_id, operation, outcome, detail)
         VALUES ($1, 'cert_issue', 'success', $2)",
    )
    .bind(ns_id)
    .bind(serde_json::json!({ "serial": serial, "namespace": ns_slug }))
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db audit log: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(CertResponse {
        cert: leaf_pem,
        ca_cert: pub_ca_cert_pem,
    }))
}

/// Fetch a secret string from AWS Secrets Manager.
async fn fetch_secret(sm: &SmClient, secret_name: &str) -> Result<String, StatusCode> {
    let resp = sm
        .get_secret_value()
        .secret_id(secret_name)
        .send()
        .await
        .map_err(|e| {
            error!("secrets manager: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    resp.secret_string()
        .map(std::borrow::ToOwned::to_owned)
        .ok_or_else(|| {
            error!("secrets manager: secret has no string value");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_csr() {
        let huge_csr = "A".repeat(17_000);
        assert!(validate_csr_size(&huge_csr).is_err());
    }

    #[test]
    fn accepts_normal_csr() {
        assert!(validate_csr_size(&"A".repeat(1_000)).is_ok());
    }
}
