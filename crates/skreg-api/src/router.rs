//! Axum router construction.

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

use crate::handlers::search::search_handler;

/// Shared application state — pool is `None` when running without a database.
pub type AppState = Arc<Option<PgPool>>;

/// Response body for the health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
///
/// `pool` may be `None` in tests that do not exercise database endpoints.
/// Routes that require the database return `503 Service Unavailable` when
/// `pool` is `None`.
pub fn build_router(pool: Option<PgPool>) -> Router {
    let state: AppState = Arc::new(pool);
    Router::new()
        .route("/healthz", get(health_handler))
        .route("/v1/search", get(search_handler))
        .with_state(state)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
