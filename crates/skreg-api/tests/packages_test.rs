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
        sm: aws_sdk_secretsmanager::Client::new(&aws_cfg),
        s3_bucket: "test-bucket".to_owned(),
        from_email: "test@example.com".to_owned(),
        publisher_ca_key_secret_name: "skreg/publisher-ca-key".to_owned(),
        publisher_ca_cert_pem: String::new(),
    }
}

#[tokio::test]
async fn packages_meta_endpoint_exists() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/packages/acme/my-skill/1.0.0").await;
    assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    assert_ne!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn packages_download_endpoint_exists() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/download/acme/my-skill/1.0.0").await;
    assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    assert_ne!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn packages_sig_endpoint_exists() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/download/acme/my-skill/1.0.0/sig").await;
    assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    assert_ne!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn packages_rejects_invalid_namespace() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/packages/ACME/my-skill/1.0.0").await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn packages_download_rejects_invalid_namespace() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/download/ACME/my-skill/1.0.0").await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn packages_sig_rejects_invalid_namespace() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/download/ACME/my-skill/1.0.0/sig").await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}
