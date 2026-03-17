//! Orchestrates the full package install pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use log::{debug, info};
use sha2::{Digest, Sha256};
use thiserror::Error;

use skreg_core::installed::{InstalledPackage, SignerKind};
use skreg_core::manifest::Manifest;
use skreg_core::package_ref::PackageRef;
use skreg_core::types::Sha256Digest;
use skreg_crypto::verifier::SignatureVerifier;
use skreg_pack::unpack::unpack_tarball;

use crate::client::RegistryClient;
use crate::error::ClientError;

/// Errors that can occur during package installation.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The registry client returned an error.
    #[error("registry error: {0}")]
    Registry(#[from] ClientError),
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
    /// Returns the installed package descriptor and manifest on success.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError`] if any step fails. On extraction failure, the
    /// partially-created installation directory may remain on disk.
    pub async fn install(
        &self,
        pkg_ref: &PackageRef,
    ) -> Result<(InstalledPackage, Manifest), InstallError> {
        info!("installing {pkg_ref}");

        let resolved = self.client.resolve(pkg_ref).await?;

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

        let digest = Sha256Digest::from_hex(&actual_hex)?;

        if let Some(ref verifier) = self.verifier {
            let tarball_manifest = skreg_pack::unpack::read_manifest_from_bytes(&resolved.tarball)?;
            let sig_hex = tarball_manifest
                .publisher_sig_hex
                .as_deref()
                .ok_or_else(|| {
                    InstallError::Crypto("package is missing publisher_sig_hex".to_owned())
                })?;
            let sig_bytes = hex::decode(sig_hex)
                .map_err(|e| InstallError::Crypto(format!("invalid publisher_sig_hex: {e}")))?;
            // publisher_sig_hex was signed over hash(tarball_1) = tarball_manifest.sha256,
            // not over hash(tarball_2) which is what `digest` holds.
            verifier
                .verify(
                    &tarball_manifest.sha256,
                    &sig_bytes,
                    &tarball_manifest.cert_chain_pem,
                )
                .map_err(|e| InstallError::Crypto(e.to_string()))?;
            debug!("publisher signature verified for {pkg_ref}");
        }

        let install_path = self
            .install_root
            .join(resolved.manifest.namespace.as_str())
            .join(resolved.manifest.name.as_str())
            .join(resolved.manifest.version.to_string());

        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &resolved.tarball)?;
        unpack_tarball(tmp.path(), &install_path)?;

        info!("installed {} to {}", pkg_ref, install_path.display());

        let manifest = resolved.manifest;
        Ok((
            InstalledPackage {
                pkg_ref: pkg_ref.clone(),
                sha256: digest,
                signer: SignerKind::Registry,
                install_path,
            },
            manifest,
        ))
    }
}
