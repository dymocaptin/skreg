//! skreg registry API library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// API key and OTP generation and hashing utilities.
pub mod auth;
/// Auth helpers: Bearer token extraction and namespace resolution.
pub mod middleware;
pub mod config;
pub mod db;
pub mod handlers;
pub mod models;
pub mod router;
