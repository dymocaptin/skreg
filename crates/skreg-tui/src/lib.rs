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

/// Run the skreg terminal UI against the given registry.
///
/// # Errors
/// Returns an error if the terminal cannot be initialized or if an I/O error occurs.
#[allow(clippy::unused_async)]
pub async fn run(registry_url: String, namespace: String, api_key: String) -> Result<()> {
    app::run(registry_url, namespace, api_key).await
}
