//! `skreg install` — download, verify, and install a skill.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use skreg_client::client::HttpRegistryClient;
use skreg_core::config::EnforcementLevel;
use skreg_core::package_ref::PackageRef;
use skreg_crypto::verifier::RsaPssVerifier;

use crate::config::{default_config_path, load_config};
use crate::installer::Installer;
use crate::linker::Linker;

fn default_install_root() -> Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".skreg").join("packages"))
}

fn links_path() -> Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".skreg").join("links.toml"))
}

fn claude_md_path() -> Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".claude").join("CLAUDE.md"))
}

/// Candidate tool skill directories, in probe order.
/// Index 0 (~/.agents/skills) is always created if absent.
fn tool_skill_dirs() -> Result<Vec<PathBuf>> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(vec![
        home.join(".agents").join("skills"),
        home.join(".claude").join("skills"),
        home.join(".cursor").join("skills"),
        home.join(".codeium").join("windsurf").join("skills"),
        home.join(".codex").join("skills"),
    ])
}

fn build_claude_md_entries(
    links: &[crate::linker::LinkRecord],
    today: &str,
) -> Vec<crate::linker::ClaudeMdEntry> {
    let mut seen = std::collections::HashSet::new();
    links
        .iter()
        .filter(|r| seen.insert(r.package.clone()))
        .map(|r| crate::linker::ClaudeMdEntry {
            package: r.package.clone(),
            verified_date: today.to_string(),
        })
        .collect()
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
) -> Result<()> {
    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference: {package_ref:?}"))?;

    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;

    // Resolve enforcement level: override > config > default
    let enforcement = enforcement_override.unwrap_or_else(|| cfg.policy.enforcement.clone());

    let client = Arc::new(HttpRegistryClient::new(cfg.registry()));
    let verifier = Arc::new(RsaPssVerifier::new());
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
    let tool_dirs = tool_skill_dirs()?;
    let mut linker = Linker::new(links_path()?);
    let symlinks =
        linker.create_symlinks(ns, name, &version, &result.install_path, &tool_dirs, true)?;

    if !symlinks.is_empty() {
        println!("\nLinked to:");
        for path in &symlinks {
            println!("  {}", path.display());
        }
    }

    // Update ~/.claude/CLAUDE.md if ~/.claude/ exists
    let claude_md = claude_md_path()?;
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
