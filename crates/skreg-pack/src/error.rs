//! Error types for pack/unpack operations.

use thiserror::Error;

/// Errors that can occur when packing or unpacking a `.skill` tarball.
#[derive(Debug, Error)]
pub enum PackError {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The source directory does not contain a required file.
    #[error("required file '{0}' not found in source directory")]
    MissingFile(String),
    /// `manifest.json` could not be parsed.
    #[error("manifest.json parse error: {0}")]
    ManifestParse(String),
    /// A tar entry is a symlink, which is not allowed in skill packages.
    #[error("symlink found in package: '{0}'")]
    Symlink(String),
    /// A tar entry path contains '..' or an absolute path component.
    #[error("path traversal attempt in package entry: '{0}'")]
    PathTraversal(String),
}
