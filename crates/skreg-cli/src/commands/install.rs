//! `skreg install` — download, verify, and install a skill.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use skreg_client::client::HttpRegistryClient;
use skreg_core::package_ref::PackageRef;
use skreg_crypto::verifier::RsaPkcs1Verifier;

use crate::config::{default_config_path, load_config};
use crate::installer::Installer;

fn default_install_root() -> Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".skreg").join("packages"))
}

/// Run `skreg install <package_ref>`.
///
/// `package_ref` is in the form `namespace/name` or `namespace/name@semver`.
///
/// # Errors
///
/// Returns an error if the package reference is invalid, download fails,
/// verification fails, or extraction fails.
pub async fn run_install(package_ref: &str) -> Result<()> {
    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference: {package_ref:?}"))?;

    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;

    let client = Arc::new(HttpRegistryClient::new(cfg.registry()));
    let verifier = Arc::new(RsaPkcs1Verifier::new());
    let install_root = default_install_root()?;

    let installer = Installer::new(client, install_root).with_verifier(verifier);
    let result = installer.install(&pkg_ref).await?;

    println!("Installed {} to {}", pkg_ref, result.install_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn install_module_compiles() {}
}
