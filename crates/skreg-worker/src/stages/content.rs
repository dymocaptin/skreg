//! Stage 2: content validity checks.

use std::path::Path;

use thiserror::Error;

const MIN_DESCRIPTION_LEN: usize = 20;

/// Patterns that suggest hardcoded secrets.
const SECRET_PATTERNS: &[&str] = &[
    "password=",
    "passwd=",
    "secret=",
    "api_key=",
    "apikey=",
    "token=",
    "private_key=",
    "-----BEGIN",
];

/// Errors returned by [`check_content`].
#[derive(Debug, Error)]
pub enum ContentError {
    /// The description field is too short.
    #[error("description is too short (minimum {MIN_DESCRIPTION_LEN} characters)")]
    DescriptionTooShort,
    /// A markdown file contains a pattern that looks like a hardcoded secret.
    #[error("possible hardcoded secret found: '{0}'")]
    HardcodedSecret(String),
    /// A non-markdown file was found inside the `references/` directory.
    #[error("non-markdown file in references/: '{0}'")]
    NonMarkdownInReferences(String),
    /// An I/O error occurred while reading the package contents.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Run Stage 2 content checks on an unpacked skill directory.
///
/// # Errors
///
/// Returns a [`ContentError`] if any check fails.
pub fn check_content(path: &Path) -> Result<(), ContentError> {
    // 1. Description length from manifest
    let manifest_raw = std::fs::read_to_string(path.join("manifest.json"))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).map_err(|e| std::io::Error::other(e.to_string()))?;
    let desc = manifest["description"].as_str().unwrap_or("");
    if desc.len() < MIN_DESCRIPTION_LEN {
        return Err(ContentError::DescriptionTooShort);
    }

    // 2. Scan all .md files for secret patterns
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;
        if entry.file_type().is_dir() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext != "md" {
            continue;
        }

        let content = std::fs::read_to_string(entry.path())?;
        for pattern in SECRET_PATTERNS {
            if content.to_lowercase().contains(&pattern.to_lowercase()) {
                return Err(ContentError::HardcodedSecret((*pattern).to_owned()));
            }
        }
    }

    // 3. references/ must contain only .md files
    let refs_dir = path.join("references");
    if refs_dir.exists() {
        for entry in std::fs::read_dir(&refs_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".md") {
                return Err(ContentError::NonMarkdownInReferences(name_str.to_string()));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_skill(dir: &std::path::Path, description: &str) {
        fs::write(
            dir.join("manifest.json"),
            format!(r#"{{"name":"test","version":"1.0.0","description":"{description}"}}"#),
        )
        .unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\ndescription: {description}\n---\n"),
        )
        .unwrap();
    }

    #[test]
    fn description_too_short_fails() {
        let dir = tempdir().unwrap();
        make_skill(dir.path(), "short");
        let err = check_content(dir.path()).unwrap_err();
        assert!(matches!(err, ContentError::DescriptionTooShort));
    }

    #[test]
    fn hardcoded_secret_fails() {
        let dir = tempdir().unwrap();
        make_skill(
            dir.path(),
            "A description that is long enough to pass the length check here",
        );
        fs::write(dir.path().join("SKILL.md"), "password=hunter2").unwrap();
        let err = check_content(dir.path()).unwrap_err();
        assert!(matches!(err, ContentError::HardcodedSecret(_)));
    }

    #[test]
    fn non_md_in_references_fails() {
        let dir = tempdir().unwrap();
        make_skill(
            dir.path(),
            "A description that is long enough to pass the length check here",
        );
        fs::create_dir(dir.path().join("references")).unwrap();
        fs::write(dir.path().join("references/script.py"), "code").unwrap();
        let err = check_content(dir.path()).unwrap_err();
        assert!(matches!(err, ContentError::NonMarkdownInReferences(_)));
    }

    #[test]
    fn valid_package_passes() {
        let dir = tempdir().unwrap();
        make_skill(
            dir.path(),
            "A description that is long enough to pass the length check here",
        );
        assert!(check_content(dir.path()).is_ok());
    }
}
