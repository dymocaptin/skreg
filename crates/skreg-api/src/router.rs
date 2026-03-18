//! Axum router construction.

use std::sync::Arc;

use aws_sdk_s3::Client as S3Client;
use aws_sdk_sesv2::Client as SesClient;
use axum::{
    http::HeaderValue,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use sqlx::PgPool;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};

use crate::handlers::auth::{login_handler, token_handler};
use crate::handlers::cert::cert_handler;
use crate::handlers::jobs::job_status_handler;
use crate::handlers::namespaces::create_namespace_handler;
use crate::handlers::packages::{
    package_download_handler, package_meta_handler, package_sig_handler,
};
use crate::handlers::preview::package_preview_handler;
use crate::handlers::publish::publish_handler;
use crate::handlers::rotate::{rotate_confirm_handler, rotate_submit_handler};
use crate::handlers::search::search_handler;

/// Shared application state injected into every handler.
#[derive(Clone)]
pub struct AppState {
    /// `PostgreSQL` connection pool.
    pub pool: PgPool,
    /// AWS S3 client.
    pub s3: S3Client,
    /// AWS SES v2 client.
    pub ses: SesClient,
    /// S3 bucket name for package artifacts.
    pub s3_bucket: String,
    /// Sender address for transactional email.
    pub from_email: String,
    /// PEM-encoded Publisher CA private key, resolved once at startup.
    pub publisher_ca_key_pem: String,
    /// PEM-encoded Publisher CA certificate.
    pub publisher_ca_cert_pem: String,
    /// When `true`, OTPs are logged at INFO level instead of sent via SES.
    pub ses_disabled: bool,
}

/// Arc-wrapped [`AppState`] used as the Axum router state.
pub type SharedState = Arc<AppState>;

/// Response body for the health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
///
/// # Panics
///
/// Panics if the hard-coded `skreg.ai` origin cannot be parsed as an HTTP
/// header value (this should never happen in practice).
pub fn build_router(state: AppState) -> Router {
    let shared = Arc::new(state);
    let origin: HeaderValue = "https://skreg.ai"
        .parse()
        .expect("skreg.ai is a valid header value");
    let cors = CorsLayer::new()
        .allow_origin(origin)
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request());
    Router::new()
        .route("/healthz", get(health_handler))
        .route("/v1/search", get(search_handler))
        .route("/v1/namespaces", post(create_namespace_handler))
        .route("/v1/namespaces/:ns/cert", post(cert_handler))
        .route("/v1/namespaces/:ns/rotate-key", post(rotate_submit_handler))
        .route(
            "/v1/namespaces/:ns/rotate-key/confirm",
            get(rotate_confirm_handler),
        )
        .route("/v1/auth/login", post(login_handler))
        .route("/v1/auth/token", post(token_handler))
        .route("/v1/publish", post(publish_handler))
        .route("/v1/jobs/:id", get(job_status_handler))
        .route("/v1/packages/:ns/:name/:version", get(package_meta_handler))
        .route(
            "/v1/packages/:ns/:name/:version/preview",
            get(package_preview_handler),
        )
        .route(
            "/v1/download/:ns/:name/:version",
            get(package_download_handler),
        )
        .route(
            "/v1/download/:ns/:name/:version/sig",
            get(package_sig_handler),
        )
        .layer(cors)
        .with_state(shared)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
