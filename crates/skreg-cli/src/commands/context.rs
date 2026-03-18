//! `skreg context` — manage registry contexts.

use std::collections::HashMap;
use std::path::PathBuf;

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
    },
    /// Set the active registry context
    Use {
        /// Context name
        name: String,
    },
    /// List all registry contexts
    List,
}

/// Handle `skreg context <command>`.
///
/// # Errors
///
/// Returns an error if the context operation fails.
pub fn handle(command: ContextCommands) -> Result<()> {
    match command {
        ContextCommands::Add {
            name,
            registry,
            root_ca,
        } => add(&name, registry, root_ca),
        ContextCommands::Use { name } => set_active(name),
        ContextCommands::List => list(),
    }
}

fn add(name: &str, registry: String, root_ca: Option<PathBuf>) -> Result<()> {
    let root_ca_pem = if let Some(path) = root_ca {
        let pem = std::fs::read_to_string(&path)?;
        if !pem.contains("BEGIN CERTIFICATE") {
            anyhow::bail!("file does not look like a PEM certificate");
        }
        Some(pem)
    } else {
        None
    };

    let path = default_config_path();
    let mut cfg = load_config(&path).unwrap_or_else(|_| CliConfig {
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
        true,
    );

    save_config(&cfg, &path)?;
    println!("Context '{name}' added and activated.");
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn set_active(_name: String) -> Result<()> {
    // Placeholder implementation for switching the active context.
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn list() -> Result<()> {
    // Placeholder implementation for listing all contexts.
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn context_module_compiles() {}
}
