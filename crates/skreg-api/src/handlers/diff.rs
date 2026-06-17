//! Structured per-file diff model and computation (pure; no I/O).

// HTTP wiring is added in a later task; these items are not yet called from
// outside the module but will be when the endpoint is wired up.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

/// Number of unchanged context lines kept around each change.
const CONTEXT_RADIUS: usize = 3;

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
    /// Per-file diffs. Identical files are omitted. `SKILL.md` is first.
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
                let text = raw.strip_suffix('\n').unwrap_or(raw).to_owned();
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
pub(crate) fn compute_file_diffs(
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) -> Vec<FileDiff> {
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    paths.extend(old.keys().map(String::as_str));
    paths.extend(new.keys().map(String::as_str));

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
