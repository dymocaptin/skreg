//! `skreg yank` — remove a published skill from the registry (soft yank).

use anyhow::{Context, Result};

use skreg_client::client::{HttpRegistryClient, RegistryClient};
use skreg_core::package_ref::PackageRef;

use crate::config::{apply_context, default_config_path, load_config};

/// Parsed target of a yank: namespace, name, and an optional specific version.
struct YankTarget {
    namespace: String,
    name: String,
    version: Option<String>,
}

/// Parse a `namespace/name` or `namespace/name@version` reference.
fn parse_target(package_ref: &str) -> Result<YankTarget> {
    let pkg_ref = PackageRef::parse(package_ref)
        .with_context(|| format!("invalid package reference: {package_ref:?}"))?;
    Ok(YankTarget {
        namespace: pkg_ref.namespace.to_string(),
        name: pkg_ref.name.to_string(),
        version: pkg_ref.version.map(|v| v.to_string()),
    })
}

/// Run `skreg yank <package_ref>`.
///
/// `namespace/name` yanks all versions; `namespace/name@version` yanks one.
///
/// # Errors
///
/// Returns an error if not logged in, the reference is invalid, or the registry
/// rejects the request.
pub async fn run_yank(package_ref: &str, context: Option<&str>) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;
    let cfg = apply_context(cfg, context)?;

    let target = parse_target(package_ref)?;

    let client = HttpRegistryClient::new(cfg.registry().to_owned());
    let yanked = client
        .yank(
            cfg.api_key(),
            &target.namespace,
            &target.name,
            target.version.as_deref(),
        )
        .await
        .context("yank request failed")?;

    match (&target.version, yanked) {
        (Some(v), 0) => {
            println!(
                "{}/{}@{v} was already yanked (nothing to do)",
                target.namespace, target.name
            );
        }
        (Some(v), _) => {
            println!("Yanked {}/{}@{v}", target.namespace, target.name);
        }
        (None, 0) => {
            println!(
                "{}/{} had no installable versions to yank (nothing to do)",
                target.namespace, target.name
            );
        }
        (None, n) => {
            println!(
                "Yanked {}/{} ({n} version(s))",
                target.namespace, target.name
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_target;

    #[test]
    fn parses_namespace_name_without_version() {
        let t = parse_target("acme/my-skill").unwrap();
        assert_eq!(t.namespace, "acme");
        assert_eq!(t.name, "my-skill");
        assert_eq!(t.version, None);
    }

    #[test]
    fn parses_namespace_name_with_version() {
        let t = parse_target("acme/my-skill@1.0.0").unwrap();
        assert_eq!(t.namespace, "acme");
        assert_eq!(t.name, "my-skill");
        assert_eq!(t.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn rejects_invalid_ref() {
        assert!(parse_target("notavalidref").is_err());
    }
}
