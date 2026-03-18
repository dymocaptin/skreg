//! `skreg certify` — obtain a CA-issued publisher cert.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::config::{default_config_path, load_config};
use crate::keys::{ensure_keys_exist, keys_dir};

/// Response body from `POST /v1/namespaces/:ns/cert`.
#[derive(Deserialize)]
struct CertResponse {
    cert: String,
    ca_cert: String,
}

/// Run `skreg certify` — obtain a CA-issued publisher certificate.
///
/// Loads the current config, ensures a local key pair exists, generates a
/// proper PKCS#10 CSR containing the public key signed by the local private
/// key, POSTs it to the registry, then writes the returned `publisher.crt`
/// and `publisher-ca.crt` to `~/.skreg/keys/`.
///
/// The private key never leaves the local machine — the server signs only the
/// public key embedded in the CSR.
///
/// # Errors
///
/// Returns an error if the config is missing, the registry is unreachable,
/// or the cert response cannot be written to disk.
pub async fn run_certify(key: Option<&Path>, context: Option<&str>) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;
    let cfg = crate::config::apply_context(cfg, context)?;

    let namespace = cfg.namespace().to_owned();
    let api_key = cfg.api_key().to_owned();
    let registry = cfg.registry().to_owned();

    let kdir = keys_dir()?;

    // Load or generate keys. If the caller passed an explicit key path, read
    // it from disk; otherwise fall through to ensure_keys_exist which will
    // load or auto-generate.
    let private_key_pem = if let Some(key_path) = key {
        std::fs::read_to_string(key_path)
            .with_context(|| format!("reading key from {}", key_path.display()))?
    } else {
        let keys = ensure_keys_exist(&kdir, &namespace)?;
        keys.private_key_pem
    };

    // Build a proper PKCS#10 CSR. The CSR contains the public key and a
    // proof-of-possession signature so the server never needs the private key.
    let csr_pem = build_csr_pem(&namespace, &private_key_pem)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{registry}/v1/namespaces/{namespace}/cert"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "text/plain")
        .body(csr_pem)
        .send()
        .await
        .context("sending cert request to registry")?;

    if !resp.status().is_success() {
        bail!(
            "certify failed: {} — {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let cert_resp: CertResponse = resp.json().await.context("parsing cert response")?;

    // Write publisher.crt and publisher-ca.crt to the keys directory.
    std::fs::create_dir_all(&kdir)
        .with_context(|| format!("creating keys directory {}", kdir.display()))?;

    let cert_path = kdir.join("publisher.crt");
    let ca_path = kdir.join("publisher-ca.crt");

    std::fs::write(&cert_path, &cert_resp.cert)
        .with_context(|| format!("writing {}", cert_path.display()))?;
    std::fs::write(&ca_path, &cert_resp.ca_cert)
        .with_context(|| format!("writing {}", ca_path.display()))?;

    println!("Certificate issued and written to {}", cert_path.display());
    println!("CA certificate written to {}", ca_path.display());

    Ok(())
}

/// Build a PKCS#10 CSR PEM for `namespace` using the provided private key.
///
/// Uses [`rcgen::Certificate::serialize_request_pem`] which produces a
/// standards-compliant CSR containing the public key and a proof-of-possession
/// signature made with the private key.  The CN is set to `namespace`.
///
/// # Errors
///
/// Returns an error if the key cannot be parsed or CSR serialization fails.
pub(crate) fn build_csr_pem(namespace: &str, private_key_pem: &str) -> Result<String> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair};

    let key_pair =
        KeyPair::from_pem(private_key_pem).context("parsing private key PEM for rcgen")?;

    let mut params = CertificateParams::new(vec![namespace.to_owned()]);
    params.alg = &rcgen::PKCS_RSA_SHA256;
    params.key_pair = Some(key_pair);

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, namespace);
    params.distinguished_name = dn;

    let cert = Certificate::from_params(params).context("building certificate params")?;
    cert.serialize_request_pem()
        .context("serializing CSR to PEM")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn certify_module_compiles() {}

    #[test]
    fn build_csr_pem_produces_pkcs10() {
        let dir = tempdir().unwrap();
        let keys = crate::keys::ensure_keys_exist(dir.path(), "acme").unwrap();
        let pem = build_csr_pem("acme", &keys.private_key_pem).unwrap();
        assert!(pem.contains("CERTIFICATE REQUEST"));
    }
}
