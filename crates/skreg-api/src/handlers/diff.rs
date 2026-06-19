//! Structured per-file diff model and computation (pure; no I/O).

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde::{Deserialize, Serialize};
use skreg_core::types::{Namespace, PackageName};
use skreg_core::version::is_valid_segment;

use crate::handlers::packages::{list_published_versions, PublishedVersion};
use crate::handlers::preview::collect_files;
use crate::router::{AppState, SharedState};
use similar::{ChangeTag, TextDiff};

/// Number of unchanged context lines kept around each change.
const CONTEXT_RADIUS: usize = 3;

/// Paths that are always excluded from diffs. `manifest.json` contains
/// registry-managed fields (version string, sha256, signature) that change on
/// every publish, so including it in the diff produces noise and prevents
/// identical-content version bumps from reading as "No changes".
const EXCLUDED_PATHS: &[&str] = &["manifest.json"];

/// Kind of a single diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// Unchanged context line.
    Context,
    /// Line present only in the `to` version.
    Insert,
    /// Line present only in the `from` version.
    Delete,
}

/// A single line within a hunk. `text` has no trailing newline.
#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    /// Whether the line is context, an insertion, or a deletion.
    pub kind: LineKind,
    /// Line content without the trailing newline.
    pub text: String,
}

/// A contiguous block of changes with surrounding context, in unified-diff form.
#[derive(Debug, Clone, Serialize)]
pub struct Hunk {
    /// 1-based start line in the `from` file.
    pub old_start: usize,
    /// Number of `from` lines covered by this hunk.
    pub old_lines: usize,
    /// 1-based start line in the `to` file.
    pub new_start: usize,
    /// Number of `to` lines covered by this hunk.
    pub new_lines: usize,
    /// Ordered lines making up the hunk.
    pub lines: Vec<DiffLine>,
}

/// Change classification for a file across the two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// Present only in the `to` version.
    Added,
    /// Present only in the `from` version.
    Removed,
    /// Present in both, with differing content.
    Modified,
}

/// Diff for a single file.
#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    /// Path relative to the package root.
    pub path: String,
    /// Added, removed, or modified.
    pub status: FileStatus,
    /// True when either side is not valid UTF-8; `hunks` is then empty.
    pub binary: bool,
    /// Unified hunks (empty for binary files).
    pub hunks: Vec<Hunk>,
}

/// Top-level diff response body.
#[derive(Debug, Clone, Serialize)]
pub struct DiffResponse {
    /// The older version being compared.
    pub from: String,
    /// The newer version being compared.
    pub to: String,
    /// Per-file diffs. Identical files are omitted. `manifest.json` is always
    /// excluded (registry-managed fields). `SKILL.md` is first.
    pub files: Vec<FileDiff>,
}

/// Order key: `SKILL.md` sorts before everything else, then lexicographic.
fn order_key(path: &str) -> (u8, &str) {
    if path == "SKILL.md" {
        (0, path)
    } else {
        (1, path)
    }
}

/// Build the unified hunks for a UTF-8 text pair.
fn text_hunks(old: &str, new: &str) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old, new);
    let mut hunks = Vec::new();
    for group in diff.grouped_ops(CONTEXT_RADIUS) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let old_start = first.old_range().start;
        let old_end = last.old_range().end;
        let new_start = first.new_range().start;
        let new_end = last.new_range().end;
        let mut lines = Vec::new();
        for op in &group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => LineKind::Context,
                    ChangeTag::Delete => LineKind::Delete,
                    ChangeTag::Insert => LineKind::Insert,
                };
                let raw = change.value();
                let without_lf = raw.strip_suffix('\n').unwrap_or(raw);
                let text = without_lf
                    .strip_suffix('\r')
                    .unwrap_or(without_lf)
                    .to_owned();
                lines.push(DiffLine { kind, text });
            }
        }
        hunks.push(Hunk {
            old_start: old_start + 1,
            old_lines: old_end - old_start,
            new_start: new_start + 1,
            new_lines: new_end - new_start,
            lines,
        });
    }
    hunks
}

/// Compute per-file diffs between two unpacked package trees.
///
/// `old`/`new` map relative file paths to raw bytes. Files identical in both
/// trees are omitted. Files where either side is non-UTF-8 are reported with
/// `binary: true` and no hunks. `SKILL.md` is always first.
///
/// `manifest.json` is never emitted as a [`FileDiff`] because its
/// registry-managed fields (version string, sha256, signature) change on every
/// publish and produce noise in every diff. See [`EXCLUDED_PATHS`].
pub(crate) fn compute_file_diffs(
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) -> Vec<FileDiff> {
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for key in old.keys().map(String::as_str) {
        if !EXCLUDED_PATHS.contains(&key) {
            paths.insert(key);
        }
    }
    for key in new.keys().map(String::as_str) {
        if !EXCLUDED_PATHS.contains(&key) {
            paths.insert(key);
        }
    }

    let mut sorted: Vec<&str> = paths.into_iter().collect();
    sorted.sort_by(|a, b| order_key(a).cmp(&order_key(b)));

    let mut files = Vec::new();
    for path in sorted {
        let old_bytes = old.get(path);
        let new_bytes = new.get(path);
        let status = match (old_bytes, new_bytes) {
            (None, Some(_)) => FileStatus::Added,
            (Some(_), None) => FileStatus::Removed,
            (Some(o), Some(n)) => {
                if o == n {
                    continue;
                }
                FileStatus::Modified
            }
            (None, None) => continue,
        };

        let old_text = old_bytes.map(|b| std::str::from_utf8(b));
        let new_text = new_bytes.map(|b| std::str::from_utf8(b));
        let binary = matches!(old_text, Some(Err(_))) || matches!(new_text, Some(Err(_)));

        let hunks = if binary {
            Vec::new()
        } else {
            let o = old_text.and_then(Result::ok).unwrap_or("");
            let n = new_text.and_then(Result::ok).unwrap_or("");
            text_hunks(o, n)
        };

        files.push(FileDiff {
            path: path.to_owned(),
            status,
            binary,
            hunks,
        });
    }
    files
}

/// Query parameters for the diff endpoint.
#[derive(Debug, Deserialize)]
pub struct DiffQuery {
    /// Older version; defaults to the second-most-recent published version.
    pub from: Option<String>,
    /// Newer version; defaults to the most-recent published version.
    pub to: Option<String>,
}

/// Fetch a tarball from S3 by storage path, unpack it, and read every file into
/// a path → bytes map.
async fn read_package_files(
    state: &AppState,
    storage_path: &str,
) -> Result<BTreeMap<String, Vec<u8>>, StatusCode> {
    let obj = state
        .s3
        .get_object()
        .bucket(&state.s3_bucket)
        .key(storage_path)
        .send()
        .await
        .map_err(|e| {
            error!("s3 get_object error (diff): {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let data = obj.body.collect().await.map_err(|e| {
        error!("s3 body collect error (diff): {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let bytes = data.into_bytes();
    let tmp = skreg_pack::unpack::unpack_to_tempdir(&bytes).map_err(|e| {
        error!("unpack error (diff): {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut rel_paths = Vec::new();
    collect_files(tmp.path(), tmp.path(), &mut rel_paths).map_err(|e| {
        error!("file walk error (diff): {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut files = BTreeMap::new();
    for rel in rel_paths {
        let abs = tmp.path().join(&rel);
        let contents = std::fs::read(&abs).map_err(|e| {
            error!("file read error (diff): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        files.insert(rel, contents);
    }
    Ok(files)
}

/// Resolve a requested (possibly `None`/`"latest"`) version against the
/// published list, returning its row. `default_index` is used when the request
/// omits the version (0 = latest, 1 = previous).
fn resolve_requested<'a>(
    requested: Option<&str>,
    published: &'a [PublishedVersion],
    default_index: usize,
) -> Result<&'a PublishedVersion, StatusCode> {
    match requested {
        None => published.get(default_index).ok_or(StatusCode::NOT_FOUND),
        Some("latest") => published.first().ok_or(StatusCode::NOT_FOUND),
        Some(v) => {
            if !is_valid_segment(v) {
                return Err(StatusCode::BAD_REQUEST);
            }
            published
                .iter()
                .find(|p| p.version == v)
                .ok_or(StatusCode::NOT_FOUND)
        }
    }
}

/// Handle `GET /v1/packages/:ns/:name/diff?from=&to=`.
///
/// # Errors
///
/// - `400` invalid namespace/name, invalid version segment, or `from == to`
/// - `404` unknown package/version, or fewer than two published versions
/// - `503` S3 failure
/// - `500` unpack / read failure
pub async fn package_diff_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw)): Path<(String, String)>,
    Query(q): Query<DiffQuery>,
) -> Result<Json<DiffResponse>, StatusCode> {
    let ns = Namespace::new(&ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(&name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;

    let published = list_published_versions(&state, ns.as_str(), pkg_name.as_str()).await?;

    let to_row = resolve_requested(q.to.as_deref(), &published, 0)?;
    let from_row = resolve_requested(q.from.as_deref(), &published, 1)?;

    if from_row.version == to_row.version {
        return Err(StatusCode::BAD_REQUEST);
    }

    let old_files = read_package_files(&state, &from_row.storage_path).await?;
    let new_files = read_package_files(&state, &to_row.storage_path).await?;

    let files = compute_file_diffs(&old_files, &new_files);
    Ok(Json(DiffResponse {
        from: from_row.version.clone(),
        to: to_row.version.clone(),
        files,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pv(version: &str) -> crate::handlers::packages::PublishedVersion {
        crate::handlers::packages::PublishedVersion {
            version: version.to_owned(),
            published_at: chrono::Utc::now(),
            sha256: String::new(),
            storage_path: String::new(),
        }
    }

    #[test]
    fn resolve_defaults_to_latest_and_previous() {
        let list = vec![pv("2.0.0"), pv("1.0.0")];
        let to = super::resolve_requested(None, &list, 0).unwrap();
        let from = super::resolve_requested(None, &list, 1).unwrap();
        assert_eq!(to.version, "2.0.0");
        assert_eq!(from.version, "1.0.0");
    }

    #[test]
    fn resolve_rejects_invalid_segment() {
        let list = vec![pv("1.0.0")];
        let err = super::resolve_requested(Some("../bad"), &list, 0).unwrap_err();
        assert_eq!(err, axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolve_unknown_version_is_404() {
        let list = vec![pv("1.0.0")];
        let err = super::resolve_requested(Some("9.9.9"), &list, 0).unwrap_err();
        assert_eq!(err, axum::http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn resolve_missing_previous_is_404() {
        let list = vec![pv("1.0.0")];
        let err = super::resolve_requested(None, &list, 1).unwrap_err();
        assert_eq!(err, axum::http::StatusCode::NOT_FOUND);
    }

    fn map(pairs: &[(&str, &[u8])]) -> BTreeMap<String, Vec<u8>> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.to_vec()))
            .collect()
    }

    #[test]
    fn identical_files_are_omitted() {
        let a = map(&[("SKILL.md", b"hello\n")]);
        let b = map(&[("SKILL.md", b"hello\n")]);
        assert!(compute_file_diffs(&a, &b).is_empty());
    }

    #[test]
    fn added_file_is_all_inserts() {
        let a = map(&[]);
        let b = map(&[("references/new.md", b"line one\nline two\n")]);
        let files = compute_file_diffs(&a, &b);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, FileStatus::Added);
        assert!(!files[0].binary);
        let kinds: Vec<LineKind> = files[0].hunks[0].lines.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![LineKind::Insert, LineKind::Insert]);
    }

    #[test]
    fn removed_file_is_all_deletes() {
        let a = map(&[("references/old.md", b"gone\n")]);
        let b = map(&[]);
        let files = compute_file_diffs(&a, &b);
        assert_eq!(files[0].status, FileStatus::Removed);
        assert_eq!(files[0].hunks[0].lines[0].kind, LineKind::Delete);
    }

    #[test]
    fn modified_file_has_insert_and_delete() {
        let a = map(&[("SKILL.md", b"line a\ncommon\n")]);
        let b = map(&[("SKILL.md", b"line b\ncommon\n")]);
        let files = compute_file_diffs(&a, &b);
        assert_eq!(files[0].status, FileStatus::Modified);
        let kinds: Vec<LineKind> = files[0].hunks[0].lines.iter().map(|l| l.kind).collect();
        assert!(kinds.contains(&LineKind::Delete));
        assert!(kinds.contains(&LineKind::Insert));
        assert!(kinds.contains(&LineKind::Context));
        // text has no trailing newline
        assert_eq!(files[0].hunks[0].lines[0].text, "line a");
    }

    #[test]
    fn non_utf8_is_binary_with_no_hunks() {
        let a = map(&[("logo.png", &[0xff, 0xfe, 0x00])]);
        let b = map(&[("logo.png", &[0x00, 0x01, 0x02])]);
        let files = compute_file_diffs(&a, &b);
        assert_eq!(files[0].status, FileStatus::Modified);
        assert!(files[0].binary);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn skill_md_sorts_first() {
        let a = map(&[("SKILL.md", b"x\n"), ("references/a.md", b"a\n")]);
        let b = map(&[("SKILL.md", b"y\n"), ("references/a.md", b"b\n")]);
        let files = compute_file_diffs(&a, &b);
        assert_eq!(files[0].path, "SKILL.md");
        assert_eq!(files[1].path, "references/a.md");
    }

    #[test]
    fn manifest_json_is_excluded() {
        // Both SKILL.md and manifest.json differ between versions.
        let a = map(&[
            ("SKILL.md", b"version one content\n"),
            (
                "manifest.json",
                b"{\"version\":\"1.0.0\",\"sha256\":\"aaa\"}\n",
            ),
        ]);
        let b = map(&[
            ("SKILL.md", b"version two content\n"),
            (
                "manifest.json",
                b"{\"version\":\"2.0.0\",\"sha256\":\"bbb\"}\n",
            ),
        ]);
        let files = compute_file_diffs(&a, &b);
        // SKILL.md must appear; manifest.json must not.
        assert!(
            files.iter().any(|f| f.path == "SKILL.md"),
            "SKILL.md should be in the diff"
        );
        assert!(
            !files.iter().any(|f| f.path == "manifest.json"),
            "manifest.json must never appear in the diff"
        );
    }

    #[test]
    fn only_manifest_json_differs_returns_empty() {
        // SKILL.md is identical; only manifest.json changes (simulating a
        // content-identical version bump). The caller should see "No changes".
        let a = map(&[
            ("SKILL.md", b"same content\n"),
            (
                "manifest.json",
                b"{\"version\":\"1.0.0\",\"sha256\":\"aaa\"}\n",
            ),
        ]);
        let b = map(&[
            ("SKILL.md", b"same content\n"),
            (
                "manifest.json",
                b"{\"version\":\"1.0.1\",\"sha256\":\"bbb\"}\n",
            ),
        ]);
        let files = compute_file_diffs(&a, &b);
        assert!(
            files.is_empty(),
            "expected empty diff when only manifest.json differs, got: {files:?}"
        );
    }
}
