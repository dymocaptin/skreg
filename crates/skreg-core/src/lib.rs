//! Core domain types for the skreg ecosystem.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod config;
pub mod installed;
pub mod limits;
pub mod manifest;
pub mod package_ref;
pub mod types;
pub mod verification;
pub mod version;
pub use verification::VerificationKind;
