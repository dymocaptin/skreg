//! GET /v1/packages/:ns/:name/:version — package metadata
//! GET /v1/download/:ns/:name/:version — tarball download
//! GET /v1/download/:ns/:name/:version/sig — signature download

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde::Serialize;
use skreg_core::types::{Namespace, PackageName};

use crate::router::{AppState, SharedState};

/// A row from the `versions` + `packages` join used to resolve a version.
#[derive(sqlx::FromRow)]
pub(crate) struct VersionRow {
    pub(crate) version: String,
    pub(crate) sha256: String,
    pub(crate) storage_path: String,
    pub(crate) sig_path: String,
    pub(crate) description: String,
    pub(crate) category: Option<String>,
}

/// Response body for the package metadata endpoint.
/// Field names must match `skreg_core::manifest::Manifest` exactly.
#[derive(Debug, Serialize)]
pub struct ManifestResponse {
    /// Publisher namespace slug.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Package version string.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Optional category tag.
    pub category: Option<String>,
    /// SHA-256 hex digest of the tarball.
    pub sha256: String,
    /// PEM-encoded certificate chain. Empty for registry-signed packages.
    pub cert_chain_pem: Vec<String>,
}

/// Validate a version segment: "latest" or alphanumeric + `.`, `-`, `+`, max 32 chars.
pub(crate) fn validate_version(v: &str) -> bool {
    if v == "latest" {
        return true;
    }
    if v.is_empty() || v.len() > 32 {
        return false;
    }
    v.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
}

/// Resolve a version row from the DB given validated namespace, name, and version.
/// If version is "latest", returns the most recently published version.
pub(crate) async fn resolve_version_row(
    state: &AppState,
    ns: &str,
    name: &str,
    version: &str,
) -> Result<VersionRow, StatusCode> {
    let row = if version == "latest" {
        sqlx::query_as::<_, VersionRow>(
            "SELECT v.version, v.sha256, v.storage_path, v.sig_path,
                    p.description, p.category
             FROM versions v
             JOIN packages p ON p.id = v.package_id
             JOIN namespaces n ON n.id = p.namespace_id
             WHERE n.slug = $1
               AND p.name = $2
               AND v.yanked_at IS NULL
               AND n.banned_at IS NULL
             ORDER BY v.published_at DESC, v.id DESC
             LIMIT 1",
        )
        .bind(ns)
        .bind(name)
        .fetch_optional(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, VersionRow>(
            "SELECT v.version, v.sha256, v.storage_path, v.sig_path,
                    p.description, p.category
             FROM versions v
             JOIN packages p ON p.id = v.package_id
             JOIN namespaces n ON n.id = p.namespace_id
             WHERE n.slug = $1
               AND p.name = $2
               AND v.version = $3
               AND v.yanked_at IS NULL
               AND n.banned_at IS NULL",
        )
        .bind(ns)
        .bind(name)
        .bind(version)
        .fetch_optional(&state.pool)
        .await
    };

    match row {
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("db query error: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle `GET /v1/packages/:ns/:name/:version` — return package manifest JSON.
///
/// # Errors
///
/// Returns `400` for invalid namespace, name, or version. Returns `404` if the
/// package or version does not exist. Returns `500` on database error.
pub async fn package_meta_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw, version_raw)): Path<(String, String, String)>,
) -> Result<Json<ManifestResponse>, StatusCode> {
    let ns = Namespace::new(&ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(&name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !validate_version(&version_raw) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = resolve_version_row(&state, ns.as_str(), pkg_name.as_str(), &version_raw).await?;

    Ok(Json(ManifestResponse {
        namespace: ns.as_str().to_owned(),
        name: pkg_name.as_str().to_owned(),
        version: row.version,
        description: row.description,
        category: row.category,
        sha256: row.sha256,
        cert_chain_pem: vec![],
    }))
}

/// Handle `GET /v1/download/:ns/:name/:version` — return tarball bytes.
///
/// # Errors
///
/// Returns `400` for invalid namespace, name, or version. Returns `404` if the
/// package or version does not exist. Returns `503` on S3 error.
pub async fn package_download_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw, version_raw)): Path<(String, String, String)>,
) -> Result<Bytes, StatusCode> {
    let ns = Namespace::new(&ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(&name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !validate_version(&version_raw) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = resolve_version_row(&state, ns.as_str(), pkg_name.as_str(), &version_raw).await?;

    let obj = state
        .s3
        .get_object()
        .bucket(&state.s3_bucket)
        .key(&row.storage_path)
        .send()
        .await
        .map_err(|e| {
            error!("s3 get_object error: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    let data = obj.body.collect().await.map_err(|e| {
        error!("s3 body collect error: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    Ok(data.into_bytes())
}

/// Handle `GET /v1/download/:ns/:name/:version/sig` — return signature bytes.
///
/// # Errors
///
/// Returns `400` for invalid namespace, name, or version. Returns `404` if the
/// package or version does not exist. Returns `503` on S3 error.
pub async fn package_sig_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw, version_raw)): Path<(String, String, String)>,
) -> Result<Bytes, StatusCode> {
    let ns = Namespace::new(&ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(&name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !validate_version(&version_raw) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = resolve_version_row(&state, ns.as_str(), pkg_name.as_str(), &version_raw).await?;

    let obj = state
        .s3
        .get_object()
        .bucket(&state.s3_bucket)
        .key(&row.sig_path)
        .send()
        .await
        .map_err(|e| {
            error!("s3 get_object (sig) error: {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    let data = obj.body.collect().await.map_err(|e| {
        error!("s3 body collect (sig) error: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    Ok(data.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::validate_version;

    #[test]
    fn validate_version_accepts_latest() {
        assert!(validate_version("latest"));
    }

    #[test]
    fn validate_version_accepts_semver() {
        assert!(validate_version("1.2.3"));
        assert!(validate_version("1.0.0-alpha.1"));
        assert!(validate_version("2.0.0+build.1"));
    }

    #[test]
    fn validate_version_rejects_empty() {
        assert!(!validate_version(""));
    }

    #[test]
    fn validate_version_rejects_too_long() {
        assert!(!validate_version(&"1".repeat(33)));
    }

    #[test]
    fn validate_version_rejects_path_traversal() {
        assert!(!validate_version("../etc/passwd"));
        assert!(!validate_version("1.0/bad"));
    }

    #[test]
    fn validate_version_rejects_special_chars() {
        assert!(!validate_version("1.0.0 beta"));
        assert!(!validate_version("1.0.0@tag"));
    }
}
