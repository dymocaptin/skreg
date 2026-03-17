//! Vetting pipeline stages.

pub mod content;
pub mod safety;
pub mod signing;
pub mod static_analysis;
pub mod structure;
pub mod verify_publisher;

use anyhow::Result;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_secretsmanager::Client as SmClient;
use sqlx::PgPool;
use uuid::Uuid;

use structure::check_structure;

/// Run the full vetting pipeline for `job_id`.
///
/// # Errors
///
/// Returns an error if any stage fails or a database/S3 operation fails.
pub async fn run_pipeline(
    job_id: Uuid,
    pool: &PgPool,
    s3: &S3Client,
    sm: &SmClient,
    bucket: &str,
    ca_secret_arn: &str,
) -> Result<()> {
    // Load job + version info (including namespace slug for Stage 4)
    let row = sqlx::query_as::<_, (Uuid, String, String, String, String, String)>(
        "SELECT v.id, v.sha256, v.storage_path, p.name, v.version, n.slug
         FROM vetting_jobs j
         JOIN versions v ON v.id = j.version_id
         JOIN packages p ON p.id = v.package_id
         JOIN namespaces n ON n.id = p.namespace_id
         WHERE j.id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await?;

    let (version_id, sha256, storage_path, pkg_name, version, namespace_slug) = row;

    // Download tarball from S3 to tempdir
    let obj = s3
        .get_object()
        .bucket(bucket)
        .key(&storage_path)
        .send()
        .await?;
    let bytes = obj.body.collect().await?.into_bytes();
    let tmp = skreg_pack::unpack::unpack_to_tempdir(&bytes)?;

    // Stage 1
    check_structure(tmp.path()).map_err(|e| anyhow::anyhow!("Stage 1 failed: {e}"))?;

    // Stage 2
    content::check_content(tmp.path()).map_err(|e| anyhow::anyhow!("Stage 2 failed: {e}"))?;

    // Stage 2.5 — static analysis
    let rules_dir = std::path::PathBuf::from(
        std::env::var("SKREG_YARA_RULES_DIR")
            .unwrap_or_else(|_| "crates/skreg-worker/rules".into()),
    );
    let compiled_rules = static_analysis::startup::check_yara_rules(&rules_dir)
        .map_err(|e| anyhow::anyhow!("YARA compilation failed: {e}"))?;
    let tracee_available = std::path::Path::new("/var/run/tracee/tracee.sock").exists();

    let sa_findings =
        static_analysis::run_static_analysis(tmp.path(), &compiled_rules, tracee_available)
            .map_err(|e| anyhow::anyhow!("Stage 2.5 failed: {e}"))?;

    if sa_findings.iter().any(|f| f.severity.is_blocking()) {
        let results = serde_json::json!({
            "status": "rejected",
            "stage": "static_analysis",
            "findings": sa_findings.iter().map(|f| serde_json::json!({
                "file": f.file,
                "tool": f.tool,
                "rule_id": f.rule_id,
                "severity": format!("{:?}", f.severity),
                "message": f.message,
            })).collect::<Vec<_>>()
        });
        sqlx::query(
            "UPDATE vetting_jobs SET status = 'rejected', completed_at = now(), results = $1 WHERE id = $2"
        )
        .bind(sqlx::types::Json(results))
        .bind(job_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    // Stage 3 — load existing names and yanked versions from DB
    let existing_names: Vec<String> = sqlx::query_scalar("SELECT name FROM packages")
        .fetch_all(pool)
        .await?;
    let yanked: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.name, v.version FROM versions v
         JOIN packages p ON p.id = v.package_id
         WHERE v.yanked_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    safety::check_safety(&pkg_name, &version, &existing_names, &yanked)
        .map_err(|e| anyhow::anyhow!("Stage 3 failed: {e}"))?;

    // Stage 4 — verify publisher signature
    verify_publisher::run_verify_publisher(
        version_id,
        &sha256,
        &storage_path,
        &namespace_slug,
        pool,
        s3,
        bucket,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Stage 4 failed: {e}"))?;

    // Stage 5 — sign with registry CA and store .sig in S3
    let sig_path = signing::run_signing(&sha256, &storage_path, s3, sm, bucket, ca_secret_arn)
        .await
        .map_err(|e| anyhow::anyhow!("Stage 5 failed: {e}"))?;

    sqlx::query("UPDATE versions SET sig_path = $1 WHERE id = $2")
        .bind(&sig_path)
        .bind(version_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "UPDATE vetting_jobs SET status = 'pass', completed_at = now(),
         results = '{\"message\": \"all stages passed\"}'::jsonb WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}
