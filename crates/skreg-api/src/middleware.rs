//! Auth helpers: extract Bearer token, resolve namespace from DB.

use axum::http::StatusCode;
use log::error;
use sqlx::PgPool;

use crate::auth::hash_secret;

/// Extract the raw token from an `Authorization: Bearer <token>` header value.
pub fn extract_bearer(header: &str) -> Option<String> {
    let token = header.strip_prefix("Bearer ")?;
    if token.is_empty() { None } else { Some(token.to_owned()) }
}

/// Resolve a namespace slug from a raw API key.
///
/// Also updates `last_used_at`.
///
/// # Errors
///
/// Returns `UNAUTHORIZED` if the key is not found.
pub async fn resolve_namespace(
    pool: &PgPool,
    raw_key: &str,
) -> Result<(uuid::Uuid, String), StatusCode> {
    let key_hash = hash_secret(raw_key);

    let row = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "UPDATE api_keys SET last_used_at = now()
         WHERE key_hash = $1
         RETURNING namespace_id,
                   (SELECT slug FROM namespaces WHERE id = namespace_id)",
    )
    .bind(&key_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| { error!("db: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?
    .ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_token_valid() {
        let result = extract_bearer("Bearer skreg_abc123");
        assert_eq!(result, Some("skreg_abc123".to_owned()));
    }

    #[test]
    fn extract_bearer_token_missing_prefix() {
        assert_eq!(extract_bearer("skreg_abc123"), None);
    }

    #[test]
    fn extract_bearer_token_empty() {
        assert_eq!(extract_bearer("Bearer "), None);
    }
}
