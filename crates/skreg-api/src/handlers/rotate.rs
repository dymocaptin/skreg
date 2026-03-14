//! POST /v1/namespaces/:ns/rotate-key  — submit a key-rotation request.
//! GET  /v1/namespaces/:ns/rotate-key/confirm — confirm via email token.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use der::DecodePem;
use log::error;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::middleware::{extract_bearer, resolve_namespace};
use crate::router::SharedState;

// ---------------------------------------------------------------------------
// Shared token shape (mirrors the CLI's RotationToken)
// ---------------------------------------------------------------------------

/// Rotation token submitted by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationToken {
    /// Authenticated namespace slug.
    pub namespace: String,
    /// SHA-256 SPKI fingerprint (hex) of the current key being replaced.
    pub old_key_fingerprint: String,
    /// SHA-256 SPKI fingerprint (hex) of the new key.
    pub new_key_fingerprint: String,
    /// PEM-encoded cert chain for the new key (leaf first).
    pub new_cert_chain_pem: Vec<String>,
    /// 32-byte random nonce (hex) — prevents replay.
    pub nonce: String,
    /// RFC 3339 timestamp when this token was created.
    pub issued_at: String,
    /// RFC 3339 timestamp when this token expires.
    pub expires_at: String,
}

/// Serialize `token` to the canonical JSON bytes used for signature verification.
fn canonical_json_bytes(token: &RotationToken) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_string(token).map(String::into_bytes)
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/namespaces/:ns/rotate-key`.
#[derive(Debug, Deserialize)]
pub struct RotateSubmitRequest {
    /// The rotation token containing key fingerprints and timestamps.
    pub token: RotationToken,
    /// Hex-encoded RSA-PSS signature over `sha256(canonical_json)` with the *old* key.
    pub old_sig: String,
    /// Hex-encoded RSA-PSS signature over `sha256(canonical_json)` with the *new* key.
    pub new_sig: String,
}

/// Response body for `POST /v1/namespaces/:ns/rotate-key`.
#[derive(Debug, Serialize)]
pub struct RotateSubmitResponse {
    /// Human-readable status message instructing the user to check their email.
    pub message: String,
}

/// Query parameters for `GET /v1/namespaces/:ns/rotate-key/confirm`.
#[derive(Debug, Deserialize)]
pub struct ConfirmQuery {
    /// The one-time confirmation token sent by email.
    pub token: String,
}

/// Response body for `GET /v1/namespaces/:ns/rotate-key/confirm`.
#[derive(Debug, Serialize)]
pub struct ConfirmResponse {
    /// Human-readable confirmation message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// PSS verification helper
// ---------------------------------------------------------------------------

/// Verify a hex-encoded RSA-PSS signature over a pre-hashed digest.
///
/// `cert_pem` — PEM certificate whose public key is used for verification.
/// `sig_hex`  — hex-encoded signature bytes.
/// `digest`   — raw 32-byte SHA-256 digest.
///
/// Returns `Ok(())` on success, `Err(StatusCode)` on any failure.
fn verify_pss_from_cert(cert_pem: &str, sig_hex: &str, digest: &[u8]) -> Result<(), StatusCode> {
    use rsa::pkcs8::DecodePublicKey;
    use rsa::pss::{Signature, VerifyingKey};
    use rsa::signature::hazmat::PrehashVerifier;

    // Parse the certificate and extract the DER-encoded SPKI.
    let cert = x509_cert::Certificate::from_pem(cert_pem).map_err(|e| {
        error!("parsing cert PEM for verification: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    let spki_der = {
        use der::Encode;
        cert.tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|e| {
                error!("DER-encoding SPKI: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

    let public_key = rsa::RsaPublicKey::from_public_key_der(&spki_der).map_err(|e| {
        error!("parsing RSA public key from SPKI: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    let verifying_key = VerifyingKey::<Sha256>::new(public_key);

    let sig_bytes = hex::decode(sig_hex).map_err(|e| {
        error!("decoding signature hex: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    let signature = Signature::try_from(sig_bytes.as_slice()).map_err(|e| {
        error!("parsing RSA-PSS signature: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    verifying_key
        .verify_prehash(digest, &signature)
        .map_err(|e| {
            error!("PSS signature verification failed: {e}");
            StatusCode::UNAUTHORIZED
        })
}

// ---------------------------------------------------------------------------
// Email helper
// ---------------------------------------------------------------------------

async fn send_rotation_email(
    state: &crate::router::AppState,
    ns_id: uuid::Uuid,
    confirm_token: &str,
    namespace: &str,
) -> Result<(), StatusCode> {
    let confirm_url = format!(
        "{}/v1/namespaces/{}/rotate-key/confirm?token={}",
        // Use the from_email domain as a rough base; in production this would
        // be a configured public API URL env var. For now derive from registry
        // env or fall back to a placeholder.
        std::env::var("PUBLIC_API_URL").unwrap_or_else(|_| "https://api.skreg.ai".to_owned()),
        namespace,
        confirm_token,
    );

    // Look up the email address associated with this namespace.
    let email = sqlx::query_scalar::<_, String>(
        "SELECT email FROM api_keys WHERE namespace_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(ns_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("db fetch email for rotation: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        error!("no api_key email found for namespace {ns_id}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    state
        .ses
        .send_email()
        .from_email_address(&state.from_email)
        .destination(
            aws_sdk_sesv2::types::Destination::builder()
                .to_addresses(&email)
                .build(),
        )
        .content(
            aws_sdk_sesv2::types::EmailContent::builder()
                .simple(
                    aws_sdk_sesv2::types::Message::builder()
                        .subject(
                            aws_sdk_sesv2::types::Content::builder()
                                .data("Confirm your skreg key rotation")
                                .build()
                                .map_err(|e| {
                                    error!("ses build error: {e}");
                                    StatusCode::INTERNAL_SERVER_ERROR
                                })?,
                        )
                        .body(
                            aws_sdk_sesv2::types::Body::builder()
                                .text(
                                    aws_sdk_sesv2::types::Content::builder()
                                        .data(format!(
                                            "A key rotation was requested for namespace \
                                             \"{namespace}\".\n\n\
                                             Confirm here (link valid for 24 h):\n{confirm_url}\n\n\
                                             If you did not request this, ignore this email."
                                        ))
                                        .build()
                                        .map_err(|e| {
                                            error!("ses build error: {e}");
                                            StatusCode::INTERNAL_SERVER_ERROR
                                        })?,
                                )
                                .build(),
                        )
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .map_err(|e| {
            error!("SES send error: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate token time bounds and namespace match.
fn validate_token(token: &RotationToken, ns_slug: &str) -> Result<(), StatusCode> {
    if token.namespace != ns_slug {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let issued_at = chrono::DateTime::parse_from_rfc3339(&token.issued_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(&token.expires_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let now = chrono::Utc::now();
    if now > expires_at {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if issued_at > now + chrono::Duration::minutes(2) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if expires_at - issued_at > chrono::Duration::minutes(5) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(())
}

/// Run all DB checks: rate limit, nonce, pinned key, old cert, signatures.
///
/// Returns `(old_cert_pem, new_cert_pem)` on success.
async fn validate_rotation_db(
    state: &crate::router::AppState,
    ns_id: uuid::Uuid,
    ns_slug: &str,
    body: &RotateSubmitRequest,
) -> Result<(String, String), StatusCode> {
    let token = &body.token;

    let pending_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pending_rotations
         WHERE namespace_id = $1 AND created_at > now() - INTERVAL '24 hours'",
    )
    .bind(ns_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db rate-limit check: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if pending_count >= 3 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let nonce_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM rotation_nonces WHERE nonce = $1)",
    )
    .bind(&token.nonce)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db nonce check: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if nonce_exists {
        return Err(StatusCode::CONFLICT);
    }

    let pinned: Option<String> =
        sqlx::query_scalar("SELECT pinned_publisher_key FROM namespaces WHERE id = $1")
            .bind(ns_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                error!("db pinned key check: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .flatten();
    if let Some(ref fp) = pinned {
        if fp != &token.old_key_fingerprint {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    let token_bytes = canonical_json_bytes(token).map_err(|e| {
        error!("canonical_json_bytes: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let digest: Vec<u8> = Sha256::digest(&token_bytes).to_vec();

    let old_cert_pem: String = sqlx::query_scalar(
        "SELECT pem FROM publisher_certs
         WHERE namespace_id = $1 AND revoked_at IS NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(ns_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("db fetch old cert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        error!("no active publisher cert for {ns_slug}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    verify_pss_from_cert(&old_cert_pem, &body.old_sig, &digest)?;

    let new_cert_pem = token.new_cert_chain_pem.first().ok_or_else(|| {
        error!("new_cert_chain_pem is empty");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;
    verify_pss_from_cert(new_cert_pem, &body.new_sig, &digest)?;

    Ok((old_cert_pem, new_cert_pem.clone()))
}

/// Persist the validated rotation: insert nonce, pending row, and audit log.
///
/// Returns the `confirm_token`.
async fn persist_rotation(
    state: &crate::router::AppState,
    ns_id: uuid::Uuid,
    ns_slug: &str,
    token: &RotationToken,
    new_cert_pem: &str,
) -> Result<String, StatusCode> {
    sqlx::query("INSERT INTO rotation_nonces (nonce) VALUES ($1)")
        .bind(&token.nonce)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!("db insert nonce: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let confirm_token = hex::encode(rand::random::<[u8; 32]>());
    let token_json = serde_json::to_value(token).map_err(|e| {
        error!("serializing token: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query(
        "INSERT INTO pending_rotations
             (namespace_id, confirm_token, token_json, new_key_fingerprint, new_cert_pem)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ns_id)
    .bind(&confirm_token)
    .bind(&token_json)
    .bind(&token.new_key_fingerprint)
    .bind(new_cert_pem)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db insert pending_rotation: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query(
        "INSERT INTO pki_audit_log (namespace_id, operation, outcome, detail)
         VALUES ($1, 'rotate_submit', 'success', $2)",
    )
    .bind(ns_id)
    .bind(serde_json::json!({ "namespace": ns_slug }))
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db audit log: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(confirm_token)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Handle `POST /v1/namespaces/:ns/rotate-key`.
///
/// Validates the rotation request (auth, time bounds, rate limit, nonce,
/// pinned key, PSS signatures), stores a `pending_rotations` row, and sends
/// a confirmation email.
///
/// # Errors
///
/// - `401` — missing/invalid Bearer token or bad signature
/// - `403` — token namespace does not match `:ns`
/// - `409` — nonce already used
/// - `422` — malformed token or cert
/// - `429` — rate limit exceeded
/// - `500` — database or SES error
pub async fn rotate_submit_handler(
    State(state): State<SharedState>,
    Path(ns): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RotateSubmitRequest>,
) -> Result<Json<RotateSubmitResponse>, StatusCode> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let raw_key = extract_bearer(auth).ok_or(StatusCode::UNAUTHORIZED)?;
    let (ns_id, ns_slug) = resolve_namespace(&state.pool, &raw_key).await?;

    if ns_slug != ns {
        return Err(StatusCode::FORBIDDEN);
    }

    validate_token(&body.token, &ns_slug)?;
    let (_old_cert, new_cert_pem) = validate_rotation_db(&state, ns_id, &ns_slug, &body).await?;
    let confirm_token =
        persist_rotation(&state, ns_id, &ns_slug, &body.token, &new_cert_pem).await?;

    if let Err(e) = send_rotation_email(&state, ns_id, &confirm_token, &ns_slug).await {
        error!("send_rotation_email failed: {e:?}");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    Ok(Json(RotateSubmitResponse {
        message: "Check your email to confirm the rotation.".to_owned(),
    }))
}

/// Handle `GET /v1/namespaces/:ns/rotate-key/confirm?token=...`.
///
/// Looks up the pending rotation by `confirm_token`, verifies it is not
/// expired and belongs to `:ns`, then in a transaction updates
/// `namespaces.pinned_publisher_key` and deletes the pending row.
///
/// # Errors
///
/// - `404` — confirm token not found or expired
/// - `403` — token belongs to a different namespace
/// - `500` — database error
pub async fn rotate_confirm_handler(
    State(state): State<SharedState>,
    Path(ns): Path<String>,
    Query(params): Query<ConfirmQuery>,
) -> Result<Json<ConfirmResponse>, StatusCode> {
    // Look up pending rotation
    let row = sqlx::query_as::<_, (uuid::Uuid, String, String, String)>(
        "SELECT pr.namespace_id, n.slug, pr.new_key_fingerprint, pr.new_cert_pem
         FROM pending_rotations pr
         JOIN namespaces n ON n.id = pr.namespace_id
         WHERE pr.confirm_token = $1
           AND pr.created_at > now() - INTERVAL '24 hours'",
    )
    .bind(&params.token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("db fetch pending_rotation: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let (ns_id, row_ns_slug, new_fp, new_cert_pem) = row;

    if row_ns_slug != ns {
        return Err(StatusCode::FORBIDDEN);
    }

    // Apply rotation in a transaction.
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("db begin tx: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query("UPDATE namespaces SET pinned_publisher_key = $1 WHERE id = $2")
        .bind(&new_fp)
        .bind(ns_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("db update pinned_publisher_key: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    sqlx::query("DELETE FROM pending_rotations WHERE confirm_token = $1")
        .bind(&params.token)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("db delete pending_rotation: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Insert new cert into publisher_certs so it can be used for future verification.
    let expires_at = chrono::Utc::now() + chrono::Duration::days(90);
    let serial_bytes: [u8; 7] = rand::random();
    let mut serial_i64: i64 = 0;
    for b in serial_bytes {
        serial_i64 = (serial_i64 << 8) | i64::from(b);
    }

    sqlx::query(
        "INSERT INTO publisher_certs (namespace_id, serial, pem, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(ns_id)
    .bind(serial_i64)
    .bind(&new_cert_pem)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("db insert new publisher_cert: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("db commit rotation: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Audit log (after commit — best-effort).
    let _ = sqlx::query(
        "INSERT INTO pki_audit_log (namespace_id, operation, outcome, detail)
         VALUES ($1, 'rotate_confirm', 'success', $2)",
    )
    .bind(ns_id)
    .bind(serde_json::json!({ "namespace": ns, "new_fp": new_fp }))
    .execute(&state.pool)
    .await;

    Ok(Json(ConfirmResponse {
        message: "Key rotation confirmed. Your new key is now active.".to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_is_deterministic() {
        let t = RotationToken {
            namespace: "acme".into(),
            old_key_fingerprint: "aaa".into(),
            new_key_fingerprint: "bbb".into(),
            new_cert_chain_pem: vec!["cert".into()],
            nonce: "nnn".into(),
            issued_at: "2026-01-01T00:00:00Z".into(),
            expires_at: "2026-01-01T00:05:00Z".into(),
        };
        let b1 = canonical_json_bytes(&t).unwrap();
        let b2 = canonical_json_bytes(&t.clone()).unwrap();
        assert_eq!(b1, b2);
    }
}
