use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::build_router;

#[tokio::test]
async fn health_returns_200() {
    // build_router accepts an Option<PgPool>; pass None for unit tests
    // (health endpoint does not use the DB).
    let app = build_router(None);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/healthz").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}
