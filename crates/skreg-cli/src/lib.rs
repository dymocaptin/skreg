//! skreg CLI library — command implementations and install orchestration.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// CLI subcommand implementations.
pub mod commands;
/// CLI configuration — read/write `~/.skreg/config.toml`.
pub mod config;
/// Symlink creation, removal, and tracking in `~/.skreg/links.toml`.
pub mod linker;
/// Publisher key management — auto-keygen and RSA-PSS signing.
pub mod keys;
/// Re-export installer from skreg-client for backwards compatibility.
pub use skreg_client::installer;
