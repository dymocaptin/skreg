#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let database_url = std::env::var("DATABASE_URL")?;
    let bucket = std::env::var("S3_BUCKET")?;
    let ca_secret_arn = std::env::var("CA_SECRET_ARN")?;

    let pool = sqlx::PgPool::connect(&database_url).await?;
    let aws_cfg = aws_config::load_from_env().await;
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);
    let sm = aws_sdk_secretsmanager::Client::new(&aws_cfg);

    skreg_worker::runner::run(pool, s3, sm, bucket, ca_secret_arn).await
}
