//! Job runner: listens on `pg_notify("vetting_jobs")` and dispatches stage pipeline.

use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use log::{error, info};
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use uuid::Uuid;

use crate::stages::run_pipeline;

/// Shared configuration threaded through the job pipeline.
struct JobCtx<'a> {
    pool: &'a PgPool,
    s3: &'a S3Client,
    smtp_host: &'a str,
    smtp_port: u16,
    from_email: &'a str,
    bucket: &'a str,
    registry_ca_key_pem: &'a str,
}

/// Start the `pg_notify` listener loop. Blocks until a fatal error occurs.
///
/// # Errors
///
/// Returns an error if the initial database connection or listener setup fails.
pub async fn run(
    pool: PgPool,
    s3: S3Client,
    smtp_host: String,
    smtp_port: u16,
    from_email: String,
    bucket: String,
    registry_ca_key_pem: String,
) -> Result<()> {
    // Process any jobs already pending in the DB before entering the listen loop.
    // This handles the timing gap where pg_notify fires before this process starts.
    drain_pending(&JobCtx {
        pool: &pool,
        s3: &s3,
        smtp_host: &smtp_host,
        smtp_port,
        from_email: &from_email,
        bucket: &bucket,
        registry_ca_key_pem: &registry_ca_key_pem,
    })
    .await;

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
                let smtp2 = smtp_host.clone();
                let from2 = from_email.clone();
                let bucket2 = bucket.clone();
                let pem2 = registry_ca_key_pem.clone();
                tokio::spawn(async move {
                    let ctx = JobCtx {
                        pool: &pool2,
                        s3: &s3_2,
                        smtp_host: &smtp2,
                        smtp_port,
                        from_email: &from2,
                        bucket: &bucket2,
                        registry_ca_key_pem: &pem2,
                    };
                    if let Err(e) = process_job(job_id, &ctx).await {
                        error!("job {job_id} failed: {e}");
                    }
                });
            }
            Err(e) => error!("invalid job_id payload '{payload}': {e}"),
        }
    }
}

/// Process jobs already sitting in `pending` state — handles startup timing gaps.
async fn drain_pending(ctx: &JobCtx<'_>) {
    let ids: Vec<Uuid> = match sqlx::query_scalar(
        "SELECT id FROM vetting_jobs WHERE status = 'pending' ORDER BY created_at",
    )
    .fetch_all(ctx.pool)
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            error!("drain query failed: {e}");
            return;
        }
    };

    for job_id in ids {
        if let Err(e) = process_job(job_id, ctx).await {
            error!("drain job {job_id} failed: {e}");
        }
    }
}

async fn process_job(job_id: Uuid, ctx: &JobCtx<'_>) -> Result<()> {
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(i64::from_ne_bytes(job_id.as_bytes()[..8].try_into()?))
        .fetch_one(ctx.pool)
        .await?;

    if !locked {
        info!("job {job_id} already being processed, skipping");
        return Ok(());
    }

    info!("processing job {job_id}");

    match run_pipeline(job_id, ctx.pool, ctx.s3, ctx.bucket, ctx.registry_ca_key_pem).await {
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
            .execute(ctx.pool)
            .await?;

            if let Err(email_err) = send_failure_email(job_id, &msg, ctx).await {
                error!("failed to send failure email for job {job_id}: {email_err}");
            }
        }
    }

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(i64::from_ne_bytes(job_id.as_bytes()[..8].try_into()?))
        .execute(ctx.pool)
        .await?;

    Ok(())
}

async fn send_failure_email(job_id: Uuid, message: &str, ctx: &JobCtx<'_>) -> Result<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT ak.email,
                n.slug || '/' || p.name || '@' || v.version
         FROM vetting_jobs j
         JOIN versions v ON v.id = j.version_id
         JOIN packages p ON p.id = v.package_id
         JOIN namespaces n ON n.id = p.namespace_id
         JOIN api_keys ak ON ak.namespace_id = n.id
         WHERE j.id = $1
         LIMIT 1",
    )
    .bind(job_id)
    .fetch_optional(ctx.pool)
    .await?;

    let Some((to_email, pkg_ref)) = row else {
        error!("no email found for job {job_id}, skipping failure email");
        return Ok(());
    };

    crate::email::send_email(
        ctx.smtp_host,
        ctx.smtp_port,
        ctx.from_email,
        &to_email,
        &format!("Publishing {pkg_ref} failed"),
        &format!("Publishing {pkg_ref} failed: {message}"),
    )
    .await
    .map_err(|e| anyhow::anyhow!(e))?;

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
