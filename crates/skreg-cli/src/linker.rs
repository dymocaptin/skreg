//! Linker — symlink creation, removal, and tracking in `~/.skreg/links.toml`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A single tracked symlink record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRecord {
    /// Package key, e.g. `"dymo/color-analysis@1.0.0"`.
    pub package: String,
    /// Absolute path of the symlink on disk.
    pub path: String,
}

/// Contents of `links.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LinksFile {
    /// All tracked symlinks.
    #[serde(default)]
    pub links: Vec<LinkRecord>,
}

/// Manages symlinks from tool skill directories to installed package directories.
///
/// Loaded from (or initialised at) a `links.toml` file. Call [`Linker::create_symlinks`] to
/// create and track new symlinks, [`Linker::remove_symlinks`] to tear them down, and
/// [`Linker::links`] to inspect the current state. Changes are persisted automatically after
/// each mutating operation.
pub struct Linker {
    links_path: PathBuf,
    file: LinksFile,
}

impl Linker {
    /// Load (or initialise) the linker from `links_path`.
    #[must_use]
    pub fn new(links_path: PathBuf) -> Self {
        let file = if links_path.exists() {
            if let Some(f) = fs::read_to_string(&links_path)
                .ok()
                .and_then(|s| toml::from_str::<LinksFile>(&s).ok())
            {
                f
            } else {
                // File exists but could not be read or parsed — warn and reset.
                if let Ok(raw) = fs::read_to_string(&links_path) {
                    let err = toml::from_str::<LinksFile>(&raw)
                        .err()
                        .map_or_else(|| "unknown error".to_string(), |e| e.to_string());
                    eprintln!(
                        "skreg: warning: corrupt links.toml at {}: {}",
                        links_path.display(),
                        err
                    );
                } else {
                    eprintln!(
                        "skreg: warning: corrupt links.toml at {}: could not read file",
                        links_path.display()
                    );
                }
                LinksFile::default()
            }
        } else {
            LinksFile::default()
        };
        Self { links_path, file }
    }

    /// Create symlinks pointing at `version_dir` inside each `tool_dir`.
    ///
    /// - `tool_dirs[0]` is created with `create_dir_all` when `ensure_primary_dir` is `true`;
    ///   all other tool dirs are skipped if they do not already exist.
    /// - Returns the list of symlink paths that were created.
    ///
    /// # Errors
    ///
    /// Returns an error if any filesystem operation (directory creation, symlink creation,
    /// stale-link removal, or saving `links.toml`) fails.
    pub fn create_symlinks(
        &mut self,
        ns: &str,
        name: &str,
        version: &str,
        version_dir: &Path,
        tool_dirs: &[PathBuf],
        ensure_primary_dir: bool,
    ) -> Result<Vec<PathBuf>> {
        anyhow::ensure!(
            version_dir.is_dir(),
            "version_dir does not exist: {}",
            version_dir.display()
        );

        let mut created = Vec::new();
        let pkg_key = format!("{ns}/{name}@{version}");

        // Remove stale records for this package before adding new ones.
        self.file.links.retain(|r| r.package != pkg_key);

        for (i, tool_dir) in tool_dirs.iter().enumerate() {
            if i == 0 && ensure_primary_dir {
                fs::create_dir_all(tool_dir)
                    .with_context(|| format!("failed to create tool dir {}", tool_dir.display()))?;
            } else if !tool_dir.exists() {
                continue;
            }

            let ns_dir = tool_dir.join(ns);
            fs::create_dir_all(&ns_dir)
                .with_context(|| format!("failed to create namespace dir {}", ns_dir.display()))?;

            let link_path = ns_dir.join(name);

            // Remove stale link/file if present.
            if link_path.exists() || link_path.is_symlink() {
                fs::remove_file(&link_path).with_context(|| {
                    format!("failed to remove stale symlink {}", link_path.display())
                })?;
            }

            std::os::unix::fs::symlink(version_dir, &link_path)
                .with_context(|| format!("failed to create symlink {}", link_path.display()))?;

            self.file.links.push(LinkRecord {
                package: pkg_key.clone(),
                path: link_path.to_string_lossy().into_owned(),
            });

            created.push(link_path);
        }

        self.save()?;
        Ok(created)
    }

    /// Remove all symlinks tracked for `pkg_key` and purge their records.
    ///
    /// Returns the number of symlinks removed.
    ///
    /// # Errors
    ///
    /// Returns an error if removing a symlink from the filesystem or saving `links.toml` fails.
    pub fn remove_symlinks(&mut self, pkg_key: &str) -> Result<usize> {
        let mut removed = 0;
        let mut kept = Vec::new();

        for record in self.file.links.drain(..) {
            if record.package == pkg_key {
                let p = PathBuf::from(&record.path);
                if p.exists() || p.is_symlink() {
                    fs::remove_file(&p)
                        .with_context(|| format!("failed to remove symlink {}", p.display()))?;
                }
                removed += 1;
            } else {
                kept.push(record);
            }
        }

        self.file.links = kept;
        self.save()?;
        Ok(removed)
    }

    /// Return all tracked link records.
    #[must_use]
    pub fn links(&self) -> &[LinkRecord] {
        &self.file.links
    }

    /// Persist `links.toml` to disk.
    fn save(&self) -> Result<()> {
        if let Some(parent) = self.links_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
        }
        fs::write(
            &self.links_path,
            toml::to_string(&self.file)
                .with_context(|| "failed to serialize links.toml".to_string())?,
        )
        .with_context(|| format!("failed to write links.toml {}", self.links_path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_version_dir(tmp: &TempDir, ns: &str, name: &str, ver: &str) -> PathBuf {
        let p = tmp.path().join("packages").join(ns).join(name).join(ver);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn link_record_roundtrips_toml() {
        let record = LinkRecord {
            package: "dymo/color-analysis@1.0.0".to_string(),
            path: "/home/dymo/.claude/skills/dymo/color-analysis".to_string(),
        };
        let s = toml::to_string(&LinksFile {
            links: vec![record.clone()],
        })
        .unwrap();
        let back: LinksFile = toml::from_str(&s).unwrap();
        assert_eq!(back.links[0].package, record.package);
        assert_eq!(back.links[0].path, record.path);
    }

    #[test]
    fn create_symlinks_links_into_existing_tool_dirs() {
        let tmp = TempDir::new().unwrap();
        let version_dir = make_version_dir(&tmp, "dymo", "color-analysis", "1.0.0");

        let agents_skills = tmp.path().join("agents_skills");
        fs::create_dir_all(&agents_skills).unwrap();

        let claude_skills = tmp.path().join("claude_skills");
        fs::create_dir_all(&claude_skills).unwrap();

        // cursor_skills does NOT exist — should not be linked
        let tool_dirs = vec![
            agents_skills.clone(),
            claude_skills.clone(),
            tmp.path().join("cursor_skills"),
        ];

        let links_path = tmp.path().join("links.toml");
        let mut linker = Linker::new(links_path.clone());

        linker
            .create_symlinks(
                "dymo",
                "color-analysis",
                "1.0.0",
                &version_dir,
                &tool_dirs,
                true,
            )
            .unwrap();

        assert!(agents_skills.join("dymo").join("color-analysis").exists());
        assert!(claude_skills.join("dymo").join("color-analysis").exists());
        assert!(!tmp
            .path()
            .join("cursor_skills")
            .join("dymo")
            .join("color-analysis")
            .exists());

        let file: LinksFile = toml::from_str(&fs::read_to_string(&links_path).unwrap()).unwrap();
        assert_eq!(file.links.len(), 2);
    }

    #[test]
    fn remove_symlinks_removes_tracked_links() {
        let tmp = TempDir::new().unwrap();
        let version_dir = make_version_dir(&tmp, "dymo", "color-analysis", "1.0.0");

        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let tool_dirs = vec![skills_dir.clone()];
        let links_path = tmp.path().join("links.toml");
        let mut linker = Linker::new(links_path.clone());

        linker
            .create_symlinks(
                "dymo",
                "color-analysis",
                "1.0.0",
                &version_dir,
                &tool_dirs,
                true,
            )
            .unwrap();
        assert!(skills_dir.join("dymo").join("color-analysis").is_symlink());

        linker.remove_symlinks("dymo/color-analysis@1.0.0").unwrap();

        assert!(!skills_dir.join("dymo").join("color-analysis").exists());
        let file: LinksFile = toml::from_str(&fs::read_to_string(&links_path).unwrap()).unwrap();
        assert!(file.links.is_empty());
    }

    #[test]
    fn remove_symlinks_is_noop_for_unknown_package() {
        let tmp = TempDir::new().unwrap();
        let links_path = tmp.path().join("links.toml");
        let mut linker = Linker::new(links_path);
        linker.remove_symlinks("acme/nonexistent@1.0.0").unwrap();
    }

    #[test]
    fn create_symlinks_replaces_existing_symlink() {
        let tmp = TempDir::new().unwrap();
        let version_dir = make_version_dir(&tmp, "dymo", "color-analysis", "1.0.0");

        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let tool_dirs = vec![skills_dir.clone()];
        let links_path = tmp.path().join("links.toml");
        let mut linker = Linker::new(links_path.clone());

        // First install.
        linker
            .create_symlinks(
                "dymo",
                "color-analysis",
                "1.0.0",
                &version_dir,
                &tool_dirs,
                true,
            )
            .unwrap();

        // Second install of the same package (reinstall).
        linker
            .create_symlinks(
                "dymo",
                "color-analysis",
                "1.0.0",
                &version_dir,
                &tool_dirs,
                true,
            )
            .unwrap();

        let link_path = skills_dir.join("dymo").join("color-analysis");
        assert!(
            link_path.is_symlink(),
            "symlink should exist after reinstall"
        );
        assert_eq!(
            fs::read_link(&link_path).unwrap(),
            version_dir,
            "symlink should point to correct target"
        );

        let file: LinksFile = toml::from_str(&fs::read_to_string(&links_path).unwrap()).unwrap();
        assert_eq!(
            file.links.len(),
            1,
            "links.toml should have exactly one entry, not two"
        );
    }

    #[test]
    fn create_symlinks_replaces_broken_symlink() {
        let tmp = TempDir::new().unwrap();
        let version_dir = make_version_dir(&tmp, "dymo", "color-analysis", "1.0.0");

        let skills_dir = tmp.path().join("skills");
        let ns_dir = skills_dir.join("dymo");
        fs::create_dir_all(&ns_dir).unwrap();

        // Manually create a dangling symlink at the expected location.
        let link_path = ns_dir.join("color-analysis");
        let nonexistent = tmp.path().join("does-not-exist");
        std::os::unix::fs::symlink(&nonexistent, &link_path).unwrap();
        assert!(
            link_path.is_symlink(),
            "pre-condition: dangling symlink exists"
        );
        assert!(!link_path.exists(), "pre-condition: target does not exist");

        let tool_dirs = vec![skills_dir.clone()];
        let links_path = tmp.path().join("links.toml");
        let mut linker = Linker::new(links_path.clone());

        linker
            .create_symlinks(
                "dymo",
                "color-analysis",
                "1.0.0",
                &version_dir,
                &tool_dirs,
                true,
            )
            .unwrap();

        assert!(
            link_path.is_symlink(),
            "symlink should exist after replacement"
        );
        assert!(link_path.exists(), "symlink target should now be valid");
        assert_eq!(
            fs::read_link(&link_path).unwrap(),
            version_dir,
            "symlink should point to correct target"
        );
    }
}
