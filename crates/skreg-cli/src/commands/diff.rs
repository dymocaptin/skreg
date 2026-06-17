//! `skreg diff` — show a git-style diff between two published versions.

use anyhow::Context;
use skreg_client::client::{
    FileDiff, FileStatus, HttpRegistryClient, LineKind, RegistryClient, SkillDiff,
};
use skreg_core::package_ref::PackageRef;
use skreg_core::version::is_valid_segment;

use crate::config::{apply_context, default_config_path, load_config};

/// ANSI color codes, used only when `use_color` is true.
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";

fn status_label(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "added",
        FileStatus::Removed => "removed",
        FileStatus::Modified => "modified",
    }
}

fn paint(text: &str, code: &str, use_color: bool) -> String {
    if use_color {
        format!("{code}{text}{RESET}")
    } else {
        text.to_owned()
    }
}

fn format_file(file: &FileDiff, use_color: bool, out: &mut String) {
    let header = format!("diff --skreg {} ({})", file.path, status_label(file.status));
    out.push_str(&paint(&header, BOLD, use_color));
    out.push('\n');

    if file.binary {
        out.push_str(&paint("Binary file differs", DIM, use_color));
        out.push('\n');
        return;
    }

    for hunk in &file.hunks {
        let head = format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        );
        out.push_str(&paint(&head, DIM, use_color));
        out.push('\n');
        for line in &hunk.lines {
            let (prefix, code) = match line.kind {
                LineKind::Context => (" ", DIM),
                LineKind::Insert => ("+", GREEN),
                LineKind::Delete => ("-", RED),
            };
            out.push_str(&paint(&format!("{prefix}{}", line.text), code, use_color));
            out.push('\n');
        }
    }
}

/// Render a full diff to a string for terminal display.
#[must_use]
pub fn format_diff(diff: &SkillDiff, use_color: bool) -> String {
    let mut out = String::new();
    let title = format!("Comparing {} \u{2192} {}", diff.from, diff.to);
    out.push_str(&paint(&title, BOLD, use_color));
    out.push('\n');
    if diff.files.is_empty() {
        out.push_str("No changes between these versions.\n");
        return out;
    }
    for file in &diff.files {
        out.push('\n');
        format_file(file, use_color, &mut out);
    }
    out
}

/// Run the `skreg diff` command.
///
/// # Errors
///
/// Returns an error if the package ref is malformed, a version flag is
/// syntactically invalid, or the registry request fails.
pub async fn run_diff(
    package_ref: &str,
    from: Option<&str>,
    to: Option<&str>,
    use_color: bool,
    context: Option<&str>,
) -> anyhow::Result<()> {
    for (label, v) in [("--from", from), ("--to", to)] {
        if let Some(v) = v {
            anyhow::ensure!(
                is_valid_segment(v),
                "invalid {label} version {v:?} — expected a version string or 'latest'"
            );
        }
    }

    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference {package_ref:?}"))?;
    let ns = pkg_ref.namespace.as_str();
    let name = pkg_ref.name.as_str();

    let cfg = load_config(&default_config_path())
        .context("not logged in — run `skreg login <namespace>` first")?;
    let cfg = apply_context(cfg, context)?;
    let client = HttpRegistryClient::new(cfg.registry());

    let diff = client
        .diff(ns, name, from, to)
        .await
        .with_context(|| format!("failed to fetch diff for {ns}/{name}"))?;

    print!("{}", format_diff(&diff, use_color));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreg_client::client::{DiffLine, FileDiff, Hunk};

    fn sample() -> SkillDiff {
        SkillDiff {
            from: "1.0.0".to_owned(),
            to: "1.0.1".to_owned(),
            files: vec![FileDiff {
                path: "SKILL.md".to_owned(),
                status: FileStatus::Modified,
                binary: false,
                hunks: vec![Hunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    lines: vec![
                        DiffLine {
                            kind: LineKind::Delete,
                            text: "old".to_owned(),
                        },
                        DiffLine {
                            kind: LineKind::Insert,
                            text: "new".to_owned(),
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn plain_output_has_diff_markers_and_no_ansi() {
        let out = format_diff(&sample(), false);
        assert!(out.contains("diff --skreg SKILL.md (modified)"));
        assert!(out.contains("@@ -1,1 +1,1 @@"));
        assert!(out.contains("-old"));
        assert!(out.contains("+new"));
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn color_output_contains_ansi_codes() {
        let out = format_diff(&sample(), true);
        assert!(out.contains("\x1b[32m")); // green insert
        assert!(out.contains("\x1b[31m")); // red delete
    }

    #[test]
    fn empty_diff_reports_no_changes() {
        let diff = SkillDiff {
            from: "1.0.0".into(),
            to: "1.0.1".into(),
            files: vec![],
        };
        let out = format_diff(&diff, false);
        assert!(out.contains("No changes between these versions."));
    }
}
