//! skreg registry API server entry point.

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
    let ses_conf = aws_sdk_sesv2::config::Builder::from(&aws_cfg)
        .region(aws_sdk_sesv2::config::Region::new(
            config.ses_region.clone(),
        ))
        .build();
    let state = AppState {
        pool,
        s3: aws_sdk_s3::Client::new(&aws_cfg),
        ses: aws_sdk_sesv2::Client::from_conf(ses_conf),
        sm: aws_sdk_secretsmanager::Client::new(&aws_cfg),
        s3_bucket: config.s3_bucket.clone(),
        from_email: config.from_email.clone(),
        publisher_ca_key_secret_name: std::env::var("PUBLISHER_CA_KEY_SECRET_NAME")
            .unwrap_or_else(|_| "skreg/publisher-ca-key".to_owned()),
        publisher_ca_cert_pem: std::env::var("PUBLISHER_CA_CERT_PEM").unwrap_or_default(),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    log::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
