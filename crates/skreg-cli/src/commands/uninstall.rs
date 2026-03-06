//! `skreg uninstall` — remove an installed skill.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use skreg_core::package_ref::PackageRef;

fn default_install_root() -> Result<PathBuf> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".skreg").join("packages"))
}

/// Remove an installed package given its `namespace/name` reference and an
/// explicit install root (used by tests).
///
/// # Errors
///
/// Returns an error if the package is not installed or removal fails.
pub fn run_uninstall_with_root(package_ref: &str, install_root: &Path) -> Result<()> {
    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference: {package_ref:?}"))?;

    let name_dir = install_root
        .join(pkg_ref.namespace.as_str())
        .join(pkg_ref.name.as_str());

    if !name_dir.exists() {
        anyhow::bail!("{pkg_ref} is not installed");
    }

    // Find the single version directory
    let version_dir = std::fs::read_dir(&name_dir)
        .with_context(|| format!("failed to read {}", name_dir.display()))?
        .filter_map(std::result::Result::ok)
        .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .ok_or_else(|| anyhow::anyhow!("{pkg_ref} is not installed"))?;

    let version = version_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    std::fs::remove_dir_all(&version_dir)
        .with_context(|| format!("failed to remove {}", version_dir.display()))?;

    // Best-effort cleanup of empty parent dirs
    if std::fs::read_dir(&name_dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(&name_dir);
        let ns_dir = install_root.join(pkg_ref.namespace.as_str());
        if std::fs::read_dir(&ns_dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(&ns_dir);
        }
    }

    println!("Uninstalled {pkg_ref}@{version}");
    Ok(())
}

/// Run `skreg uninstall <package_ref>`.
///
/// # Errors
///
/// Returns an error if the package is not installed or removal fails.
pub fn run_uninstall(package_ref: &str) -> Result<()> {
    let install_root = default_install_root()?;
    run_uninstall_with_root(package_ref, &install_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_installed(root: &Path, ns: &str, name: &str, version: &str) -> PathBuf {
        let version_dir = root.join(ns).join(name).join(version);
        std::fs::create_dir_all(&version_dir).unwrap();
        version_dir
    }

    #[test]
    fn uninstall_removes_version_dir_and_cleans_empty_parents() {
        let tmp = TempDir::new().unwrap();
        let version_dir = make_installed(tmp.path(), "acme", "my-skill", "1.0.0");

        run_uninstall_with_root("acme/my-skill", tmp.path()).unwrap();

        assert!(!version_dir.exists(), "version dir should be gone");
        assert!(
            !tmp.path().join("acme").join("my-skill").exists(),
            "empty name dir should be cleaned up"
        );
        assert!(
            !tmp.path().join("acme").exists(),
            "empty namespace dir should be cleaned up"
        );
    }

    #[test]
    fn uninstall_leaves_nonempty_namespace_dir() {
        let tmp = TempDir::new().unwrap();
        make_installed(tmp.path(), "acme", "my-skill", "1.0.0");
        make_installed(tmp.path(), "acme", "other-skill", "2.0.0");

        run_uninstall_with_root("acme/my-skill", tmp.path()).unwrap();

        assert!(!tmp.path().join("acme").join("my-skill").exists());
        assert!(tmp.path().join("acme").join("other-skill").exists());
        assert!(tmp.path().join("acme").exists());
    }

    #[test]
    fn uninstall_errors_when_not_installed() {
        let tmp = TempDir::new().unwrap();
        let result = run_uninstall_with_root("acme/my-skill", tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not installed"), "error was: {msg}");
    }

    #[test]
    fn uninstall_rejects_invalid_package_ref() {
        let tmp = TempDir::new().unwrap();
        let result = run_uninstall_with_root("notavalidref", tmp.path());
        assert!(result.is_err());
    }
}
