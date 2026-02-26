//! skillpkg CLI library — command implementations and install orchestration.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// CLI subcommand implementations.
pub mod commands;
/// CLI configuration — read/write `~/.skillpkg/config.toml`.
pub mod config;
pub mod installer;
