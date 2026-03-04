//! Application state and top-level event loop.

use anyhow::Result;

/// Start the TUI event loop.
///
/// # Errors
/// Returns an error if terminal initialisation fails or an I/O error occurs.
#[allow(clippy::unused_async)]
pub async fn run(
    _registry_url: String,
    _namespace: String,
    _api_key: String,
) -> Result<()> {
    println!("skreg TUI — coming soon");
    Ok(())
}
