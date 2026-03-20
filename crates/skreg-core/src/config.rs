//! CLI configuration — read/write `~/.skreg/config.toml`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Enforcement level for policy checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// How policy violations are enforced.
    #[serde(default)]
    pub enforcement: EnforcementLevel,
}

/// Per-context configuration (registry URL, namespace, API key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextConfig {
    /// Base URL of the skill registry.
    pub registry: String,
    /// Authenticated namespace slug.
    pub namespace: String,
    /// Plaintext API key for this namespace.
    pub api_key: String,
    /// Optional PEM-encoded root CA certificate for the registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ca_pem: Option<String>,
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
    /// Adds or updates a context. If it's the first context or `activate` is true, sets it as active.
    pub fn add_context(&mut self, name: String, config: ContextConfig, activate: bool) {
        let is_first = self.contexts.is_empty();
        self.contexts.insert(name.clone(), config);
        if is_first || activate {
            self.active_context = name;
        }
    }

    /// Remove a context by name.
    ///
    /// # Errors
    ///
    /// Returns an error if the context does not exist or is currently active.
    pub fn remove_context(&mut self, name: &str) -> Result<()> {
        if !self.contexts.contains_key(name) {
            anyhow::bail!("context '{name}' not found");
        }
        if name == self.active_context {
            anyhow::bail!("cannot remove active context '{name}'; switch to another context first");
        }
        self.contexts.remove(name);
        Ok(())
    }

    /// Switches the active context.
    ///
    /// # Errors
    ///
    /// Returns an error if the named context does not exist.
    pub fn switch_context(&mut self, name: &str) -> Result<()> {
        if !self.contexts.contains_key(name) {
            anyhow::bail!("context '{name}' not found");
        }
        name.clone_into(&mut self.active_context);
        Ok(())
    }

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
                root_ca_pem: None,
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
                root_ca_pem: Some("---BEGIN CERT---".to_owned()),
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
        assert_eq!(
            loaded.active_context_config().root_ca_pem,
            Some("---BEGIN CERT---".to_owned())
        );
    }

    #[test]
    fn add_context_sets_active_if_first() {
        let mut config = CliConfig {
            active_context: String::new(),
            contexts: HashMap::new(),
            policy: PolicyConfig::default(),
        };
        let ctx = ContextConfig {
            registry: "r".into(),
            namespace: "n".into(),
            api_key: "k".into(),
            root_ca_pem: None,
        };
        config.add_context("new".into(), ctx.clone(), false);
        assert_eq!(config.active_context, "new");
        assert_eq!(config.contexts.get("new"), Some(&ctx));
    }

    #[test]
    fn add_context_activates_if_requested() {
        let mut contexts = HashMap::new();
        contexts.insert(
            "old".into(),
            ContextConfig {
                registry: "r1".into(),
                namespace: "n1".into(),
                api_key: "k1".into(),
                root_ca_pem: None,
            },
        );
        let mut config = CliConfig {
            active_context: "old".into(),
            contexts,
            policy: PolicyConfig::default(),
        };
        let ctx2 = ContextConfig {
            registry: "r2".into(),
            namespace: "n2".into(),
            api_key: "k2".into(),
            root_ca_pem: None,
        };
        config.add_context("new".into(), ctx2, true);
        assert_eq!(config.active_context, "new");
    }

    #[test]
    fn switch_context_updates_active() {
        let mut contexts = HashMap::new();
        contexts.insert(
            "c1".into(),
            ContextConfig {
                registry: "r1".into(),
                namespace: "n1".into(),
                api_key: "k1".into(),
                root_ca_pem: None,
            },
        );
        contexts.insert(
            "c2".into(),
            ContextConfig {
                registry: "r2".into(),
                namespace: "n2".into(),
                api_key: "k2".into(),
                root_ca_pem: None,
            },
        );
        let mut config = CliConfig {
            active_context: "c1".into(),
            contexts,
            policy: PolicyConfig::default(),
        };
        config.switch_context("c2").unwrap();
        assert_eq!(config.active_context, "c2");
    }

    #[test]
    fn switch_context_fails_if_missing() {
        let mut config = CliConfig {
            active_context: "default".into(),
            contexts: HashMap::new(),
            policy: PolicyConfig::default(),
        };
        assert!(config.switch_context("nonexistent").is_err());
    }

    #[test]
    fn remove_context_succeeds() {
        let mut contexts = HashMap::new();
        contexts.insert(
            "active".into(),
            ContextConfig {
                registry: "r1".into(),
                namespace: "n1".into(),
                api_key: "k1".into(),
                root_ca_pem: None,
            },
        );
        contexts.insert(
            "other".into(),
            ContextConfig {
                registry: "r2".into(),
                namespace: "n2".into(),
                api_key: "k2".into(),
                root_ca_pem: None,
            },
        );
        let mut config = CliConfig {
            active_context: "active".into(),
            contexts,
            policy: PolicyConfig::default(),
        };
        config.remove_context("other").unwrap();
        assert!(!config.contexts.contains_key("other"));
        assert_eq!(config.active_context, "active");
    }

    #[test]
    fn remove_active_context_fails() {
        let mut contexts = HashMap::new();
        contexts.insert(
            "active".into(),
            ContextConfig {
                registry: "r1".into(),
                namespace: "n1".into(),
                api_key: "k1".into(),
                root_ca_pem: None,
            },
        );
        let mut config = CliConfig {
            active_context: "active".into(),
            contexts,
            policy: PolicyConfig::default(),
        };
        assert!(config.remove_context("active").is_err());
    }

    #[test]
    fn remove_nonexistent_context_fails() {
        let mut config = CliConfig {
            active_context: "default".into(),
            contexts: HashMap::new(),
            policy: PolicyConfig::default(),
        };
        assert!(config.remove_context("ghost").is_err());
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

    #[test]
    fn policy_config_parses_hint() {
        let toml = r#"
active_context = "default"

[contexts.default]
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"

[policy]
enforcement = "hint"
"#;
        let cfg: CliConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.policy.enforcement, EnforcementLevel::Hint);
    }

    #[test]
    fn policy_config_parses_explicit_confirm() {
        let toml = r#"
active_context = "default"

[contexts.default]
registry = "https://api.skreg.ai"
namespace = "testuser"
api_key = "skreg_abc123"

[policy]
enforcement = "confirm"
"#;
        let cfg: CliConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.policy.enforcement, EnforcementLevel::Confirm);
    }

    #[test]
    fn config_roundtrip_preserves_policy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut contexts = HashMap::new();
        contexts.insert(
            "default".to_owned(),
            ContextConfig {
                registry: "https://example.com".to_owned(),
                namespace: "acme".to_owned(),
                api_key: "skreg_abc".to_owned(),
                root_ca_pem: None,
            },
        );
        let cfg = CliConfig {
            active_context: "default".to_owned(),
            contexts,
            policy: PolicyConfig {
                enforcement: EnforcementLevel::Strict,
            },
        };
        save_config(&cfg, &path).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.policy, cfg.policy);
    }
}
