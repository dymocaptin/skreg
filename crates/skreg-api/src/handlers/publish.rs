//! POST /v1/publish — accept a .skill tarball, validate, store to S3, enqueue vetting.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use log::error;
use serde::Serialize;
use sha2::{Digest, Sha256};
use skreg_core::manifest::Manifest;
use skreg_pack::unpack::unpack_to_tempdir;

use crate::middleware::resolve_namespace;
use crate::router::{AppState, SharedState};

/// Response body for `POST /v1/publish`.
#[derive(Debug, Serialize)]
pub struct PublishResponse {
    /// ID of the created vetting job.
    pub job_id: String,
    /// Human-readable status message.
    pub message: String,
}

/// Build the S3 object key for a tarball.
#[must_use]
pub fn make_storage_path(ns: &str, name: &str, version: &str, sha256: &str) -> String {
    format!("{ns}/{name}/{version}/{sha256}.skill")
}

/// Compute sha256, unpack, parse and validate manifest ownership and integrity.
fn validate_manifest(body: &Bytes, ns_slug: &str) -> Result<(Manifest, String), StatusCode> {
    let sha256 = hex::encode(Sha256::digest(body));

    let tmp = unpack_to_tempdir(body).map_err(|e| {
        error!("unpack: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;
    let manifest_bytes = std::fs::read(tmp.path().join("manifest.json"))
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| {
        error!("manifest parse: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    if manifest.namespace.as_str() != ns_slug {
        return Err(StatusCode::FORBIDDEN);
    }
    if manifest.sha256.as_hex() != sha256 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    Ok((manifest, sha256))
}

/// Check version uniqueness, upload tarball to S3, persist package + version rows,
/// insert a vetting job, and notify the worker via `pg_notify`.
async fn persist_and_notify(
    state: &AppState,
    ns_id: uuid::Uuid,
    ns_slug: &str,
    manifest: &Manifest,
    sha256: &str,
    body: Bytes,
) -> Result<uuid::Uuid, StatusCode> {
    let existing = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM packages p
            JOIN versions v ON v.package_id = p.id
            WHERE p.namespace_id = $1 AND p.name = $2 AND v.version = $3
        )",
    )
    .bind(ns_id)
    .bind(manifest.name.as_str())
    .bind(manifest.version.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if existing {
        return Err(StatusCode::CONFLICT);
    }

    let storage_path = make_storage_path(
        ns_slug,
        manifest.name.as_str(),
        &manifest.version.to_string(),
        sha256,
    );
    state
        .s3
        .put_object()
        .bucket(&state.s3_bucket)
        .key(&storage_path)
        .body(aws_sdk_s3::primitives::ByteStream::from(body.to_vec()))
        .send()
        .await
        .map_err(|e| {
            error!("s3 upload: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    let pkg_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO packages (namespace_id, name, description)
         VALUES ($1, $2, $3)
         ON CONFLICT (namespace_id, name) DO UPDATE SET description = EXCLUDED.description
         RETURNING id",
    )
    .bind(ns_id)
    .bind(manifest.name.as_str())
    .bind(&manifest.description)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let version_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO versions (package_id, version, sha256, storage_path, sig_path, signer)
         VALUES ($1, $2, $3, $4, '', 'registry')
         RETURNING id",
    )
    .bind(pkg_id)
    .bind(manifest.version.to_string())
    .bind(sha256)
    .bind(&storage_path)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let job_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO vetting_jobs (version_id) VALUES ($1) RETURNING id",
    )
    .bind(version_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query("SELECT pg_notify('vetting_jobs', $1)")
        .bind(job_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!("notify: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(job_id)
}

/// Handle `POST /v1/publish` — validate a `.skill` tarball, upload to S3, and enqueue vetting.
///
/// # Errors
///
/// Returns `401` if the API key is missing or invalid, `403` if the namespace does not match,
/// `409` if the version already exists, `422` if the tarball is malformed,
/// `503` if S3 is unavailable, or `500` on a database error.
pub async fn publish_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<PublishResponse>), StatusCode> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let raw_key = crate::middleware::extract_bearer(auth).ok_or(StatusCode::UNAUTHORIZED)?;
    let (ns_id, ns_slug) = resolve_namespace(&state.pool, &raw_key).await?;

    let (manifest, sha256) = validate_manifest(&body, &ns_slug)?;

    let job_id = persist_and_notify(&state, ns_id, &ns_slug, &manifest, &sha256, body).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(PublishResponse {
            job_id: job_id.to_string(),
            message: format!("Vetting started for {}/{}", ns_slug, manifest.name),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_path_format() {
        let path = make_storage_path("acme", "my-skill", "1.0.0", "abc123");
        assert_eq!(path, "acme/my-skill/1.0.0/abc123.skill");
    }
}
