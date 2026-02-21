//! skreg registry API server entry point.

use log::info;
use skreg_api::{config::ApiConfig, db::connect_and_migrate, router::build_router};

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = ApiConfig::from_env().expect("failed to load API config");
    let pool = connect_and_migrate(&config.database_url)
        .await
        .expect("failed to connect to database and run migrations");

    let app = build_router(Some(pool));
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .expect("failed to bind TCP listener");

    info!("skreg-api listening on {}", config.bind_addr);
    axum::serve(listener, app).await.expect("server error");
}
