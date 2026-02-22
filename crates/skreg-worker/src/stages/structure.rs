//! Stage 1: structural validity checks on an unpacked skill package.

use std::path::Path;

use thiserror::Error;

const MAX_TOTAL_BYTES: u64 = 5 * 1024 * 1024; // 5 MB
const REQUIRED_FILES: &[&str] = &["SKILL.md", "manifest.json"];
const ALLOWED_EXTENSIONS: &[&str] = &["md", "json"];

/// Errors produced by structural validation.
#[derive(Debug, Error)]
pub enum StructureError {
    /// A required file is missing.
    #[error("required file '{0}' is missing")]
    MissingFile(String),
    /// Total size of all files exceeds the maximum.
    #[error("package size {size} bytes exceeds maximum of {max} bytes")]
    TooLarge {
        /// Actual total size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },
    /// A file with a disallowed extension was found.
    #[error("disallowed file type: '{0}'")]
    DisallowedFileType(String),
    /// An I/O error occurred during checking.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Run Stage 1 structural checks on the unpacked directory at `path`.
///
/// # Errors
///
/// Returns the first [`StructureError`] encountered.
pub fn check_structure(path: &Path) -> Result<(), StructureError> {
    for required in REQUIRED_FILES {
        if !path.join(required).exists() {
            return Err(StructureError::MissingFile((*required).to_owned()));
        }
    }

    let mut total_size: u64 = 0;

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
        if !ALLOWED_EXTENSIONS.contains(&ext) {
            return Err(StructureError::DisallowedFileType(
                entry.path().display().to_string(),
            ));
        }

        total_size += entry
            .metadata()
            .map_err(|e| StructureError::Io(e.into()))?
            .len();
        if total_size > MAX_TOTAL_BYTES {
            return Err(StructureError::TooLarge {
                size: total_size,
                max: MAX_TOTAL_BYTES,
            });
        }
    }

    Ok(())
}
