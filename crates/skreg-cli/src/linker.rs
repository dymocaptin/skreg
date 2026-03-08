//! Linker — symlink creation, removal, and tracking in `~/.skreg/links.toml`.

use anyhow::Result;
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
pub struct Linker {
    links_path: PathBuf,
    file: LinksFile,
}

impl Linker {
    /// Load (or initialise) the linker from `links_path`.
    #[must_use]
    pub fn new(links_path: PathBuf) -> Self {
        let file = if links_path.exists() {
            fs::read_to_string(&links_path)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            LinksFile::default()
        };
        Self { links_path, file }
    }

    /// Create symlinks pointing at `version_dir` inside each `tool_dir`.
    ///
    /// - `tool_dirs[0]` is created with `create_dir_all` when `always_create` is `true`;
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
        always_create: bool,
    ) -> Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        let pkg_key = format!("{ns}/{name}@{version}");

        for (i, tool_dir) in tool_dirs.iter().enumerate() {
            if i == 0 && always_create {
                fs::create_dir_all(tool_dir)?;
            } else if !tool_dir.exists() {
                continue;
            }

            let ns_dir = tool_dir.join(ns);
            fs::create_dir_all(&ns_dir)?;

            let link_path = ns_dir.join(name);

            // Remove stale link/file if present.
            if link_path.exists() || link_path.is_symlink() {
                fs::remove_file(&link_path)?;
            }

            std::os::unix::fs::symlink(version_dir, &link_path)?;

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
                    fs::remove_file(&p)?;
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
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.links_path, toml::to_string(&self.file)?)?;
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
}
