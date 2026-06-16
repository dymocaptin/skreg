//! POST /v1/packages/:ns/:name/yank — yank all versions of a package
//! POST /v1/packages/:ns/:name/:version/yank — yank a single version
//!
//! "Yank" is a soft, reversible removal: it sets `versions.yanked_at = now()`.
//! Yanked versions are filtered out of every read path (metadata, download,
//! search), so they become uninstallable while the artifact stays in S3.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
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

    // 4. DB step — implemented in a later task.
    let yanked = yank_versions(state, ns_id, pkg_name.as_str(), version).await?;
    Ok(Json(YankResponse { yanked }))
}

/// Set `yanked_at` on the target version(s); returns count newly yanked.
/// Stubbed in this task, implemented in the next task.
#[allow(clippy::unused_async)]
async fn yank_versions(
    _state: &AppState,
    _ns_id: uuid::Uuid,
    _name: &str,
    _version: Option<&str>,
) -> Result<i64, StatusCode> {
    Ok(0)
}
