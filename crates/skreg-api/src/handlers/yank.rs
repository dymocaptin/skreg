//! POST /v1/packages/:ns/:name/yank — yank all versions of a package
//! POST /v1/packages/:ns/:name/:version/yank — yank a single version
//!
//! "Yank" is a soft, reversible removal: it sets `versions.yanked_at = now()`.
//! Yanked versions are filtered out of every read path (metadata, download,
//! search), so they become uninstallable while the artifact stays in S3.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use log::error;
use serde::Serialize;
use skreg_core::types::{Namespace, PackageName};

use crate::handlers::packages::validate_version;
use crate::middleware::{extract_bearer, resolve_namespace};
use crate::router::{AppState, SharedState};

/// Response body for the yank endpoints.
#[derive(Debug, Serialize)]
pub struct YankResponse {
    /// Number of versions newly yanked by this call (0 if already yanked).
    pub yanked: i64,
}

/// Handle `POST /v1/packages/:ns/:name/yank` — yank all non-yanked versions.
///
/// # Errors
///
/// `400` invalid namespace/name, `401` missing/invalid key, `403` key namespace
/// mismatch, `404` package not found, `500` on DB error.
pub async fn yank_all_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    do_yank(&state, &ns_raw, &name_raw, None, &headers).await
}

/// Handle `POST /v1/packages/:ns/:name/:version/yank` — yank one version.
///
/// # Errors
///
/// `400` invalid namespace/name/version, `401` missing/invalid key, `403` key
/// namespace mismatch, `404` package/version not found, `500` on DB error.
pub async fn yank_version_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw, version_raw)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    do_yank(&state, &ns_raw, &name_raw, Some(&version_raw), &headers).await
}

/// Shared yank logic. `version = None` yanks all versions of the package.
async fn do_yank(
    state: &AppState,
    ns_raw: &str,
    name_raw: &str,
    version: Option<&str>,
    headers: &HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    // 1. Validate path params first (cheap, no DB).
    let ns = Namespace::new(ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    if let Some(v) = version {
        if !validate_version(v) || v == "latest" {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // 2. Authenticate.
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let raw_key = extract_bearer(auth).ok_or(StatusCode::UNAUTHORIZED)?;
    let (ns_id, ns_slug) = resolve_namespace(&state.pool, &raw_key).await?;

    // 3. Ownership: key must belong to the target namespace.
    if ns_slug != ns.as_str() {
        return Err(StatusCode::FORBIDDEN);
    }

    // 4. DB step.
    let yanked = yank_versions(state, ns_id, pkg_name.as_str(), version).await?;
    Ok(Json(YankResponse { yanked }))
}

/// Set `yanked_at` on the target version(s); returns the number of versions
/// newly yanked (already-yanked versions are not recounted). Idempotent.
async fn yank_versions(
    state: &AppState,
    ns_id: uuid::Uuid,
    name: &str,
    version: Option<&str>,
) -> Result<i64, StatusCode> {
    // Resolve the package id within the (already ownership-checked) namespace.
    let pkg_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM packages WHERE namespace_id = $1 AND name = $2")
            .bind(ns_id)
            .bind(name)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                error!("db: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    let pkg_id = pkg_id.ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = version {
        // 404 if the version row does not exist at all.
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM versions WHERE package_id = $1 AND version = $2)",
        )
        .bind(pkg_id)
        .bind(v)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if !exists {
            return Err(StatusCode::NOT_FOUND);
        }

        let affected = sqlx::query(
            "UPDATE versions SET yanked_at = now()
             WHERE package_id = $1 AND version = $2 AND yanked_at IS NULL",
        )
        .bind(pkg_id)
        .bind(v)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .rows_affected();
        return Ok(i64::try_from(affected).unwrap_or(i64::MAX));
    }

    // 404 if the package has no versions at all.
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM versions WHERE package_id = $1")
        .bind(pkg_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if total == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    let affected = sqlx::query(
        "UPDATE versions SET yanked_at = now()
         WHERE package_id = $1 AND yanked_at IS NULL",
    )
    .bind(pkg_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .rows_affected();
    Ok(i64::try_from(affected).unwrap_or(i64::MAX))
}
