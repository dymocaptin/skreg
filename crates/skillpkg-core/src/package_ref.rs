//! Fully-qualified package reference, e.g. `acme/deploy-helper@1.2.3`.

use std::fmt;

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{Namespace, PackageName, ValidationError};

/// Error returned when a package reference string cannot be parsed.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The string does not contain a `/` separator.
    #[error("package reference must be in the form 'namespace/name[@version]'")]
    MissingSlash,
    /// The namespace segment is invalid.
    #[error("invalid namespace: {0}")]
    InvalidNamespace(#[from] ValidationError),
    /// The version segment cannot be parsed as semver.
    #[error("invalid semver version: {0}")]
    InvalidVersion(#[from] semver::Error),
}

/// A fully-qualified package reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRef {
    /// Publisher namespace.
    pub namespace: Namespace,
    /// Package name.
    pub name: PackageName,
    /// Optional pinned version; `None` means "latest".
    pub version: Option<Version>,
}

impl PackageRef {
    /// Parse a package reference from a string in the form `ns/name[@version]`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the string is malformed.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let (ns_name, version) = match input.split_once('@') {
            Some((left, v)) => (left, Some(Version::parse(v)?)),
            None => (input, None),
        };

        let (ns_str, name_str) = ns_name.split_once('/').ok_or(ParseError::MissingSlash)?;

        Ok(Self {
            namespace: Namespace::new(ns_str)?,
            name: PackageName::new(name_str).map_err(ParseError::InvalidNamespace)?,
            version,
        })
    }
}

impl fmt::Display for PackageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)?;
        if let Some(v) = &self.version {
            write!(f, "@{v}")?;
        }
        Ok(())
    }
}
