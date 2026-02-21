//! API response and query models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single package search result.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PackageSummary {
    /// Package UUID.
    pub id: Uuid,
    /// Namespace slug.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Optional category tag.
    pub category: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Paginated search response.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Matching packages for this page.
    pub packages: Vec<PackageSummary>,
    /// Total number of matches across all pages.
    pub total: i64,
    /// Current page number (1-indexed).
    pub page: i64,
}

/// Query parameters for `GET /v1/search`.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Full-text search query.
    pub q: Option<String>,
    /// Filter by category.
    pub category: Option<String>,
    /// Page number (default 1).
    pub page: Option<i64>,
}
