//! Job runner: listens on pg_notify("vetting_jobs") and dispatches stage pipeline.

use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SmClient;
use log::{error, info};
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use uuid::Uuid;

use crate::stages::run_pipeline;

/// Start the `pg_notify` listener loop. Blocks indefinitely.
///
/// # Errors
///
/// Returns an error if the initial database connection or listener setup fails.
pub async fn run(
    pool: PgPool,
    s3: S3Client,
    sm: SmClient,
    bucket: String,
    ca_secret_arn: String,
) -> Result<()> {
    let mut listener = PgListener::connect_with(&pool).await?;
    listener.listen("vetting_jobs").await?;
    info!("worker listening on vetting_jobs channel");

    loop {
        let notification = listener.recv().await?;
        let payload = notification.payload();
        match Uuid::parse_str(payload) {
            Ok(job_id) => {
                let pool2 = pool.clone();
                let s3_2 = s3.clone();
                let sm2 = sm.clone();
                let bucket2 = bucket.clone();
                let ca_arn2 = ca_secret_arn.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        process_job(job_id, &pool2, &s3_2, &sm2, &bucket2, &ca_arn2).await
                    {
                        error!("job {job_id} failed: {e}");
                    }
                });
            }
            Err(e) => error!("invalid job_id payload '{payload}': {e}"),
        }
    }
}

async fn process_job(
    job_id: Uuid,
    pool: &PgPool,
    s3: &S3Client,
    sm: &SmClient,
    bucket: &str,
    ca_secret_arn: &str,
) -> Result<()> {
    // Acquire advisory lock so only one worker processes this job
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(i64::from_ne_bytes(job_id.as_bytes()[..8].try_into()?))
        .fetch_one(pool)
        .await?;

    if !locked {
        info!("job {job_id} already being processed, skipping");
        return Ok(());
    }

    info!("processing job {job_id}");

    match run_pipeline(job_id, pool, s3, sm, bucket, ca_secret_arn).await {
        Ok(()) => {
            info!("job {job_id} completed successfully");
        }
        Err(e) => {
            let msg = e.to_string();
            error!("job {job_id} pipeline error: {msg}");
            sqlx::query(
                "UPDATE vetting_jobs SET status = 'fail', results = $1, completed_at = now() WHERE id = $2",
            )
            .bind(serde_json::json!({"message": msg}))
            .bind(job_id)
            .execute(pool)
            .await?;
        }
    }

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(i64::from_ne_bytes(job_id.as_bytes()[..8].try_into()?))
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn job_id_parses_from_notify_payload() {
        let payload = "550e8400-e29b-41d4-a716-446655440000";
        let id = uuid::Uuid::parse_str(payload).unwrap();
        assert_eq!(id.to_string(), payload);
    }
}
