//! Error types for registry HTTP client operations.

use thiserror::Error;

/// Errors that can occur during client–registry communication.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The server returned an unexpected status code.
    #[error("unexpected status {status}: {body}")]
    UnexpectedStatus {
        /// HTTP status code received.
        status: u16,
        /// Response body (truncated).
        body: String,
    },
    /// The response body could not be parsed.
    #[error("failed to parse response: {0}")]
    Parse(String),
}
