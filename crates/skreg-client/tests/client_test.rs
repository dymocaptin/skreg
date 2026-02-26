use skreg_client::client::RegistryClient;

// Smoke test: the trait is object-safe (can be used as dyn RegistryClient).
fn _assert_object_safe(_: &dyn RegistryClient) {}
