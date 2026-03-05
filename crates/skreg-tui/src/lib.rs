//! Terminal UI for the skreg package registry.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// Application state and main event loop.
pub mod app;
/// Visual theme — colours and styles.
pub mod theme;
/// Full-screen views (package list, detail, etc.).
pub mod views;
/// Reusable UI widgets (header, footer, toast, etc.).
pub mod widgets;

use anyhow::Result;
use skreg_core::config::CliConfig;

/// Run the skreg terminal UI with the given configuration.
///
/// # Errors
/// Returns an error if the terminal cannot be initialized or if an I/O error occurs.
pub fn run(config: CliConfig) -> Result<()> {
    app::run(config)
}
