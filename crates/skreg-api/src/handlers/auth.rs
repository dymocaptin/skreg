//! POST /v1/auth/login and POST /v1/auth/token — email-OTP re-authentication.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use log::error;
use serde::{Deserialize, Serialize};

use crate::auth::{generate_api_key, generate_otp, hash_secret};
use crate::router::SharedState;

/// Request body for `POST /v1/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Namespace slug to authenticate for.
    pub namespace: String,
    /// Email address registered with this namespace.
    pub email: String,
}

/// Request body for `POST /v1/auth/token`.
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    /// Namespace slug to exchange the OTP for.
    pub namespace: String,
    /// 6-digit one-time password received by email.
    pub otp: String,
}

/// Response body for `POST /v1/auth/token`.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    /// New plaintext API key — shown once, never stored.
    pub api_key: String,
    /// The authenticated namespace slug.
    pub namespace: String,
}

/// Handle `POST /v1/auth/login` — send an OTP to the registered email.
///
/// # Errors
///
/// Returns `404` if the namespace is not found, `403` if the email is not registered,
/// `503` if the SES send fails, or `500` on a database error.
pub async fn login_handler(
    State(state): State<SharedState>,
    Json(body): Json<LoginRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = &state.pool;

    // Find namespace by slug
    let ns_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM namespaces WHERE slug = $1 AND banned_at IS NULL",
    )
    .bind(&body.namespace)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("db error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Verify email matches a key on this namespace
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM api_keys WHERE namespace_id = $1 AND email = $2)",
    )
    .bind(ns_id)
    .bind(&body.email)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("db error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !exists {
        return Err(StatusCode::FORBIDDEN);
    }

    let otp = generate_otp();
    let otp_hash = hash_secret(&otp);
    let expires_at = Utc::now() + Duration::minutes(10);

    sqlx::query("INSERT INTO otps (namespace_id, code_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(ns_id)
        .bind(&otp_hash)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("db error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Send via SES
    state.ses
        .send_email()
        .from_email_address(&state.from_email)
        .destination(
            aws_sdk_sesv2::types::Destination::builder()
                .to_addresses(&body.email)
                .build(),
        )
        .content(
            aws_sdk_sesv2::types::EmailContent::builder()
                .simple(
                    aws_sdk_sesv2::types::Message::builder()
                        .subject(
                            aws_sdk_sesv2::types::Content::builder()
                                .data("Your skreg login code")
                                .build()
                                .map_err(|e| { error!("ses build error: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?,
                        )
                        .body(
                            aws_sdk_sesv2::types::Body::builder()
                                .text(
                                    aws_sdk_sesv2::types::Content::builder()
                                        .data(format!("Your skreg one-time code is: {otp}\n\nExpires in 10 minutes."))
                                        .build()
                                        .map_err(|e| { error!("ses build error: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?,
                                )
                                .build(),
                        )
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .map_err(|e| { error!("SES error: {e}"); StatusCode::SERVICE_UNAVAILABLE })?;

    Ok(StatusCode::ACCEPTED)
}

/// Handle `POST /v1/auth/token` — exchange an OTP for a new API key.
///
/// # Errors
///
/// Returns `404` if the namespace is not found, `401` if the OTP is invalid or expired,
/// or `500` on a database error.
pub async fn token_handler(
    State(state): State<SharedState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, StatusCode> {
    let pool = &state.pool;

    let ns_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM namespaces WHERE slug = $1 AND banned_at IS NULL",
    )
    .bind(&body.namespace)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let otp_hash = hash_secret(&body.otp);

    // Consume OTP (mark used, verify not expired)
    let rows = sqlx::query(
        "UPDATE otps SET used_at = now()
         WHERE namespace_id = $1
           AND code_hash     = $2
           AND expires_at    > now()
           AND used_at IS NULL
         RETURNING id",
    )
    .bind(ns_id)
    .bind(&otp_hash)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if rows.rows_affected() == 0 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Revoke old keys and issue a new one
    let email = sqlx::query_scalar::<_, String>(
        "SELECT email FROM api_keys WHERE namespace_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(ns_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query("DELETE FROM api_keys WHERE namespace_id = $1")
        .bind(ns_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let api_key = generate_api_key();
    let key_hash = hash_secret(&api_key);

    sqlx::query("INSERT INTO api_keys (namespace_id, key_hash, email) VALUES ($1, $2, $3)")
        .bind(ns_id)
        .bind(&key_hash)
        .bind(&email)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(TokenResponse {
        api_key,
        namespace: body.namespace,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_request_deserialises() {
        let body = serde_json::json!({"namespace": "acme", "email": "dev@acme.com"});
        let r: LoginRequest = serde_json::from_value(body).unwrap();
        assert_eq!(r.namespace, "acme");
    }

    #[test]
    fn token_request_deserialises() {
        let body = serde_json::json!({"namespace": "acme", "otp": "123456"});
        let r: TokenRequest = serde_json::from_value(body).unwrap();
        assert_eq!(r.otp, "123456");
    }
}
