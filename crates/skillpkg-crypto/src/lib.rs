//! Cryptographic primitives for skillpkg: signature verification and revocation.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod error;
pub mod revocation;
pub mod verifier;
