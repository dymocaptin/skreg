//! Stage 1: structural validity checks on an unpacked skill package.

use std::path::{Component, Path};

use skreg_core::limits;
use thiserror::Error;

const REQUIRED_FILES: &[&str] = &["SKILL.md", "manifest.json"];
const ALLOWED_ROOT_FILES: &[&str] = &["SKILL.md", "manifest.json"];
const ALLOWED_ROOT_PREFIXES: &[&str] = &["LICENSE"];
const SCRIPT_EXTENSIONS: &[&str] = &["py", "sh", "bash", "js", "ts", "rb"];
const REFERENCE_EXTENSIONS: &[&str] = &["md"];
const ASSET_EXTENSIONS: &[&str] = &[
    "md", "txt", "json", "yaml", "yml", "csv", "png", "jpg", "svg", "pdf",
];

/// Errors produced by structural validation.
#[derive(Debug, Error)]
pub enum StructureError {
    /// A required file is missing.
    #[error("required file '{0}' is missing")]
    MissingFile(String),
    /// Total size of all files exceeds the maximum.
    #[error("package size {size} bytes exceeds {max} bytes")]
    PackageTooLarge {
        /// Actual total size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },
    /// Number of files exceeds the maximum.
    #[error("package contains {count} files, maximum is {max}")]
    TooManyFiles {
        /// Actual file count.
        count: usize,
        /// Maximum allowed count.
        max: usize,
    },
    /// A single file exceeds the per-file size limit.
    #[error("file '{path}' is {size} bytes, exceeds limit of {max} bytes")]
    FileTooLarge {
        /// Path of the oversized file.
        path: String,
        /// Actual file size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },
    /// A file is in a disallowed location or has a disallowed extension.
    #[error("disallowed file: '{0}'")]
    DisallowedFile(String),
    /// A file is inside a disallowed subdirectory.
    #[error("subdirectories are not allowed in '{0}'")]
    DisallowedSubdirectory(String),
    /// A symlink was found (defence-in-depth; should be caught at unpack).
    #[error("symlink found: '{0}'")]
    Symlink(String),
    /// A path traversal component was detected (defence-in-depth).
    #[error("path traversal attempt: '{0}'")]
    PathTraversal(String),
    /// `SKILL.md` is missing the YAML frontmatter block.
    #[error("SKILL.md is missing YAML frontmatter")]
    FrontmatterMissing,
    /// The frontmatter YAML could not be parsed.
    #[error("SKILL.md frontmatter is invalid YAML: {0}")]
    FrontmatterInvalid(String),
    /// A required frontmatter field is absent or empty.
    #[error("SKILL.md frontmatter field '{field}' is invalid: {reason}")]
    FrontmatterFieldInvalid {
        /// The field name.
        field: String,
        /// The reason the field is invalid.
        reason: String,
    },
    /// `SKILL.md` exceeds the maximum line count.
    #[error("SKILL.md exceeds {max} lines ({got} lines)")]
    SkillMdTooLong {
        /// Actual line count.
        got: usize,
        /// Maximum allowed line count.
        max: usize,
    },
    /// `manifest.json` exceeds its size limit.
    #[error("manifest.json is {size} bytes, exceeds limit of {max} bytes")]
    ManifestTooLarge {
        /// Actual size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },
    /// An I/O error occurred during checking.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, serde::Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    compatibility: Option<String>,
    #[allow(dead_code)]
    license: Option<String>,
    #[allow(dead_code)]
    metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "allowed-tools")]
    #[allow(dead_code)]
    allowed_tools: Option<String>,
}

/// Run Stage 1 structural checks on the unpacked directory at `path`.
///
/// # Errors
///
/// Returns the first [`StructureError`] encountered.
pub fn check_structure(path: &Path) -> Result<(), StructureError> {
    // 1. Required files exist
    for required in REQUIRED_FILES {
        if !path.join(required).exists() {
            return Err(StructureError::MissingFile((*required).to_owned()));
        }
    }

    // 2. manifest.json size limit
    let manifest_size = std::fs::metadata(path.join("manifest.json"))?.len();
    if manifest_size > limits::LIMIT_MANIFEST_SIZE {
        return Err(StructureError::ManifestTooLarge {
            size: manifest_size,
            max: limits::LIMIT_MANIFEST_SIZE,
        });
    }

    // 3. Validate SKILL.md frontmatter and line count
    validate_skill_md(&path.join("SKILL.md"))?;

    // 4. Walk all entries
    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;

    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;

        // Skip the root directory itself
        if entry.path() == path {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        // Symlink check (defence-in-depth)
        if entry.path_is_symlink() {
            return Err(StructureError::Symlink(rel.display().to_string()));
        }

        // Path safety
        validate_path_safety(rel)?;

        if entry.file_type().is_dir() {
            // Directories themselves are not counted; file location check for
            // subdirectories happens when we encounter their file children.
            continue;
        }

        // File location / extension
        validate_file_location(rel)?;

        file_count += 1;
        if file_count > limits::LIMIT_MAX_FILES {
            return Err(StructureError::TooManyFiles {
                count: file_count,
                max: limits::LIMIT_MAX_FILES,
            });
        }

        let file_size = entry
            .metadata()
            .map_err(|e| StructureError::Io(e.into()))?
            .len();

        // Per-file size limit (scripts have a tighter cap)
        let parts: Vec<_> = rel.components().collect();
        let per_file_max = if matches!(parts.as_slice(), [Component::Normal(d), _] if *d == "scripts")
        {
            limits::LIMIT_SCRIPT_FILE_SIZE
        } else {
            limits::LIMIT_FILE_SIZE
        };

        if file_size > per_file_max {
            return Err(StructureError::FileTooLarge {
                path: rel.display().to_string(),
                size: file_size,
                max: per_file_max,
            });
        }

        total_size += file_size;
        if total_size > limits::LIMIT_PACKAGE_SIZE {
            return Err(StructureError::PackageTooLarge {
                size: total_size,
                max: limits::LIMIT_PACKAGE_SIZE,
            });
        }
    }

    Ok(())
}

fn validate_path_safety(rel: &Path) -> Result<(), StructureError> {
    for component in rel.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(StructureError::PathTraversal(rel.display().to_string()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn validate_file_location(rel: &Path) -> Result<(), StructureError> {
    let parts: Vec<_> = rel.components().collect();

    match parts.as_slice() {
        [Component::Normal(name)] => {
            let name_str = name.to_string_lossy();
            let allowed = ALLOWED_ROOT_FILES.contains(&&*name_str)
                || ALLOWED_ROOT_PREFIXES
                    .iter()
                    .any(|p| name_str.starts_with(p));
            if !allowed {
                return Err(StructureError::DisallowedFile(rel.display().to_string()));
            }
        }
        [Component::Normal(dir), Component::Normal(_file)] => {
            let dir_str = dir.to_string_lossy();
            let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");
            match dir_str.as_ref() {
                "scripts" => {
                    if !SCRIPT_EXTENSIONS.contains(&ext) {
                        return Err(StructureError::DisallowedFile(rel.display().to_string()));
                    }
                }
                "references" => {
                    if !REFERENCE_EXTENSIONS.contains(&ext) {
                        return Err(StructureError::DisallowedFile(rel.display().to_string()));
                    }
                }
                "assets" => {
                    if !ASSET_EXTENSIONS.contains(&ext) {
                        return Err(StructureError::DisallowedFile(rel.display().to_string()));
                    }
                }
                _ => return Err(StructureError::DisallowedFile(rel.display().to_string())),
            }
        }
        [Component::Normal(dir), ..] => {
            return Err(StructureError::DisallowedSubdirectory(
                dir.to_string_lossy().into_owned(),
            ));
        }
        _ => return Err(StructureError::DisallowedFile(rel.display().to_string())),
    }
    Ok(())
}

fn validate_skill_md(path: &Path) -> Result<(), StructureError> {
    // Pre-read size guard: reject oversized SKILL.md before loading into memory.
    let file_size = std::fs::metadata(path)?.len();
    if file_size > limits::LIMIT_FILE_SIZE {
        return Err(StructureError::FileTooLarge {
            path: "SKILL.md".to_owned(),
            size: file_size,
            max: limits::LIMIT_FILE_SIZE,
        });
    }

    let content = std::fs::read_to_string(path)?;

    // Line count check
    let line_count = content.lines().count();
    if line_count > limits::LIMIT_SKILL_MD_LINES {
        return Err(StructureError::SkillMdTooLong {
            got: line_count,
            max: limits::LIMIT_SKILL_MD_LINES,
        });
    }

    // Must start with ---
    if !content.starts_with("---") {
        return Err(StructureError::FrontmatterMissing);
    }

    // Find closing "---" on its own line: must be "\n---\n" or "\n---" at EOF
    let after_open = &content[3..];
    let close_pos = after_open
        .find("\n---\n")
        .or_else(|| after_open.strip_suffix("\n---").map(str::len))
        .ok_or(StructureError::FrontmatterMissing)?;

    let yaml_block = &after_open[..close_pos];

    // Parse YAML
    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_block)
        .map_err(|e| StructureError::FrontmatterInvalid(e.to_string()))?;

    // Validate name
    match frontmatter.name.as_deref() {
        None | Some("") => {
            return Err(StructureError::FrontmatterFieldInvalid {
                field: "name".to_owned(),
                reason: "field is missing or empty".to_owned(),
            });
        }
        Some(name) => {
            if name.len() > limits::LIMIT_NAME_LEN {
                return Err(StructureError::FrontmatterFieldInvalid {
                    field: "name".to_owned(),
                    reason: "must be 64 characters or fewer".to_owned(),
                });
            }
            if !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(StructureError::FrontmatterFieldInvalid {
                    field: "name".to_owned(),
                    reason: "must contain only lowercase letters, digits, and hyphens".to_owned(),
                });
            }
            if name.starts_with('-') || name.ends_with('-') {
                return Err(StructureError::FrontmatterFieldInvalid {
                    field: "name".to_owned(),
                    reason: "must not start or end with a hyphen".to_owned(),
                });
            }
            if name.contains("--") {
                return Err(StructureError::FrontmatterFieldInvalid {
                    field: "name".to_owned(),
                    reason: "must not contain consecutive hyphens".to_owned(),
                });
            }
        }
    }

    // Validate description
    match frontmatter.description.as_deref() {
        None | Some("") => {
            return Err(StructureError::FrontmatterFieldInvalid {
                field: "description".to_owned(),
                reason: "field is missing or empty".to_owned(),
            });
        }
        Some(desc) => {
            if desc.len() > limits::LIMIT_DESCRIPTION_LEN {
                return Err(StructureError::FrontmatterFieldInvalid {
                    field: "description".to_owned(),
                    reason: "must be 1024 characters or fewer".to_owned(),
                });
            }
        }
    }

    // Optionally validate compatibility if present
    if let Some(compat) = frontmatter.compatibility.as_deref() {
        if compat.is_empty() || compat.len() > limits::LIMIT_COMPATIBILITY_LEN {
            return Err(StructureError::FrontmatterFieldInvalid {
                field: "compatibility".to_owned(),
                reason: "must be 1–500 characters".to_owned(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Write the minimum valid package to `dir`.
    fn make_valid_package(dir: &Path) {
        let frontmatter =
            "---\nname: my-skill\ndescription: A valid skill description\n---\n# My Skill\n";
        fs::write(dir.join("SKILL.md"), frontmatter).unwrap();
        fs::write(
            dir.join("manifest.json"),
            r#"{"name":"my-skill","version":"1.0.0","description":"A valid skill description"}"#,
        )
        .unwrap();
    }

    #[test]
    fn missing_skill_md_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("manifest.json"), "{}").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::MissingFile(ref f) if f == "SKILL.md"),
            "got: {err}"
        );
    }

    #[test]
    fn missing_manifest_fails() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: x\ndescription: y\n---\n",
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::MissingFile(ref f) if f == "manifest.json"),
            "got: {err}"
        );
    }

    #[test]
    fn too_many_files_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("references")).unwrap();
        for i in 0..=limits::LIMIT_MAX_FILES {
            fs::write(
                dir.path().join("references").join(format!("ref{i}.md")),
                "# ref",
            )
            .unwrap();
        }
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::TooManyFiles { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn package_too_large_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        let big = vec![b'x'; (limits::LIMIT_PACKAGE_SIZE + 1) as usize];
        fs::create_dir(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets").join("big.txt"), &big).unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(
                err,
                StructureError::FileTooLarge { .. } | StructureError::PackageTooLarge { .. }
            ),
            "got: {err}"
        );
    }

    #[test]
    fn script_file_too_large_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("scripts")).unwrap();
        let big = vec![b'x'; (limits::LIMIT_SCRIPT_FILE_SIZE + 1) as usize];
        fs::write(dir.path().join("scripts").join("setup.py"), &big).unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FileTooLarge { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn manifest_too_large_fails() {
        let dir = tempdir().unwrap();
        let frontmatter = "---\nname: my-skill\ndescription: A valid skill description\n---\n";
        fs::write(dir.path().join("SKILL.md"), frontmatter).unwrap();
        let big = vec![b'x'; (limits::LIMIT_MANIFEST_SIZE + 1) as usize];
        fs::write(dir.path().join("manifest.json"), &big).unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::ManifestTooLarge { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn unknown_root_file_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::write(dir.path().join("extra.toml"), "data").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedFile(_)),
            "got: {err}"
        );
    }

    #[test]
    fn script_py_in_scripts_passes() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts").join("setup.py"), "print('hi')").unwrap();
        assert!(check_structure(dir.path()).is_ok());
    }

    #[test]
    fn binary_in_scripts_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts").join("evil.exe"), "data").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedFile(_)),
            "got: {err}"
        );
    }

    #[test]
    fn subdir_in_references_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        let subdir = dir.path().join("references").join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.md"), "# ref").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedSubdirectory(_)),
            "got: {err}"
        );
    }

    #[test]
    fn subdir_in_assets_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        let subdir = dir.path().join("assets").join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("img.png"), b"\x89PNG").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedSubdirectory(_)),
            "got: {err}"
        );
    }

    #[test]
    fn non_md_in_references_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("references")).unwrap();
        fs::write(dir.path().join("references").join("data.json"), "{}").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedFile(_)),
            "got: {err}"
        );
    }

    #[test]
    fn unknown_extension_in_assets_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets").join("data.wasm"), "bad").unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::DisallowedFile(_)),
            "got: {err}"
        );
    }

    #[test]
    fn missing_frontmatter_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), "# No frontmatter here\n").unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterMissing),
            "got: {err}"
        );
    }

    #[test]
    fn frontmatter_missing_name_fails() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\ndescription: A valid skill description\n---\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterFieldInvalid { ref field, .. } if field == "name"),
            "got: {err}"
        );
    }

    #[test]
    fn frontmatter_name_consecutive_hyphens_fails() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: my--skill\ndescription: A valid skill description\n---\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterFieldInvalid { ref field, .. } if field == "name"),
            "got: {err}"
        );
    }

    #[test]
    fn frontmatter_name_starts_with_hyphen_fails() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: -bad\ndescription: A valid skill description\n---\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterFieldInvalid { ref field, .. } if field == "name"),
            "got: {err}"
        );
    }

    #[test]
    fn frontmatter_missing_description_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), "---\nname: my-skill\n---\n").unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterFieldInvalid { ref field, .. } if field == "description"),
            "got: {err}"
        );
    }

    #[test]
    fn frontmatter_description_too_long_fails() {
        let dir = tempdir().unwrap();
        let long_desc = "x".repeat(1025);
        let content = format!("---\nname: my-skill\ndescription: {long_desc}\n---\n");
        fs::write(dir.path().join("SKILL.md"), &content).unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::FrontmatterFieldInvalid { ref field, .. } if field == "description"),
            "got: {err}"
        );
    }

    #[test]
    fn skill_md_too_many_lines_fails() {
        let dir = tempdir().unwrap();
        let lines = "line\n".repeat(limits::LIMIT_SKILL_MD_LINES + 1);
        let content =
            format!("---\nname: my-skill\ndescription: A valid skill description\n---\n{lines}");
        fs::write(dir.path().join("SKILL.md"), &content).unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"x","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(
            matches!(err, StructureError::SkillMdTooLong { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn valid_minimal_package_passes() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        assert!(check_structure(dir.path()).is_ok());
    }

    #[test]
    fn valid_full_package_passes() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("scripts")).unwrap();
        fs::write(dir.path().join("scripts").join("setup.py"), "print('hi')").unwrap();
        fs::write(
            dir.path().join("scripts").join("run.sh"),
            "#!/bin/sh\necho hi",
        )
        .unwrap();
        fs::create_dir(dir.path().join("references")).unwrap();
        fs::write(dir.path().join("references").join("guide.md"), "# Guide").unwrap();
        fs::create_dir(dir.path().join("assets")).unwrap();
        fs::write(
            dir.path().join("assets").join("diagram.png"),
            b"\x89PNG\r\n\x1a\n",
        )
        .unwrap();
        fs::write(dir.path().join("LICENSE"), "Apache 2.0").unwrap();
        assert!(
            check_structure(dir.path()).is_ok(),
            "full valid package should pass"
        );
    }

    #[test]
    fn single_char_name_is_valid() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: a\ndescription: A valid skill description\n---\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"a","version":"1.0.0","description":"desc"}"#,
        )
        .unwrap();
        assert!(
            check_structure(dir.path()).is_ok(),
            "single-char name 'a' should be valid"
        );
    }

    #[test]
    fn non_script_file_at_script_size_limit_passes() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        fs::create_dir(dir.path().join("references")).unwrap();
        let slightly_over_script_limit = vec![b'x'; (limits::LIMIT_SCRIPT_FILE_SIZE + 1) as usize];
        fs::write(
            dir.path().join("references").join("big.md"),
            &slightly_over_script_limit,
        )
        .unwrap();
        assert!(
            check_structure(dir.path()).is_ok(),
            "129 KB file in references/ should pass (only scripts/ use the 128 KB cap)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_unpacked_dir_fails() {
        let dir = tempdir().unwrap();
        make_valid_package(dir.path());
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("evil-link")).unwrap();
        let err = check_structure(dir.path()).unwrap_err();
        assert!(matches!(err, StructureError::Symlink(_)), "got: {err}");
    }
}
