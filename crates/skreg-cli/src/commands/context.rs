//! `skreg context` — manage registry contexts.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Subcommand;

use crate::config::{
    default_config_path, load_config, save_config, CliConfig, ContextConfig, PolicyConfig,
};

/// Commands for registry context management.
#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Add a new registry context
    Add {
        /// Context name
        name: String,
        /// Registry URL
        #[arg(long)]
        registry: String,
        /// Optional PEM-encoded root CA certificate for the registry.
        #[arg(long, value_name = "FILE")]
        root_ca: Option<PathBuf>,
        /// Do not switch to this context after adding
        #[arg(long)]
        no_activate: bool,
    },
    /// Set the active registry context
    Use {
        /// Context name
        name: String,
    },
    /// List all registry contexts
    List,
    /// Remove a registry context
    Remove {
        /// Context name
        name: String,
    },
}

/// Handle `skreg context <command>`.
///
/// # Errors
///
/// Returns an error if the context operation fails.
pub fn handle(command: ContextCommands) -> Result<()> {
    let path = default_config_path();
    match command {
        ContextCommands::Add {
            name,
            registry,
            root_ca,
            no_activate,
        } => add(&path, &name, registry, root_ca, !no_activate),
        ContextCommands::Use { name } => set_active(&path, &name),
        ContextCommands::List => list(&path, &mut std::io::stdout()),
        ContextCommands::Remove { name } => remove(&path, &name),
    }
}

fn add(
    path: &Path,
    name: &str,
    registry: String,
    root_ca: Option<PathBuf>,
    activate: bool,
) -> Result<()> {
    let root_ca_pem = root_ca;

    let mut cfg = load_config(path).unwrap_or_else(|_| CliConfig {
        active_context: name.to_owned(),
        contexts: HashMap::new(),
        policy: PolicyConfig::default(),
    });

    if cfg.contexts.contains_key(name) {
        log::warn!("Overwriting existing context '{name}'.");
    }

    cfg.add_context(
        name.to_owned(),
        ContextConfig {
            registry,
            namespace: String::new(),
            api_key: String::new(),
            root_ca_pem,
        },
        activate,
    );

    save_config(&cfg, path)?;
    if cfg.active_context == name {
        println!("Context '{name}' added and activated.");
    } else {
        println!("Context '{name}' added.");
    }
    Ok(())
}

fn remove(path: &Path, name: &str) -> Result<()> {
    let mut cfg = load_config(path)?;
    cfg.remove_context(name)?;
    save_config(&cfg, path)?;
    println!("Context '{name}' removed.");
    Ok(())
}

fn set_active(path: &Path, name: &str) -> Result<()> {
    let mut cfg = load_config(path)?;
    cfg.switch_context(name)?;
    save_config(&cfg, path)?;
    println!("Switched to context '{name}'.");
    Ok(())
}

fn list<W: Write>(path: &Path, out: &mut W) -> Result<()> {
    let cfg = load_config(path)?;
    let mut names: Vec<_> = cfg.contexts.keys().collect();
    names.sort();

    for name in names {
        let prefix = if name == &cfg.active_context {
            "*"
        } else {
            " "
        };
        let ctx = &cfg.contexts[name];
        writeln!(
            out,
            "{prefix} {name:<8} {registry}",
            registry = ctx.registry
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn add_new_context() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "test", "https://reg.ai".to_owned(), None, true).unwrap();

        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.active_context, "test");
        assert_eq!(cfg.contexts["test"].registry, "https://reg.ai");
    }

    #[test]
    fn add_context_with_ca() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        let ca_file = NamedTempFile::new().unwrap();
        let ca_pem = "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----";
        std::fs::write(ca_file.path(), ca_pem).unwrap();

        add(
            path,
            "with-ca",
            "https://ca.reg".to_owned(),
            Some(ca_file.path().to_owned()),
            true,
        )
        .unwrap();

        let cfg = load_config(path).unwrap();
        assert_eq!(
            cfg.contexts["with-ca"].root_ca_pem,
            Some(ca_file.path().to_owned())
        );
    }

    #[test]
    fn add_no_activate_keeps_current() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "c1", "https://r1".to_owned(), None, true).unwrap();
        add(path, "c2", "https://r2".to_owned(), None, false).unwrap();

        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.active_context, "c1");
        assert!(cfg.contexts.contains_key("c2"));
    }

    #[test]
    fn switch_context() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "c1", "https://r1".to_owned(), None, true).unwrap();
        add(path, "c2", "https://r2".to_owned(), None, true).unwrap();

        set_active(path, "c1").unwrap();
        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.active_context, "c1");

        set_active(path, "c2").unwrap();
        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.active_context, "c2");
    }

    #[test]
    fn switch_to_nonexistent_fails() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "exists", "https://r".to_owned(), None, true).unwrap();
        assert!(set_active(path, "missing").is_err());
    }

    #[test]
    fn list_contexts() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "a", "https://r-a".to_owned(), None, true).unwrap();
        add(path, "b", "https://r-b".to_owned(), None, false).unwrap();
        set_active(path, "a").unwrap();

        let mut out = Vec::new();
        list(path, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();

        assert!(s.contains("* a        https://r-a"));
        assert!(s.contains("  b        https://r-b"));
    }

    #[test]
    fn remove_context_works() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "keep", "https://r1".to_owned(), None, true).unwrap();
        add(path, "drop", "https://r2".to_owned(), None, false).unwrap();

        remove(path, "drop").unwrap();

        let cfg = load_config(path).unwrap();
        assert!(!cfg.contexts.contains_key("drop"));
        assert_eq!(cfg.active_context, "keep");
    }

    #[test]
    fn remove_active_context_fails() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "active", "https://r".to_owned(), None, true).unwrap();
        assert!(remove(path, "active").is_err());
    }

    #[test]
    fn remove_nonexistent_context_fails() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        add(path, "only", "https://r".to_owned(), None, true).unwrap();
        assert!(remove(path, "ghost").is_err());
    }
}
