#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::Context as _;

    env_logger::init();
    let database_url = std::env::var("DATABASE_URL")?;
    let bucket = std::env::var("S3_BUCKET")?;

    let pool = sqlx::PgPool::connect(&database_url).await?;
    let aws_cfg = aws_config::load_from_env().await;
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);
    let ses = aws_sdk_sesv2::Client::new(&aws_cfg);

    let registry_ca_key_pem = match std::env::var("REGISTRY_CA_KEY_PEM") {
        Ok(pem) => pem,
        Err(_) => {
            let arn = std::env::var("CA_SECRET_ARN")
                .context("either REGISTRY_CA_KEY_PEM or CA_SECRET_ARN must be set")?;
            let sm = aws_sdk_secretsmanager::Client::new(&aws_cfg);
            sm.get_secret_value()
                .secret_id(&arn)
                .send()
                .await
                .context("fetching CA key from Secrets Manager")?
                .secret_string()
                .context("CA secret has no string value")?
                .to_owned()
        }
    };

    skreg_worker::runner::run(pool, s3, ses, bucket, registry_ca_key_pem).await
}
