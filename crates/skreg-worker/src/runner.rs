//! Job runner: listens on pg_notify("vetting_jobs") and dispatches stage pipeline.

use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SmClient;
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};
use aws_sdk_sesv2::Client as SesClient;
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
    ses: SesClient,
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
                let ses2 = ses.clone();
                let bucket2 = bucket.clone();
                let ca_arn2 = ca_secret_arn.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        process_job(job_id, &pool2, &s3_2, &sm2, &ses2, &bucket2, &ca_arn2).await
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
    ses: &SesClient,
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

            if let Err(email_err) = send_failure_email(job_id, &msg, pool, ses).await {
                error!("failed to send failure email for job {job_id}: {email_err}");
            }
        }
    }

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(i64::from_ne_bytes(job_id.as_bytes()[..8].try_into()?))
        .execute(pool)
        .await?;

    Ok(())
}

/// Look up the publisher's email and package ref, then send a failure notification via SES.
async fn send_failure_email(
    job_id: Uuid,
    message: &str,
    pool: &PgPool,
    ses: &SesClient,
) -> Result<()> {
    // Look up email address and package ref for this job
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
    .fetch_optional(pool)
    .await?;

    let Some((to_email, pkg_ref)) = row else {
        error!("no email found for job {job_id}, skipping failure email");
        return Ok(());
    };

    let subject = format!("Publishing {pkg_ref} failed");
    let body_text = format!("Publishing {pkg_ref} failed: {message}");

    let dest = Destination::builder().to_addresses(&to_email).build();
    let subject_content = Content::builder().data(&subject).charset("UTF-8").build()?;
    let body_content = Content::builder()
        .data(&body_text)
        .charset("UTF-8")
        .build()?;
    let body = Body::builder().text(body_content).build();
    let msg = Message::builder()
        .subject(subject_content)
        .body(body)
        .build();
    let email_content = EmailContent::builder().simple(msg).build();

    ses.send_email()
        .from_email_address("noreply@skreg.ai")
        .destination(dest)
        .content(email_content)
        .send()
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
