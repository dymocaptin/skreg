use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::{AppState, build_router};

async fn make_state() -> AppState {
    let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test")
        .expect("lazy pool");
    let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::meta::region::RegionProviderChain::default_provider())
        .load()
        .await;
    AppState {
        pool,
        s3:         aws_sdk_s3::Client::new(&aws_cfg),
        ses:        aws_sdk_sesv2::Client::new(&aws_cfg),
        s3_bucket:  "test-bucket".to_owned(),
        from_email: "test@example.com".to_owned(),
    }
}

#[tokio::test]
async fn search_without_reachable_db_returns_500() {
    // With a lazy pool pointing at a non-existent DB, the search endpoint
    // returns 500 Internal Server Error when the query is attempted.
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server
        .get("/v1/search")
        .add_query_params([("q", "test")])
        .await;
    assert_eq!(response.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}
