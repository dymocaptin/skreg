//! `skreg search` — search the registry for skills.

use anyhow::Result;

use skreg_client::client::{HttpRegistryClient, RegistryClient};

use crate::config::{default_config_path, load_config};

/// Run `skreg search <query>`.
///
/// # Errors
///
/// Returns an error if the registry request fails.
pub async fn run_search(query: &str) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg = load_config(&cfg_path)?;
    let client = HttpRegistryClient::new(&cfg.registry);
    let results = client.search(query).await?;

    if results.is_empty() {
        println!("No results for {:?}", query);
        return Ok(());
    }

    for r in &results {
        let version = r.latest_version.as_deref().unwrap_or("?");
        let desc = r.description.as_deref().unwrap_or("");
        println!("{}/{} v{}  {}", r.namespace, r.name, version, desc);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn search_module_compiles() {}
}
