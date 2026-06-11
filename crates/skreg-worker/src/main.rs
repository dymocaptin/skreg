#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::Context as _;

    env_logger::init();
    let database_url = std::env::var("DATABASE_URL")?;
    let bucket = std::env::var("S3_BUCKET")?;
    let from_email = std::env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@skreg.ai".to_owned());
    let smtp = skreg_worker::email::SmtpConfig {
        host: std::env::var("SMTP_HOST")
            .unwrap_or_else(|_| "postfix.skreg.svc.cluster.local".to_owned()),
        port: std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25),
        username: std::env::var("SMTP_USERNAME").ok(),
        password: std::env::var("SMTP_PASSWORD").ok(),
    };

    let pool = sqlx::PgPool::connect(&database_url).await?;
    let aws_cfg = aws_config::load_from_env().await;
    // S3-compatible stores (e.g. MinIO) require path-style addressing. Enable it when
    // AWS_ENDPOINT_URL is set, which indicates a non-AWS endpoint is in use.
    let s3_path_style = std::env::var("AWS_ENDPOINT_URL").is_ok();
    let s3_conf = aws_sdk_s3::config::Builder::from(&aws_cfg)
        .force_path_style(s3_path_style)
        .build();
    let s3 = aws_sdk_s3::Client::from_conf(s3_conf);

    let registry_ca_key_pem =
        std::env::var("REGISTRY_CA_KEY_PEM").context("REGISTRY_CA_KEY_PEM must be set")?;

    skreg_worker::runner::run(pool, s3, smtp, from_email, bucket, registry_ca_key_pem).await
}
