//! `skreg login <namespace>` — register or re-authenticate.

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::config::{default_config_path, save_config, CliConfig};

#[derive(Deserialize)]
struct ApiKeyResponse {
    api_key: String,
}

/// Run `skreg login <namespace>` — register a new namespace or re-authenticate via OTP.
///
/// # Errors
///
/// Returns an error if the registry is unreachable, the namespace is unknown,
/// or the OTP is invalid.
pub async fn run_login(namespace: &str) -> Result<()> {
    let registry = std::env::var("SKILLPKG_REGISTRY")
        .unwrap_or_else(|_| "https://registry.skreg.dev".to_owned());

    print!("Email: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut email = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut email)?;
    let email = email.trim().to_owned();

    let client = reqwest::Client::new();

    // Try to create a new namespace first
    let create_resp = client
        .post(format!("{registry}/v1/namespaces"))
        .json(&serde_json::json!({"slug": namespace, "email": email}))
        .send()
        .await?;

    let api_key = if create_resp.status() == 409 {
        // Namespace exists — use OTP flow
        println!("Namespace exists. Sending one-time code to {email}...");
        let login_resp = client
            .post(format!("{registry}/v1/auth/login"))
            .json(&serde_json::json!({"namespace": namespace, "email": email}))
            .send()
            .await?;

        if !login_resp.status().is_success() {
            bail!("login failed: {}", login_resp.status());
        }

        print!("Enter the 6-digit code from your email: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut otp = String::new();
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut otp)?;
        let otp = otp.trim().to_owned();

        let token_resp = client
            .post(format!("{registry}/v1/auth/token"))
            .json(&serde_json::json!({"namespace": namespace, "otp": otp}))
            .send()
            .await?;

        if !token_resp.status().is_success() {
            bail!("invalid or expired code");
        }

        token_resp.json::<ApiKeyResponse>().await?.api_key
    } else if create_resp.status().is_success() {
        create_resp.json::<ApiKeyResponse>().await?.api_key
    } else {
        bail!("failed to register namespace: {}", create_resp.status());
    };

    let cfg = CliConfig {
        registry,
        namespace: namespace.to_owned(),
        api_key,
    };
    save_config(&cfg, &default_config_path())?;
    println!(
        "Logged in as {namespace}. Config saved to {}",
        default_config_path().display()
    );
    Ok(())
}
