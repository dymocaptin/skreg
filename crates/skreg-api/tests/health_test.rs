use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::{build_router, AppState};

async fn make_state() -> AppState {
    let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test").expect("lazy pool");
    let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::meta::region::RegionProviderChain::default_provider())
        .load()
        .await;
    AppState {
        pool,
        s3: aws_sdk_s3::Client::new(&aws_cfg),
        ses: aws_sdk_sesv2::Client::new(&aws_cfg),
        s3_bucket: "test-bucket".to_owned(),
        from_email: "test@example.com".to_owned(),
    }
}

#[tokio::test]
async fn health_returns_200() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/healthz").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}
