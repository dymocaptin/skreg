//! skreg vetting worker library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// Thin async SMTP send helper.
pub mod email;
/// Job runner: pg_notify listener and pipeline dispatch.
pub mod runner;
/// Vetting pipeline stages.
pub mod stages;
