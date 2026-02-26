//! Represents a skill package installed on the local filesystem.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::package_ref::PackageRef;
use crate::types::Sha256Digest;

/// Who signed this package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignerKind {
    /// Signed by the skreg Registry Intermediate CA on behalf of an individual publisher.
    Registry,
    /// Signed by a verified publisher using their own leaf certificate.
    Publisher {
        /// Serial number of the publisher leaf certificate used to sign.
        cert_serial: u64,
    },
}

/// An installed skill package on the local filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Fully-qualified package reference including pinned version.
    pub pkg_ref: PackageRef,
    /// SHA-256 digest of the installed tarball.
    pub sha256: Sha256Digest,
    /// Who signed this package.
    pub signer: SignerKind,
    /// Absolute path to the extracted package directory.
    pub install_path: PathBuf,
}
