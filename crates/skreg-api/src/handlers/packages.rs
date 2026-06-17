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

/// Re-export of the shared version-segment validator.
pub(crate) use skreg_core::version::is_valid_segment as validate_version;

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
             JOIN vetting_jobs j ON j.version_id = v.id
             WHERE n.slug = $1
               AND p.name = $2
               AND j.status = 'pass'
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

/// A published version row, most-recent-first, used by the versions and diff endpoints.
#[derive(sqlx::FromRow)]
pub(crate) struct PublishedVersion {
    pub(crate) version: String,
    pub(crate) published_at: chrono::DateTime<chrono::Utc>,
    pub(crate) sha256: String,
    /// Storage path consumed by the diff endpoint (Task 3).
    #[allow(dead_code)]
    pub(crate) storage_path: String,
}

/// List all published (passing, non-yanked, non-banned) versions of a package,
/// most recent first.
///
/// # Errors
///
/// Returns `404` when the package has no published versions, `500` on DB error.
pub(crate) async fn list_published_versions(
    state: &AppState,
    ns: &str,
    name: &str,
) -> Result<Vec<PublishedVersion>, StatusCode> {
    let rows = sqlx::query_as::<_, PublishedVersion>(
        "SELECT v.version, v.published_at, v.sha256, v.storage_path
         FROM versions v
         JOIN packages p ON p.id = v.package_id
         JOIN namespaces n ON n.id = p.namespace_id
         WHERE n.slug = $1
           AND p.name = $2
           AND v.yanked_at IS NULL
           AND n.banned_at IS NULL
           AND EXISTS (SELECT 1 FROM vetting_jobs j WHERE j.version_id = v.id AND j.status = 'pass')
         ORDER BY v.published_at DESC, v.id DESC",
    )
    .bind(ns)
    .bind(name)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("db query error (versions): {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if rows.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(rows)
}

/// A single entry in the public versions listing.
#[derive(Debug, Serialize)]
pub struct VersionEntry {
    /// Version string.
    pub version: String,
    /// RFC 3339 publish timestamp.
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// SHA-256 hex digest of the tarball.
    pub sha256: String,
}

/// Response body for the versions listing endpoint.
#[derive(Debug, Serialize)]
pub struct VersionsResponse {
    /// Published versions, most recent first.
    pub versions: Vec<VersionEntry>,
}

/// Handle `GET /v1/packages/:ns/:name/versions`.
///
/// # Errors
///
/// Returns `400` for invalid namespace/name, `404` if the package has no
/// published versions, `500` on DB error.
pub async fn package_versions_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw)): Path<(String, String)>,
) -> Result<Json<VersionsResponse>, StatusCode> {
    let ns = Namespace::new(&ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(&name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;

    let rows = list_published_versions(&state, ns.as_str(), pkg_name.as_str()).await?;
    let versions = rows
        .into_iter()
        .map(|r| VersionEntry {
            version: r.version,
            published_at: r.published_at,
            sha256: r.sha256,
        })
        .collect();
    Ok(Json(VersionsResponse { versions }))
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
