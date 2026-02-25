//! Axum router construction.

use std::sync::Arc;

use aws_sdk_s3::Client as S3Client;
use aws_sdk_sesv2::Client as SesClient;
use axum::{routing::{get, post}, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

use crate::handlers::search::search_handler;
use crate::handlers::namespaces::create_namespace_handler;

/// Shared application state injected into every handler.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool:       PgPool,
    /// AWS S3 client.
    pub s3:         S3Client,
    /// AWS SES v2 client.
    pub ses:        SesClient,
    /// S3 bucket name for package artifacts.
    pub s3_bucket:  String,
    /// Sender address for transactional email.
    pub from_email: String,
}

/// Arc-wrapped [`AppState`] used as the Axum router state.
pub type SharedState = Arc<AppState>;

/// Response body for the health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
pub fn build_router(state: AppState) -> Router {
    let shared = Arc::new(state);
    Router::new()
        .route("/healthz",        get(health_handler))
        .route("/v1/search",      get(search_handler))
        .route("/v1/namespaces",  post(create_namespace_handler))
        .with_state(shared)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
