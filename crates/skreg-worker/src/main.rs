#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::Context as _;

    env_logger::init();
    let database_url = std::env::var("DATABASE_URL")?;
    let bucket = std::env::var("S3_BUCKET")?;
    let smtp_host =
        std::env::var("SMTP_HOST").unwrap_or_else(|_| "postfix.skreg.svc.cluster.local".to_owned());
    let smtp_port: u16 = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let from_email = std::env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@skreg.ai".to_owned());

    let pool = sqlx::PgPool::connect(&database_url).await?;
    let aws_cfg = aws_config::load_from_env().await;
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);

    let registry_ca_key_pem =
        std::env::var("REGISTRY_CA_KEY_PEM").context("REGISTRY_CA_KEY_PEM must be set")?;

    skreg_worker::runner::run(
        pool,
        s3,
        smtp_host,
        smtp_port,
        from_email,
        bucket,
        registry_ca_key_pem,
    )
    .await
}
