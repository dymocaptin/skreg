//! Vetting pipeline stages.

pub mod content;
pub mod safety;
pub mod signing;
pub mod structure;

use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SmClient;
use sqlx::PgPool;
use uuid::Uuid;

/// Run the full vetting pipeline for `job_id`.
///
/// Stages 2–4 are implemented in Tasks 10–12.
///
/// # Errors
///
/// Returns an error if any stage fails or a database/S3 operation fails.
pub async fn run_pipeline(
    _job_id: Uuid,
    _pool: &PgPool,
    _s3: &S3Client,
    _sm: &SmClient,
    _bucket: &str,
    _ca_secret_arn: &str,
) -> Result<()> {
    // Implemented in Tasks 10–12
    todo!()
}
