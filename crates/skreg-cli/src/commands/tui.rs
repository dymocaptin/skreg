//! `skreg tui` subcommand — launches the interactive terminal UI.

use anyhow::Result;

use skreg_core::config::{default_config_path, load_config};

/// Load configuration and run the TUI event loop.
///
/// # Errors
///
/// Returns an error if the config cannot be loaded or if the terminal fails to
/// initialise.
pub async fn run_tui() -> Result<()> {
    let config = load_config(&default_config_path())?;
    skreg_tui::run(config).await
}
