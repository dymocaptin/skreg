//! `skreg search` — search the registry for skills.

use anyhow::{Context, Result};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use skreg_client::client::{HttpRegistryClient, RegistryClient};

use crate::config::{default_config_path, load_config};

/// Run `skreg search <query>`.
///
/// # Errors
///
/// Returns an error if the registry request fails.
pub async fn run_search(query: &str, trusted_only: bool) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;
    let client = HttpRegistryClient::new(cfg.registry());
    let results = client.search(query, trusted_only).await?;

    if results.is_empty() {
        println!("No results for '{query}'");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(["Package", "Version", "Trusted", "Description"]);

    for r in &results {
        let package = format!("{}/{}", r.namespace, r.name);
        let version = r.latest_version.as_deref().unwrap_or("?");
        let trusted = if r.trusted { "✓" } else { "✗" };
        let desc = r.description.as_deref().unwrap_or("");
        table.add_row([package.as_str(), version, trusted, desc]);
    }

    println!("{table}");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn search_module_compiles() {}
}
