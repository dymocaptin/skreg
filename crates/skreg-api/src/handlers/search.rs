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
    let trusted_only = params.trusted.unwrap_or(false);

    let rows: Vec<PackageSummary> = sqlx::query_as(
        "
        SELECT p.id, n.slug AS namespace, p.name, p.description, p.category, p.created_at,
               (SELECT v2.version FROM versions v2
                WHERE v2.package_id = p.id AND v2.yanked_at IS NULL
                ORDER BY v2.published_at DESC LIMIT 1) AS latest_version,
               EXISTS (
                   SELECT 1 FROM publisher_certs pc
                   WHERE pc.namespace_id = p.namespace_id
                     AND pc.revoked_at IS NULL
                     AND pc.expires_at > now()
               ) AS trusted
        FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
          AND (NOT $3 OR EXISTS (
              SELECT 1 FROM publisher_certs pc
              WHERE pc.namespace_id = p.namespace_id
                AND pc.revoked_at IS NULL
                AND pc.expires_at > now()
          ))
        ORDER BY p.created_at DESC
        LIMIT $4 OFFSET $5
        ",
    )
    .bind(&query)
    .bind(&params.category)
    .bind(trusted_only)
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
              SELECT 1 FROM publisher_certs pc
              WHERE pc.namespace_id = p.namespace_id
                AND pc.revoked_at IS NULL
                AND pc.expires_at > now()
          ))
        ",
    )
    .bind(&query)
    .bind(&params.category)
    .bind(trusted_only)
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
