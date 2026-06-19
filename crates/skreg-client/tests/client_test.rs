use skreg_client::client::RegistryClient;

// Smoke test: the trait is object-safe (can be used as dyn RegistryClient).
fn _assert_object_safe(_: &dyn RegistryClient) {}

#[tokio::test]
async fn http_client_search_returns_error_on_bad_url() {
    let client = skreg_client::client::HttpRegistryClient::new("http://127.0.0.1:1");
    let result = client.search("hello", false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn http_client_search_verified_only_returns_error_on_bad_url() {
    let client = skreg_client::client::HttpRegistryClient::new("http://127.0.0.1:1");
    let result = client.search("hello", true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn http_client_yank_all_returns_error_on_bad_url() {
    let client = skreg_client::client::HttpRegistryClient::new("http://127.0.0.1:1");
    let result = client.yank("skreg_key", "acme", "my-skill", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn http_client_yank_version_returns_error_on_bad_url() {
    let client = skreg_client::client::HttpRegistryClient::new("http://127.0.0.1:1");
    let result = client
        .yank("skreg_key", "acme", "my-skill", Some("1.0.0"))
        .await;
    assert!(result.is_err());
}
