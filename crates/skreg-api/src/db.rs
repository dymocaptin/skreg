//! Database connection pool initialisation.

use sqlx::PgPool;
use thiserror::Error;

/// Errors that can occur during database initialisation.
#[derive(Debug, Error)]
pub enum DbError {
    /// SQLx returned an error connecting or migrating.
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// Migration error.
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Create a connection pool and run pending migrations.
///
/// # Errors
///
/// Returns [`DbError`] if the pool cannot be created or migrations fail.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
