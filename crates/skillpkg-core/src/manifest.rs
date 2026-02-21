//! Package manifest type representing `manifest.json` inside a `.skill` tarball.

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::types::{Namespace, PackageName, Sha256Digest};

/// The contents of a `manifest.json` file inside a `.skill` package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Publisher namespace slug.
    pub namespace: Namespace,
    /// Package name slug.
    pub name: PackageName,
    /// Package version (semver).
    pub version: Version,
    /// Human-readable description (≥ 20 characters after trimming).
    pub description: String,
    /// Optional category tag.
    pub category: Option<String>,
    /// SHA-256 hex digest of the tarball this manifest describes.
    pub sha256: Sha256Digest,
    /// PEM-encoded certificate chain used to verify the package signature.
    /// Empty for registry-signed packages (cert chain is implicit).
    pub cert_chain_pem: Vec<String>,
}
