//! Application state and top-level event loop.

use anyhow::Result;
use skreg_core::config::CliConfig;

/// Start the TUI event loop.
///
/// # Errors
/// Returns an error if terminal initialisation fails or an I/O error occurs.
#[allow(clippy::unused_async)]
pub async fn run(_config: CliConfig) -> Result<()> {
    println!("skreg TUI — coming soon");
    Ok(())
}
