//! Core domain types for the skreg ecosystem.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod config;
pub mod installed;
pub mod manifest;
pub mod package_ref;
pub mod types;
pub mod verification;
pub use verification::VerificationKind;
