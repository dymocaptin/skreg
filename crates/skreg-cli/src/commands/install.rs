//! `skreg install` — download, verify, and install a skill.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use skreg_client::client::HttpRegistryClient;
use skreg_core::config::EnforcementLevel;
use skreg_core::package_ref::PackageRef;
use skreg_crypto::verifier::{RsaPssVerifier, SignatureVerifier};

use crate::config::{default_config_path, load_config};
use crate::installer::Installer;
use skreg_client::linker::{
    build_claude_md_entries, default_claude_md_path, default_links_path, default_tool_skill_dirs,
    Linker,
};

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
pub async fn run_install(
    package_ref: &str,
    enforcement_override: Option<EnforcementLevel>,
    context: Option<&str>,
) -> Result<()> {
    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference: {package_ref:?}"))?;

    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;
    let cfg = crate::config::apply_context(cfg, context)?;

    // Resolve enforcement level: override > config > default
    let enforcement = enforcement_override.unwrap_or_else(|| cfg.policy.enforcement.clone());

    let client = Arc::new(HttpRegistryClient::new(cfg.registry()));

    // Use custom root CA if the active context specifies one.
    let verifier: Arc<dyn skreg_crypto::verifier::SignatureVerifier> = {
        let ctx_cfg = cfg.active_context_config();
        if let Some(ref ca_path) = ctx_cfg.root_ca_pem {
            // Expand leading ~ manually since std::fs doesn't do tilde expansion.
            let expanded = if ca_path.starts_with("~") {
                let home = home::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
                let suffix = ca_path
                    .strip_prefix("~")
                    .map_err(|_| anyhow::anyhow!("failed to strip ~ prefix from path"))?;
                home.join(suffix)
            } else {
                ca_path.clone()
            };
            let pem = std::fs::read(&expanded)
                .with_context(|| format!("reading root CA from {}", expanded.display()))?;
            Arc::new(RsaPssVerifier::new_with_root_pem(&pem))
        } else {
            Arc::new(RsaPssVerifier::new())
        }
    };
    let install_root = default_install_root()?;

    let installer = Installer::new(client, install_root).with_verifier(verifier);
    let (result, manifest) = installer.install(&pkg_ref).await?;

    let ns = result.pkg_ref.namespace.as_str();
    let name = result.pkg_ref.name.as_str();
    let version = result
        .install_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let pkg_key = format!("{ns}/{name}@{version}");

    println!("✓ Verified {pkg_key}");
    println!("✓ Installed to {}", result.install_path.display());

    // Symlink into tool directories
    let tool_dirs = default_tool_skill_dirs()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let links_path =
        default_links_path().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let mut linker = Linker::new(links_path);
    let symlinks =
        linker.create_symlinks(ns, name, &version, &result.install_path, &tool_dirs, true)?;

    if !symlinks.is_empty() {
        println!("\nLinked to:");
        for path in &symlinks {
            println!("  {}", path.display());
        }
    }

    // Update ~/.claude/CLAUDE.md if ~/.claude/ exists
    let claude_md = default_claude_md_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    if claude_md.parent().is_some_and(std::path::Path::exists) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let entries = build_claude_md_entries(linker.links(), &today);
        linker.write_claude_md(&claude_md, &entries, &enforcement)?;
        println!("\nUpdated {}", claude_md.display());
    }

    let tier = if manifest.cert_chain_pem.len() >= 2 {
        "publisher"
    } else {
        "self-signed"
    };
    let ca_note = if manifest.cert_chain_pem.len() >= 2 {
        ", verified by skreg CA"
    } else {
        ", key not CA-verified"
    };
    println!(
        "  Verification: {tier} (signed by {}{ca_note})",
        manifest.namespace
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn install_module_compiles() {}
}
