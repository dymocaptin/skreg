//! `skreg publish` — pack, upload, and poll vetting result.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::commands::pack::run_pack;
use crate::config::{default_config_path, load_config};

/// Seconds between job status polls.
pub const POLL_INTERVAL_SECS: u64 = 3;

#[derive(Deserialize)]
struct PublishResponse {
    job_id: String,
}

#[derive(Deserialize)]
struct JobStatus {
    status: String,
    message: Option<String>,
}

/// Run `skreg publish` — pack the current directory, upload to the registry,
/// then poll until vetting passes or fails.
///
/// # Errors
///
/// Returns an error if the config file is missing, the upload fails,
/// or vetting rejects the package.
pub async fn run_publish(context: Option<&str>) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;
    let cfg = crate::config::apply_context(cfg, context)?;

    let cwd = std::env::current_dir()?;

    let meta = crate::frontmatter::read_skill_metadata(&cwd.join("SKILL.md"))
        .context("failed to read SKILL.md metadata")?;
    let name = meta.name.as_str();
    let version = meta.version.to_string();

    let skill_file = cwd.join(format!("{name}-{version}.skill"));
    run_pack(&cwd, None, None)?;

    let bytes = std::fs::read(&skill_file)?;
    let client = reqwest::Client::new();

    println!("uploading to {}...", cfg.registry());
    let resp = client
        .post(format!("{}/v1/publish", cfg.registry()))
        .header("Authorization", format!("Bearer {}", cfg.api_key()))
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("publish failed: {} — {}", resp.status(), resp.text().await?);
    }

    let publish: PublishResponse = resp.json().await?;
    println!("vetting started (job {})", publish.job_id);

    // Poll until done
    loop {
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;

        let job: JobStatus = client
            .get(format!("{}/v1/jobs/{}", cfg.registry(), publish.job_id))
            .send()
            .await?
            .json()
            .await?;

        match job.status.as_str() {
            "pass" => {
                println!("Published {}/{name}@{version}", cfg.namespace());
                std::fs::remove_file(&skill_file).ok();
                return Ok(());
            }
            "fail" | "quarantined" => {
                bail!(
                    "Vetting failed: {}",
                    job.message.unwrap_or_else(|| "unknown reason".to_owned())
                );
            }
            _ => print!("."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const _: () = assert!(POLL_INTERVAL_SECS > 0 && POLL_INTERVAL_SECS <= 10);
}
