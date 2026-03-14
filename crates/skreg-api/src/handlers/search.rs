//! Handlers for the package search and metadata endpoints.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;

use crate::models::{PackageSummary, SearchQuery, SearchResponse};
use crate::router::SharedState;

const PAGE_SIZE: i64 = 20;

/// `GET /v1/search` — full-text package search.
///
/// # Errors
///
/// Returns [`StatusCode::INTERNAL_SERVER_ERROR`] if the query fails.
pub async fn search_handler(
    State(state): State<SharedState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let pool = &state.pool;

    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;
    let query = params.q.unwrap_or_default();
    let verified_only = params.verified.unwrap_or(false);

    let rows: Vec<PackageSummary> = sqlx::query_as(
        "
        SELECT p.id, n.slug AS namespace, p.name, p.description, p.category, p.created_at,
               (SELECT v2.version FROM versions v2
                WHERE v2.package_id = p.id AND v2.yanked_at IS NULL
                ORDER BY v2.published_at DESC LIMIT 1) AS latest_version,
               COALESCE((SELECT v3.signer FROM versions v3
                WHERE v3.package_id = p.id AND v3.yanked_at IS NULL
                ORDER BY v3.published_at DESC LIMIT 1), 'self_signed') AS verification
        FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
          AND (NOT $3 OR EXISTS (
              SELECT 1 FROM versions v4
              WHERE v4.package_id = p.id
                AND v4.yanked_at IS NULL
                AND v4.signer = 'publisher'
          ))
        ORDER BY p.created_at DESC
        LIMIT $4 OFFSET $5
        ",
    )
    .bind(&query)
    .bind(&params.category)
    .bind(verified_only)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("search query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total: i64 = sqlx::query_scalar::<_, Option<i64>>(
        "
        SELECT COUNT(*) FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
          AND (NOT $3 OR EXISTS (
              SELECT 1 FROM versions v4
              WHERE v4.package_id = p.id
                AND v4.yanked_at IS NULL
                AND v4.signer = 'publisher'
          ))
        ",
    )
    .bind(&query)
    .bind(&params.category)
    .bind(verified_only)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("count query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .unwrap_or(0);

    Ok(Json(SearchResponse {
        packages: rows,
        total,
        page,
    }))
}
