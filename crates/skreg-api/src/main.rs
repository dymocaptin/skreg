//! skreg registry API server entry point.

use anyhow::Context;
use skreg_api::{
    config::ApiConfig,
    db::connect_and_migrate,
    router::{build_router, AppState},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config = ApiConfig::from_env()?;
    let pool = connect_and_migrate(&config.database_url).await?;
    let aws_cfg = aws_config::load_from_env().await;
    // S3-compatible stores (e.g. MinIO) require path-style addressing. Enable it when
    // AWS_ENDPOINT_URL is set, which indicates a non-AWS endpoint is in use.
    let s3_path_style = std::env::var("AWS_ENDPOINT_URL").is_ok();
    let s3_conf = aws_sdk_s3::config::Builder::from(&aws_cfg)
        .force_path_style(s3_path_style)
        .build();
    let publisher_ca_key_pem =
        std::env::var("PUBLISHER_CA_KEY_PEM").context("PUBLISHER_CA_KEY_PEM must be set")?;
    let state = AppState {
        pool,
        s3: aws_sdk_s3::Client::from_conf(s3_conf),
        s3_bucket: config.s3_bucket.clone(),
        from_email: config.from_email.clone(),
        smtp: config.smtp.clone(),
        publisher_ca_key_pem,
        publisher_ca_cert_pem: std::env::var("PUBLISHER_CA_CERT_PEM").unwrap_or_default(),
        smtp_disabled: std::env::var("SMTP_DISABLED").as_deref() == Ok("true"),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    log::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
