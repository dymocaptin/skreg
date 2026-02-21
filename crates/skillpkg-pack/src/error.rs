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
}
