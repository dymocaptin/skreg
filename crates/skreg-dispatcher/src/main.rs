//! skreg-dispatcher: receives MinIO webhook events and spawns K8s worker Jobs.

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

mod job;

#[derive(Clone)]
struct AppState {
    kube: kube::Client,
    namespace: String,
    worker_image: String,
    pki_secret: String,
    db_secret: String,
    minio_secret: String,
    s3_bucket: String,
    s3_endpoint: String,
    aws_region: String,
    smtp_host: String,
    smtp_port: u16,
    from_email: String,
}

/// MinIO S3-notification webhook payload (S3-compatible schema).
#[derive(Debug, Deserialize)]
struct MinioEvent {
    #[serde(rename = "Key")]
    key: Option<String>,
    #[serde(rename = "Records")]
    records: Option<Vec<MinioRecord>>,
}

#[derive(Debug, Deserialize)]
struct MinioRecord {
    s3: S3Detail,
}

#[derive(Debug, Deserialize)]
struct S3Detail {
    object: S3Object,
}

#[derive(Debug, Deserialize)]
struct S3Object {
    key: String,
}

async fn notify_handler(
    State(state): State<Arc<AppState>>,
    Json(event): Json<MinioEvent>,
) -> StatusCode {
    let key = event
        .key
        .or_else(|| event.records?.into_iter().next().map(|r| r.s3.object.key));

    let Some(k) = key else {
        return StatusCode::OK;
    };
    if !k.ends_with(".skill") {
        return StatusCode::OK;
    }

    log::info!("dispatching worker for {k}");
    let cfg = job::WorkerConfig {
        namespace: &state.namespace,
        worker_image: &state.worker_image,
        pki_secret: &state.pki_secret,
        db_secret: &state.db_secret,
        minio_secret: &state.minio_secret,
        s3_bucket: &state.s3_bucket,
        s3_endpoint: &state.s3_endpoint,
        aws_region: &state.aws_region,
        smtp_host: &state.smtp_host,
        smtp_port: state.smtp_port,
        from_email: &state.from_email,
    };
    match job::ensure_worker_job(&state.kube, &cfg).await {
        Ok(name) => {
            log::info!("job: {name}");
            StatusCode::OK
        }
        Err(e) => {
            log::error!("job create failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let kube = kube::Client::try_default().await?;
    let namespace = std::env::var("KUBE_NAMESPACE").unwrap_or_else(|_| "skreg".to_owned());
    let worker_image = std::env::var("WORKER_IMAGE")?;
    let pki_secret = std::env::var("PKI_SECRET_NAME").unwrap_or_else(|_| "skreg-pki".to_owned());
    let db_secret = std::env::var("DB_SECRET_NAME").unwrap_or_else(|_| "skreg-db".to_owned());
    let minio_secret =
        std::env::var("MINIO_SECRET_NAME").unwrap_or_else(|_| "skreg-minio".to_owned());
    let s3_bucket = std::env::var("S3_BUCKET")?;
    let s3_endpoint = std::env::var("AWS_ENDPOINT_URL")?;
    let aws_region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
    let smtp_host =
        std::env::var("SMTP_HOST").unwrap_or_else(|_| "postfix.skreg.svc.cluster.local".to_owned());
    let smtp_port: u16 = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let from_email = std::env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@skreg.ai".to_owned());
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:9090".to_owned());

    let state = Arc::new(AppState {
        kube,
        namespace,
        worker_image,
        pki_secret,
        db_secret,
        minio_secret,
        s3_bucket,
        s3_endpoint,
        aws_region,
        smtp_host,
        smtp_port,
        from_email,
    });
    let app = Router::new()
        .route("/notify", post(notify_handler))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    log::info!("dispatcher listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
