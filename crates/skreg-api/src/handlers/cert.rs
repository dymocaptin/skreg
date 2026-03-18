//! POST /v1/namespaces/:ns/cert — issue a Publisher CA-signed leaf certificate.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use log::error;
use rcgen::{Certificate, CertificateParams, CertificateSigningRequest, KeyPair};
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

/// Parse a PKCS#10 CSR PEM and validate that its CN matches `expected_namespace`.
///
/// Returns the parsed [`CertificateSigningRequest`] on success.
///
/// # Errors
///
/// Returns `422 Unprocessable Entity` if the CSR cannot be parsed or the CN
/// does not match the authenticated namespace.
fn parse_and_validate_csr(
    csr_pem: &str,
    expected_namespace: &str,
) -> Result<CertificateSigningRequest, StatusCode> {
    let csr = CertificateSigningRequest::from_pem(csr_pem).map_err(|e| {
        error!("CSR parse error: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    // Extract CN from the CSR's distinguished name and verify it matches
    // the authenticated namespace.
    let cn = csr
        .params
        .distinguished_name
        .get(&rcgen::DnType::CommonName)
        .and_then(|v| {
            if let rcgen::DnValue::Utf8String(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            error!("CSR missing CommonName");
            StatusCode::UNPROCESSABLE_ENTITY
        })?;

    if cn != expected_namespace {
        error!("CSR CN {cn:?} does not match namespace {expected_namespace:?}");
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    Ok(csr)
}

/// Sign a CSR with the Publisher CA and return `(leaf_cert_pem, serial_i64)`.
///
/// Builds the CA [`Certificate`] from the CA key + cert PEM, then calls
/// [`CertificateSigningRequest::serialize_pem_with_signer`] so that the leaf
/// cert contains the *client's* public key — the server never generates or
/// sees the client private key.
///
/// # Errors
///
/// Returns an error if key parsing or signing fails.
fn sign_csr_with_ca(
    csr: &CertificateSigningRequest,
    pub_ca_key_pem: &str,
    pub_ca_cert_pem: &str,
) -> anyhow::Result<(String, i64)> {
    use anyhow::Context;

    let ca_key = KeyPair::from_pem(pub_ca_key_pem).context("parsing CA key PEM")?;
    let ca_params = CertificateParams::from_ca_cert_pem(pub_ca_cert_pem, ca_key)
        .context("parsing CA cert PEM")?;
    let ca_cert = Certificate::from_params(ca_params).context("building CA Certificate")?;

    let leaf_pem = csr
        .serialize_pem_with_signer(&ca_cert)
        .context("signing CSR with CA")?;

    // Generate a random positive serial using 7 bytes (stays in positive i64 range).
    let serial_bytes: [u8; 7] = rand::random();
    let mut serial_i64: i64 = 0;
    for b in serial_bytes {
        serial_i64 = (serial_i64 << 8) | i64::from(b);
    }

    Ok((leaf_pem, serial_i64))
}

/// Handle `POST /v1/namespaces/:ns/cert`.
///
/// Authenticates the caller via Bearer token, validates the PKCS#10 CSR
/// (checking CN matches the authenticated namespace), rate-limits to 5
/// issuances per namespace per 24 h, checks for an existing active cert,
/// signs the CSR with the Publisher CA (so the leaf cert contains the
/// *client's* public key), persists to `publisher_certs`, writes an audit
/// log entry, and returns the cert + CA PEM.
///
/// # Errors
///
/// - `401` — missing or invalid Bearer token
/// - `403` — token namespace does not match `:ns`
/// - `409` — an active certificate already exists for this namespace
/// - `413` — CSR body exceeds 16 KiB
/// - `422` — CSR is malformed or CN does not match namespace
/// - `429` — rate limit exceeded (5 issuances per 24 h)
/// - `500` — database error
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

    // Parse CSR body
    let csr_pem = std::str::from_utf8(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Validate CSR size
    validate_csr_size(csr_pem)?;

    // Parse and validate CSR — CN must match the authenticated namespace
    let csr = parse_and_validate_csr(csr_pem, &ns_slug)?;

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

    let ca_key_pem = state.publisher_ca_key_pem.clone();
    let pub_ca_cert_pem = state.publisher_ca_cert_pem.clone();

    // Sign the CSR with the CA — the leaf cert contains the client's public key
    let (leaf_pem, serial) =
        sign_csr_with_ca(&csr, &ca_key_pem, &pub_ca_cert_pem).map_err(|e| {
            error!("sign_csr_with_ca: {e}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::PKCS_RSA_SHA256;

    #[test]
    fn rejects_oversized_csr() {
        let huge_csr = "A".repeat(17_000);
        assert!(validate_csr_size(&huge_csr).is_err());
    }

    #[test]
    fn accepts_normal_csr() {
        assert!(validate_csr_size(&"A".repeat(1_000)).is_ok());
    }

    #[test]
    fn parse_and_validate_csr_rejects_cn_mismatch() {
        // Build a CSR with CN="acme" and check it rejects namespace "other"
        use rand::rngs::OsRng;
        use rsa::pkcs8::EncodePrivateKey;

        let rsa_key = rsa::RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pkcs8_pem = rsa_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string();

        let key_pair = KeyPair::from_pem(&pkcs8_pem).unwrap();
        let mut params = rcgen::CertificateParams::new(vec!["acme".to_owned()]);
        params.alg = &PKCS_RSA_SHA256;
        params.key_pair = Some(key_pair);
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "acme");
        params.distinguished_name = dn;
        let cert = rcgen::Certificate::from_params(params).unwrap();
        let csr_pem = cert.serialize_request_pem().unwrap();

        // Correct namespace succeeds
        assert!(parse_and_validate_csr(&csr_pem, "acme").is_ok());
        // Wrong namespace fails
        assert!(parse_and_validate_csr(&csr_pem, "other").is_err());
    }
}
