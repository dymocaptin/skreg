//! Axum router construction.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

/// Response body for the health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
///
/// `pool` may be `None` in tests that do not exercise database endpoints.
pub fn build_router(pool: Option<PgPool>) -> Router {
    match pool {
        Some(p) => {
            let r: Router<PgPool> =
                Router::new().route("/healthz", get(health_handler));
            r.with_state(p)
        }
        None => Router::new().route("/healthz", get(health_handler)),
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
