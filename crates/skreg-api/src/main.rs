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
    let ses_conf = aws_sdk_sesv2::config::Builder::from(&aws_cfg)
        .region(aws_sdk_sesv2::config::Region::new(
            config.ses_region.clone(),
        ))
        .build();

    let publisher_ca_key_pem = match std::env::var("PUBLISHER_CA_KEY_PEM") {
        Ok(pem) => pem,
        Err(_) => {
            let secret_name = std::env::var("PUBLISHER_CA_KEY_SECRET_NAME")
                .unwrap_or_else(|_| "skreg/publisher-ca-key".to_owned());
            let sm = aws_sdk_secretsmanager::Client::new(&aws_cfg);
            sm.get_secret_value()
                .secret_id(&secret_name)
                .send()
                .await
                .context("fetching publisher CA key from Secrets Manager")?
                .secret_string()
                .context("publisher CA secret has no string value")?
                .to_owned()
        }
    };

    let ses_disabled = std::env::var("SES_DISABLED").as_deref() == Ok("true");

    let state = AppState {
        pool,
        s3: aws_sdk_s3::Client::new(&aws_cfg),
        ses: aws_sdk_sesv2::Client::from_conf(ses_conf),
        s3_bucket: config.s3_bucket.clone(),
        from_email: config.from_email.clone(),
        publisher_ca_key_pem,
        publisher_ca_cert_pem: std::env::var("PUBLISHER_CA_CERT_PEM").unwrap_or_default(),
        ses_disabled,
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    log::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
