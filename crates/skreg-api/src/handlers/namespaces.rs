//! POST /v1/namespaces — register a namespace and receive an API key.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde::{Deserialize, Serialize};

use crate::auth::{generate_api_key, hash_secret};
use crate::router::SharedState;

/// Request body for `POST /v1/namespaces`.
#[derive(Debug, Deserialize)]
pub struct CreateNamespaceRequest {
    /// Desired namespace slug.
    pub slug: String,
    /// Contact email address for this namespace.
    pub email: String,
}

/// Response body for `POST /v1/namespaces`.
#[derive(Debug, Serialize)]
pub struct CreateNamespaceResponse {
    /// Plaintext API key — shown once, never stored.
    pub api_key: String,
    /// The registered namespace slug.
    pub namespace: String,
}

/// Validate namespace slug: lowercase alphanumeric + hyphens, 3–32 chars.
#[must_use]
pub fn is_valid_slug(slug: &str) -> bool {
    let len = slug.len();
    (3..=32).contains(&len)
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Handle `POST /v1/namespaces` — register a new namespace and return a plaintext API key.
///
/// # Errors
///
/// Returns `422` if the slug is invalid, `409` if the slug is already taken,
/// or `500` on a database error.
pub async fn create_namespace_handler(
    State(state): State<SharedState>,
    Json(body): Json<CreateNamespaceRequest>,
) -> Result<Json<CreateNamespaceResponse>, StatusCode> {
    if !is_valid_slug(&body.slug) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let pool = &state.pool;

    // Insert namespace (409 if slug taken)
    let ns_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO namespaces (slug, kind) VALUES ($1, 'individual')
         ON CONFLICT (slug) DO NOTHING
         RETURNING id",
    )
    .bind(&body.slug)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("db error creating namespace: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::CONFLICT)?;

    let api_key = generate_api_key();
    let key_hash = hash_secret(&api_key);

    sqlx::query("INSERT INTO api_keys (namespace_id, key_hash, email) VALUES ($1, $2, $3)")
        .bind(ns_id)
        .bind(&key_hash)
        .bind(&body.email)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("db error creating api key: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(CreateNamespaceResponse {
        api_key,
        namespace: body.slug,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn create_namespace_request_deserialises() {
        let body = json!({ "slug": "acme", "email": "dev@acme.com" });
        let req: super::CreateNamespaceRequest = serde_json::from_value(body).unwrap();
        assert_eq!(req.slug, "acme");
        assert_eq!(req.email, "dev@acme.com");
    }

    #[test]
    fn invalid_slug_rejected() {
        assert!(!super::is_valid_slug("AB")); // uppercase
        assert!(!super::is_valid_slug("ab")); // too short
        assert!(!super::is_valid_slug(&"a".repeat(33))); // too long
        assert!(super::is_valid_slug("acme-corp")); // valid
        assert!(super::is_valid_slug("abc")); // min length
    }
}
