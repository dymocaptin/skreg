//! Validated newtype wrappers for core domain primitives.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when a domain value fails validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    /// The value is empty.
    #[error("value must not be empty")]
    Empty,
    /// The value exceeds the maximum length.
    #[error("value exceeds maximum length of {max} characters (got {got})")]
    TooLong {
        /// Maximum allowed length.
        max: usize,
        /// Actual length.
        got: usize,
    },
    /// The value contains disallowed characters.
    #[error("value contains invalid characters: only lowercase alphanumeric and hyphens allowed")]
    InvalidCharacters,
    /// The hex string is not the expected length.
    #[error("expected 64 hex characters, got {0}")]
    InvalidHexLength(usize),
    /// The hex string contains non-hex characters.
    #[error("value contains non-hex characters")]
    InvalidHex,
}

/// A validated namespace slug (lowercase alphanumeric + hyphens, 1–64 chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Namespace(String);

impl Namespace {
    /// Create a new `Namespace` from a string slice, validating the slug format.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the slug is empty, exceeds 64 characters,
    /// or contains characters other than lowercase letters, digits, and hyphens.
    pub fn new(slug: &str) -> Result<Self, ValidationError> {
        if slug.is_empty() {
            return Err(ValidationError::Empty);
        }
        if slug.len() > 64 {
            return Err(ValidationError::TooLong {
                max: 64,
                got: slug.len(),
            });
        }
        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ValidationError::InvalidCharacters);
        }
        Ok(Self(slug.to_owned()))
    }

    /// Return the inner slug string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated package name (same constraints as [`Namespace`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageName(String);

impl PackageName {
    /// Create a new `PackageName`, validating the slug format.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the name is invalid per namespace rules.
    pub fn new(name: &str) -> Result<Self, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::Empty);
        }
        if name.len() > 64 {
            return Err(ValidationError::TooLong {
                max: 64,
                got: name.len(),
            });
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ValidationError::InvalidCharacters);
        }
        Ok(Self(name.to_owned()))
    }

    /// Return the inner name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated SHA-256 hex digest (exactly 64 lowercase hex characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Parse a `Sha256Digest` from a hex string.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the string is not exactly 64 lowercase hex characters.
    pub fn from_hex(hex: &str) -> Result<Self, ValidationError> {
        if hex.len() != 64 {
            return Err(ValidationError::InvalidHexLength(hex.len()));
        }
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidHex);
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    /// Return the hex string representation.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
