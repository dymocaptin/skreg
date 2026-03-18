//! CLI configuration — re-exported from `skreg-core` with CLI-specific helpers.
pub use skreg_core::config::{
    default_config_path, load_config, save_config, CliConfig, ContextConfig, PolicyConfig,
};

use anyhow::Result;

/// Override `cfg.active_context` with `ctx` if provided.
///
/// Validates that the named context exists.
///
/// # Errors
///
/// Returns an error if `ctx` is `Some` but names a context not in `cfg`.
pub fn apply_context(mut cfg: CliConfig, ctx: Option<&str>) -> Result<CliConfig> {
    if let Some(name) = ctx {
        if !cfg.contexts.contains_key(name) {
            anyhow::bail!(
                "context {:?} not found; available: {:?}",
                name,
                cfg.contexts.keys().collect::<Vec<_>>()
            );
        }
        name.clone_into(&mut cfg.active_context);
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn two_context_cfg() -> CliConfig {
        let mut contexts = HashMap::new();
        contexts.insert(
            "default".to_owned(),
            ContextConfig {
                registry: "https://api.skreg.ai".to_owned(),
                namespace: "acme".to_owned(),
                api_key: "key1".to_owned(),
                root_ca_pem: None,
            },
        );
        contexts.insert(
            "local".to_owned(),
            ContextConfig {
                registry: "http://localhost:8080".to_owned(),
                namespace: "devuser".to_owned(),
                api_key: "dev_key".to_owned(),
                root_ca_pem: Some(std::path::PathBuf::from("/home/dev/.skreg/dev/root-ca.pem")),
            },
        );
        CliConfig {
            active_context: "default".to_owned(),
            contexts,
            policy: skreg_core::config::PolicyConfig::default(),
        }
    }

    #[test]
    fn apply_context_none_leaves_active_unchanged() {
        let cfg = two_context_cfg();
        let result = apply_context(cfg, None).unwrap();
        assert_eq!(result.active_context, "default");
    }

    #[test]
    fn apply_context_switches_to_named_context() {
        let cfg = two_context_cfg();
        let result = apply_context(cfg, Some("local")).unwrap();
        assert_eq!(result.active_context, "local");
        assert_eq!(result.registry(), "http://localhost:8080");
    }

    #[test]
    fn apply_context_errors_on_unknown_context() {
        let cfg = two_context_cfg();
        let err = apply_context(cfg, Some("prod")).unwrap_err();
        assert!(err.to_string().contains("prod"));
    }
}
