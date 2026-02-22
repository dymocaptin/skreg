//! API server configuration loaded from environment variables.

use std::env;

use thiserror::Error;

/// Errors during configuration loading.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required environment variable is missing.
    #[error("missing required environment variable: {0}")]
    Missing(String),
}

/// API server runtime configuration.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// `PostgreSQL` connection URL.
    pub database_url: String,
    /// TCP address to bind (e.g. `0.0.0.0:8080`).
    pub bind_addr: String,
}

impl ApiConfig {
    /// Load configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Missing`] if `DATABASE_URL` is not set.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| ConfigError::Missing("DATABASE_URL".to_owned()))?,
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        })
    }
}
