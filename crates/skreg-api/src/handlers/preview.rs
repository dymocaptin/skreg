//! GET /v1/packages/:ns/:name/:version/preview — package preview without download

use std::io;
use std::path::Path;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde::Serialize;
use skreg_core::types::{Namespace, PackageName};

use crate::handlers::packages::{resolve_version_row, validate_version};
use crate::router::SharedState;

/// Maximum SKILL.md bytes returned by the preview endpoint.
const SKILL_MD_MAX: usize = 16 * 1024;

/// Response body for the preview endpoint.
#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    /// All file paths in the package, relative to the package root, sorted.
    pub files: Vec<String>,
    /// Content of `SKILL.md`, capped at `SKILL_MD_MAX` bytes.
    pub skill_md: String,
    /// True when `SKILL.md` was cut at the byte limit.
    pub truncated: bool,
}

/// Recursively collect relative file paths from `dir` (rooted at `base`) into `files`.
///
/// Entries within each directory are sorted by name for deterministic output.
pub(crate) fn collect_files(base: &Path, dir: &Path, files: &mut Vec<String>) -> io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        // Defensive: strip_prefix should always succeed since paths come from read_dir(dir),
        // but skip any entry that cannot be relativized rather than failing the whole walk.
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        if path.is_dir() {
            collect_files(base, &path, files)?;
        } else {
            files.push(rel.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

/// Handle `GET /v1/packages/:ns/:name/:version/preview`.
///
/// Fetches the `.skill` tarball from S3, unpacks it in memory, and returns
/// the file listing and SKILL.md content (capped at 16 KB).
///
/// # Errors
///
/// - `400` for invalid namespace, name, or version
/// - `404` if the package does not exist
/// - `503` on S3 failure
/// - `500` on unpack or file I/O failure
pub async fn package_preview_handler(
    State(state): State<SharedState>,
    AxumPath((ns_raw, name_raw, version_raw)): AxumPath<(String, String, String)>,
) -> Result<Json<PreviewResponse>, StatusCode> {
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
            error!("s3 get_object error (preview): {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    let data = obj.body.collect().await.map_err(|e| {
        error!("s3 body collect error (preview): {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let bytes = data.into_bytes();
    let tmp = skreg_pack::unpack::unpack_to_tempdir(&bytes).map_err(|e| {
        error!("unpack error (preview): {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut files = Vec::new();
    collect_files(tmp.path(), tmp.path(), &mut files).map_err(|e| {
        error!("file walk error (preview): {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let skill_md_path = tmp.path().join("SKILL.md");
    let raw = std::fs::read_to_string(&skill_md_path).unwrap_or_default();
    let (skill_md, truncated) = if raw.len() > SKILL_MD_MAX {
        // Walk back from SKILL_MD_MAX to find a valid UTF-8 char boundary.
        let mut at = SKILL_MD_MAX;
        while !raw.is_char_boundary(at) {
            at -= 1;
        }
        (raw[..at].to_string(), true)
    } else {
        (raw, false)
    };

    Ok(Json(PreviewResponse {
        files,
        skill_md,
        truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::collect_files;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn collect_files_lists_recursively() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("SKILL.md"), "hello").unwrap();
        fs::write(tmp.path().join("manifest.json"), "{}").unwrap();
        fs::create_dir(tmp.path().join("references")).unwrap();
        fs::write(tmp.path().join("references").join("foo.md"), "ref").unwrap();

        let mut files = Vec::new();
        collect_files(tmp.path(), tmp.path(), &mut files).unwrap();
        files.sort();

        assert_eq!(
            files,
            vec!["SKILL.md", "manifest.json", "references/foo.md"]
        );
    }

    #[test]
    fn collect_files_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let mut files = Vec::new();
        collect_files(tmp.path(), tmp.path(), &mut files).unwrap();
        assert!(files.is_empty());
    }
}
