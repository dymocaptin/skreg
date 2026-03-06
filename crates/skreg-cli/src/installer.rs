//! Orchestrates the full package install pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{debug, info};
use sha2::{Digest, Sha256};
use thiserror::Error;

use skreg_client::client::RegistryClient;
use skreg_core::installed::{InstalledPackage, SignerKind};
use skreg_core::package_ref::PackageRef;
use skreg_core::types::Sha256Digest;
use skreg_crypto::verifier::SignatureVerifier;
use skreg_pack::unpack::unpack_tarball;

/// Errors that can occur during package installation.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The registry client returned an error.
    #[error("registry error: {0}")]
    Registry(#[from] skreg_client::error::ClientError),
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
    Pack(#[from] skreg_pack::error::PackError),
    /// A core validation error occurred.
    #[error("validation error: {0}")]
    Validation(#[from] skreg_core::types::ValidationError),
}

/// Orchestrates download, verification, and extraction of a skill package.
pub struct Installer {
    client: Arc<dyn RegistryClient>,
    install_root: PathBuf,
    verifier: Option<Arc<dyn SignatureVerifier>>,
}

/// Remove all version subdirectories under `name_dir`, enforcing the
/// single-version constraint. No-op if `name_dir` does not exist.
fn clear_existing_versions(name_dir: &Path) -> std::io::Result<()> {
    if !name_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(name_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

impl Installer {
    /// Create a new `Installer`.
    ///
    /// # Arguments
    ///
    /// * `client` — Registry HTTP client.
    /// * `install_root` — Base directory for installed packages
    ///   (e.g. `~/.skreg/packages`).
    pub fn new(client: Arc<dyn RegistryClient>, install_root: PathBuf) -> Self {
        Self {
            client,
            install_root,
            verifier: None,
        }
    }

    /// Attach an optional signature verifier.
    ///
    /// When set, `install()` will call `verifier.verify()` after the sha256
    /// check and return an error if the signature is invalid.
    #[must_use]
    pub fn with_verifier(mut self, verifier: Arc<dyn SignatureVerifier>) -> Self {
        self.verifier = Some(verifier);
        self
    }

    /// Download, verify, and extract a package.
    ///
    /// Returns the installed package descriptor on success.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError`] if any step fails. On extraction failure, the
    /// partially-created installation directory may remain on disk.
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

        // Safety: actual_hex comes from sha2::Digest::finalize(), always valid hex.
        let digest = Sha256Digest::from_hex(&actual_hex)?;

        if let Some(ref verifier) = self.verifier {
            verifier
                .verify(
                    &digest,
                    &resolved.signature,
                    &resolved.manifest.cert_chain_pem,
                )
                .map_err(|e| InstallError::Crypto(e.to_string()))?;
            debug!("signature verified for {pkg_ref}");
        }

        let name_dir = self
            .install_root
            .join(resolved.manifest.namespace.as_str())
            .join(resolved.manifest.name.as_str());
        clear_existing_versions(&name_dir)?;

        let install_path = name_dir.join(resolved.manifest.version.to_string());

        // Write tarball to temp file then unpack
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &resolved.tarball)?;
        unpack_tarball(tmp.path(), &install_path)?;

        info!("installed {} to {}", pkg_ref, install_path.display());

        Ok(InstalledPackage {
            pkg_ref: pkg_ref.clone(),
            sha256: digest,
            signer: SignerKind::Registry,
            install_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clear_existing_versions_removes_old_version_dirs() {
        let tmp = TempDir::new().unwrap();
        let name_dir = tmp.path().join("acme").join("my-skill");
        // Simulate an old installed version with a file inside
        let old_version = name_dir.join("1.0.0");
        std::fs::create_dir_all(old_version.join("subdir")).unwrap();

        clear_existing_versions(&name_dir).unwrap();

        // Version dir should be gone
        assert!(!old_version.exists());
        // name_dir itself should still exist (we only remove children)
        assert!(name_dir.exists());
    }

    #[test]
    fn clear_existing_versions_is_noop_when_dir_absent() {
        let tmp = TempDir::new().unwrap();
        let name_dir = tmp.path().join("acme").join("my-skill");
        // Should not error even if name_dir doesn't exist
        clear_existing_versions(&name_dir).unwrap();
    }

    #[test]
    fn clear_existing_versions_removes_multiple_version_dirs() {
        let tmp = TempDir::new().unwrap();
        let name_dir = tmp.path().join("acme").join("my-skill");
        std::fs::create_dir_all(name_dir.join("1.0.0")).unwrap();
        std::fs::create_dir_all(name_dir.join("2.0.0")).unwrap();

        clear_existing_versions(&name_dir).unwrap();

        assert!(!name_dir.join("1.0.0").exists());
        assert!(!name_dir.join("2.0.0").exists());
        assert!(name_dir.exists());
    }
}
