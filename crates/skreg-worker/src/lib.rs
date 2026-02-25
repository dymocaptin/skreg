//! skreg vetting worker library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// Job runner: pg_notify listener and pipeline dispatch.
pub mod runner;
/// Vetting pipeline stages.
pub mod stages;
