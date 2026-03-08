//! CLI configuration — read/write `~/.skreg/config.toml`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Enforcement level for policy checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementLevel {
    /// Surface a hint but take no blocking action.
    Hint,
    /// Prompt the user for confirmation before proceeding.
    #[default]
    Confirm,
    /// Abort immediately without prompting.
    Strict,
}

/// Policy configuration for the CLI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// How policy violations are enforced.
    #[serde(default)]
    pub enforcement: EnforcementLevel,
}

/// Per-context configuration (registry URL, namespace, API key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Base URL of the skill registry.
    pub registry: String,
    /// Authenticated namespace slug.
    pub namespace: String,
    /// Plaintext API key for this namespace.
    pub api_key: String,
}

/// Persisted CLI configuration — supports multiple named contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Name of the currently active context.
    pub active_context: String,
    /// Map of context name to its configuration.
    pub contexts: HashMap<String, ContextConfig>,
    /// Policy enforcement settings.
    #[serde(default)]
    pub policy: PolicyConfig,
}

/// Old flat config format (pre-multi-context) used only during migration.
#[derive(Deserialize)]
struct OldConfig {
    registry: String,
    namespace: String,
    api_key: String,
}

impl CliConfig {
    /// Return the [`ContextConfig`] for the currently active context.
    ///
    /// # Panics
    ///
    /// Panics if `active_context` does not name an entry in `contexts`.
    #[must_use]
    pub fn active_context_config(&self) -> &ContextConfig {
        self.contexts.get(&self.active_context).unwrap_or_else(|| {
            panic!(
                "active context {:?} not found; available: {:?}",
                self.active_context,
                self.contexts.keys().collect::<Vec<_>>()
            )
        })
    }

    /// Registry base URL from the active context.
    #[must_use]
    pub fn registry(&self) -> &str {
        &self.active_context_config().registry
    }

    /// Namespace slug from the active context.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.active_context_config().namespace
    }

    /// API key from the active context.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.active_context_config().api_key
    }

    /// Parse a TOML string, auto-migrating old flat format when needed.
    ///
    /// Tries the new multi-context format first.  If that fails, falls back to
    /// the old flat format (`registry`, `namespace`, `api_key` at top level)
    /// and wraps it as a `[contexts.default]` entry with
    /// `active_context = "default"`.
    ///
    /// # Errors
    ///
    /// Returns an error if neither format can be parsed.
    pub fn from_str_with_migration(s: &str) -> Result<Self> {
        // Try new format first.
        if let Ok(cfg) = toml::from_str::<CliConfig>(s) {
            return Ok(cfg);
        }

        // Fall back to old flat format.
        let old: OldConfig = toml::from_str(s)?;
        let mut contexts = HashMap::new();
        contexts.insert(
            "default".to_owned(),
            ContextConfig {
                registry: old.registry,
                namespace: old.namespace,
                api_key: old.api_key,
            },
        );
        Ok(CliConfig {
            active_context: "default".to_owned(),
            contexts,
            policy: PolicyConfig::default(),
        })
    }
}

/// Return the default path for the CLI config file (`~/.skreg/config.toml`).
#[must_use]
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home).join(".skreg").join("config.toml")
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
/// If the file is in the old flat format it is automatically migrated and
/// rewritten in the new multi-context format before returning.
///
/// # Errors
///
/// Returns an error if the file cannot be read or deserialized.
pub fn load_config(path: &Path) -> Result<CliConfig> {
    let raw = std::fs::read_to_string(path)?;
    let was_old_format = toml::from_str::<CliConfig>(&raw).is_err();
    let cfg = CliConfig::from_str_with_migration(&raw)?;
    if was_old_format {
        save_config(&cfg, path)?;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const OLD: &str = r#"
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"
"#;

    const NEW: &str = r#"
active_context = "default"

[contexts.default]
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"
"#;

    #[test]
    fn parses_new_format() {
        let config: CliConfig = toml::from_str(NEW).unwrap();
        assert_eq!(config.active_context, "default");
        assert_eq!(config.registry(), "https://api.skreg.ai");
        assert_eq!(config.namespace(), "testuser");
        assert_eq!(config.api_key(), "skreg_abc123");
    }

    #[test]
    fn migrates_old_format() {
        let config = CliConfig::from_str_with_migration(OLD).unwrap();
        assert_eq!(config.active_context, "default");
        assert_eq!(config.registry(), "https://api.skreg.ai");
    }

    #[test]
    fn active_context_config_returns_correct_entry() {
        let config: CliConfig = toml::from_str(NEW).unwrap();
        let ctx = config.active_context_config();
        assert_eq!(ctx.namespace, "testuser");
    }

    #[test]
    fn config_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut contexts = HashMap::new();
        contexts.insert(
            "default".to_owned(),
            ContextConfig {
                registry: "https://example.com".to_owned(),
                namespace: "acme".to_owned(),
                api_key: "skreg_abc".to_owned(),
            },
        );
        let cfg = CliConfig {
            active_context: "default".to_owned(),
            contexts,
            policy: PolicyConfig::default(),
        };
        save_config(&cfg, &path).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.api_key(), "skreg_abc");
    }

    #[test]
    fn load_config_rewrites_old_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(&path, OLD).unwrap();
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg.active_context, "default");
        // File should now contain "active_context"
        let rewritten = std::fs::read_to_string(&path).unwrap();
        assert!(rewritten.contains("active_context"));
    }

    #[test]
    fn policy_config_defaults_to_confirm() {
        let toml = r#"
active_context = "default"

[contexts.default]
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"
"#;
        let cfg: CliConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.policy.enforcement, EnforcementLevel::Confirm);
    }

    #[test]
    fn policy_config_parses_strict() {
        let toml = r#"
active_context = "default"

[contexts.default]
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"

[policy]
enforcement = "strict"
"#;
        let cfg: CliConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.policy.enforcement, EnforcementLevel::Strict);
    }
}
