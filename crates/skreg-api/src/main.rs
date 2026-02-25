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
    let state = AppState {
        pool,
        s3: aws_sdk_s3::Client::new(&aws_cfg),
        ses: aws_sdk_sesv2::Client::new(&aws_cfg),
        s3_bucket: config.s3_bucket.clone(),
        from_email: config.from_email.clone(),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    log::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
