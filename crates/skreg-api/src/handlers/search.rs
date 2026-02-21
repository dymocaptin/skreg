//! Handlers for the package search and metadata endpoints.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;

use crate::models::{PackageSummary, SearchQuery, SearchResponse};
use crate::router::AppState;

const PAGE_SIZE: i64 = 20;

/// `GET /v1/search` — full-text package search.
pub async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let pool = state.as_ref().as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;
    let query = params.q.unwrap_or_default();

    let rows: Vec<PackageSummary> = sqlx::query_as(
        r#"
        SELECT p.id, n.slug AS namespace, p.name, p.description, p.category, p.created_at
        FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
        ORDER BY p.created_at DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&query)
    .bind(&params.category)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("search query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total: i64 = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT COUNT(*) FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
        "#,
    )
    .bind(&query)
    .bind(&params.category)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("count query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .unwrap_or(0);

    Ok(Json(SearchResponse { packages: rows, total, page }))
}
