//! Orchestrates the full package install pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use log::{debug, info};
use sha2::{Digest, Sha256};
use thiserror::Error;

use skillpkg_client::client::RegistryClient;
use skillpkg_core::installed::{InstalledPackage, SignerKind};
use skillpkg_core::package_ref::PackageRef;
use skillpkg_core::types::Sha256Digest;
use skillpkg_pack::unpack::unpack_tarball;

/// Errors that can occur during package installation.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The registry client returned an error.
    #[error("registry error: {0}")]
    Registry(#[from] skillpkg_client::error::ClientError),
    /// The tarball sha256 does not match the manifest.
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    DigestMismatch {
        /// Expected hex digest from the manifest.
        expected: String,
        /// Actual computed hex digest.
        actual: String,
    },
    /// A crypto validation error occurred.
    #[error("crypto error: {0}")]
    Crypto(String),
    /// An I/O error occurred during extraction.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A pack/unpack error occurred.
    #[error("pack error: {0}")]
    Pack(#[from] skillpkg_pack::error::PackError),
    /// A core validation error occurred.
    #[error("validation error: {0}")]
    Validation(#[from] skillpkg_core::types::ValidationError),
}

/// Orchestrates download, verification, and extraction of a skill package.
pub struct Installer {
    client: Arc<dyn RegistryClient>,
    install_root: PathBuf,
}

impl Installer {
    /// Create a new `Installer`.
    ///
    /// # Arguments
    ///
    /// * `client` — Registry HTTP client.
    /// * `install_root` — Base directory for installed packages
    ///   (e.g. `~/.skillpkg/packages`).
    pub fn new(client: Arc<dyn RegistryClient>, install_root: PathBuf) -> Self {
        Self {
            client,
            install_root,
        }
    }

    /// Download, verify, and extract a package.
    ///
    /// Returns the installed package descriptor on success.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError`] if any step fails. Partial installs are
    /// cleaned up before returning.
    pub async fn install(&self, pkg_ref: &PackageRef) -> Result<InstalledPackage, InstallError> {
        info!("installing {pkg_ref}");

        let resolved = self.client.resolve(pkg_ref).await?;

        // Verify sha256
        let actual_hex = {
            let mut hasher = Sha256::new();
            hasher.update(&resolved.tarball);
            format!("{:x}", hasher.finalize())
        };
        let expected_hex = resolved.manifest.sha256.as_hex();
        if actual_hex != expected_hex {
            return Err(InstallError::DigestMismatch {
                expected: expected_hex.to_owned(),
                actual: actual_hex,
            });
        }

        debug!("sha256 verified for {pkg_ref}");

        let install_path = self
            .install_root
            .join(resolved.manifest.namespace.as_str())
            .join(resolved.manifest.name.as_str())
            .join(resolved.manifest.version.to_string());

        // Write tarball to temp file then unpack
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &resolved.tarball)?;
        unpack_tarball(tmp.path(), &install_path)?;

        info!("installed {} to {}", pkg_ref, install_path.display());

        Ok(InstalledPackage {
            pkg_ref: pkg_ref.clone(),
            sha256: Sha256Digest::from_hex(&actual_hex)?,
            signer: SignerKind::Registry,
            install_path,
        })
    }
}
