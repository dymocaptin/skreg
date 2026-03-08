//! Installed package scanning — discovers locally installed packages structured as `{ns}/{name}/{version}/`.

use std::path::{Path, PathBuf};

use dirs;

/// A single installed package entry.
#[derive(Debug)]
pub struct InstalledPkg {
    /// Publisher namespace slug.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Path to the version directory on disk.
    pub path: PathBuf,
}

/// Returns the root directory where skill packages are installed (`~/.skreg/packages/`).
#[must_use]
pub fn packages_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".skreg")
        .join("packages")
}

/// Scan `base` for installed packages structured as `{ns}/{name}/{version}/`.
///
/// Returns an empty `Vec` if `base` does not exist.
///
/// # Errors
///
/// Returns an error if any directory entry cannot be read.
pub fn scan_installed(base: &Path) -> anyhow::Result<Vec<InstalledPkg>> {
    let mut out = Vec::new();
    if !base.exists() {
        return Ok(out);
    }
    for ns_entry in std::fs::read_dir(base)? {
        let ns_entry = ns_entry?;
        if !ns_entry.file_type()?.is_dir() {
            continue;
        }
        let namespace = ns_entry.file_name().to_string_lossy().into_owned();
        for pkg_entry in std::fs::read_dir(ns_entry.path())? {
            let pkg_entry = pkg_entry?;
            if !pkg_entry.file_type()?.is_dir() {
                continue;
            }
            let name = pkg_entry.file_name().to_string_lossy().into_owned();
            for ver_entry in std::fs::read_dir(pkg_entry.path())? {
                let ver_entry = ver_entry?;
                if !ver_entry.file_type()?.is_dir() {
                    continue;
                }
                out.push(InstalledPkg {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    version: ver_entry.file_name().to_string_lossy().into_owned(),
                    path: ver_entry.path(),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn make_entry(tmp: &TempDir, ns: &str, name: &str, ver: &str) {
        fs::create_dir_all(tmp.path().join(ns).join(name).join(ver)).unwrap();
    }

    #[test]
    fn scan_finds_all_installed() {
        let tmp = TempDir::new().unwrap();
        make_entry(&tmp, "dymo", "color-analysis", "1.2.0");
        make_entry(&tmp, "tools", "palette-gen", "2.0.0");
        let pkgs = scan_installed(tmp.path()).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert!(pkgs.iter().any(|p| p.name == "color-analysis"));
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let pkgs = scan_installed(tmp.path()).unwrap();
        assert!(pkgs.is_empty());
    }
}
