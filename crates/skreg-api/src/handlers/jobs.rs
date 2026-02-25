//! GET /v1/jobs/{id} — poll vetting job status.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use serde::Serialize;

use crate::router::SharedState;

/// Response body for `GET /v1/jobs/{id}`.
#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    /// Job UUID.
    pub id: String,
    /// Current status: `pending`, `pass`, `fail`, or `quarantined`.
    pub status: String,
    /// Optional human-readable detail from the vetting results.
    pub message: Option<String>,
}

/// Handle `GET /v1/jobs/{id}` — return the current status of a vetting job.
pub async fn job_status_handler(
    State(state): State<SharedState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<JobStatusResponse>, StatusCode> {
    let row = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
        "SELECT status, results FROM vetting_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let message = row
        .1
        .and_then(|v| v.get("message").cloned())
        .and_then(|v| v.as_str().map(str::to_owned));

    Ok(Json(JobStatusResponse {
        id: id.to_string(),
        status: row.0,
        message,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_response_serialises() {
        let r = JobStatusResponse {
            id: "abc".to_owned(),
            status: "pending".to_owned(),
            message: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("pending"));
    }
}
