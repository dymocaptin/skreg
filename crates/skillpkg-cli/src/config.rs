//! CLI configuration — read/write `~/.skillpkg/config.toml`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Persisted CLI configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct CliConfig {
    /// Base URL of the skill registry.
    pub registry:  String,
    /// Authenticated namespace slug.
    pub namespace: String,
    /// Plaintext API key for this namespace.
    pub api_key:   String,
}

/// Return the default path for the CLI config file (`~/.skillpkg/config.toml`).
#[must_use]
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home).join(".skillpkg").join("config.toml")
}

/// Write `cfg` to `path`, creating parent directories if necessary.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or the file cannot be written.
pub fn save_config(cfg: &CliConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string(cfg)?)?;
    Ok(())
}

/// Load and deserialize a [`CliConfig`] from `path`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or deserialized.
pub fn load_config(path: &Path) -> Result<CliConfig> {
    let raw = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = CliConfig {
            registry:  "https://example.com".to_owned(),
            namespace: "acme".to_owned(),
            api_key:   "skreg_abc".to_owned(),
        };
        save_config(&cfg, &path).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.api_key, "skreg_abc");
    }
}
