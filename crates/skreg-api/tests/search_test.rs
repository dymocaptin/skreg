use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::build_router;

#[tokio::test]
async fn search_without_db_returns_503() {
    // Without a pool, endpoints that require DB return 503 Service Unavailable.
    // axum-test 14 requires query params via add_query_params, not inline ?key=val.
    let app = build_router(None);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/search").add_query_params(&[("q", "test")]).await;
    assert_eq!(response.status_code(), StatusCode::SERVICE_UNAVAILABLE);
}
