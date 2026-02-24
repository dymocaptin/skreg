//! skreg registry API library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

/// API key and OTP generation and hashing utilities.
pub mod auth;
pub mod config;
pub mod db;
pub mod handlers;
pub mod models;
pub mod router;
