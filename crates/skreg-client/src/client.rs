//! Registry HTTP client trait and `reqwest`-backed implementation.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use log::debug;
use skreg_core::manifest::Manifest;
use skreg_core::package_ref::PackageRef;

use crate::error::ClientError;

/// Boxed future returned by dyn-compatible async trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Version metadata returned by the registry for a resolved package.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    /// The full manifest for this version.
    pub manifest: Manifest,
    /// Signed tarball bytes.
    pub tarball: Vec<u8>,
    /// Detached signature bytes.
    pub signature: Vec<u8>,
}

/// A single result from a registry search.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchResult {
    /// Namespace slug the package belongs to.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Human-readable description, if any.
    pub description: Option<String>,
    /// Latest published version string (most recent by `published_at`), if any.
    pub latest_version: Option<String>,
    /// Whether the package's namespace holds a valid publisher cert.
    pub trusted: bool,
}

/// Communicates with a skreg-compatible registry.
pub trait RegistryClient: Send + Sync {
    /// Resolve a package reference to its latest (or pinned) version metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or parse failure.
    fn resolve<'a>(
        &'a self,
        pkg_ref: &'a PackageRef,
    ) -> BoxFuture<'a, Result<ResolvedVersion, ClientError>>;

    /// Search the registry for packages matching `query`.
    ///
    /// If `trusted_only` is `true`, only packages whose namespace holds a valid
    /// publisher cert are returned.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or parse failure.
    fn search<'a>(
        &'a self,
        query: &'a str,
        trusted_only: bool,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, ClientError>>;
}

/// `reqwest`-backed implementation of [`RegistryClient`].
#[derive(Debug, Clone)]
pub struct HttpRegistryClient {
    base_url: String,
    http: Arc<reqwest::Client>,
}

impl HttpRegistryClient {
    /// Create a new client targeting `base_url`.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: Arc::new(reqwest::Client::new()),
        }
    }
}

impl HttpRegistryClient {
    /// Fetch the [`Manifest`] for a specific package version without downloading the tarball.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or parse failure.
    pub async fn fetch_manifest(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Result<Manifest, ClientError> {
        let url = format!("{}/v1/packages/{namespace}/{name}/{version}", self.base_url,);
        debug!("fetching manifest from {url}");
        self.http
            .get(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(ClientError::Http)?
            .json::<Manifest>()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }
}

impl RegistryClient for HttpRegistryClient {
    fn resolve<'a>(
        &'a self,
        pkg_ref: &'a PackageRef,
    ) -> BoxFuture<'a, Result<ResolvedVersion, ClientError>> {
        Box::pin(async move {
            let version_segment = pkg_ref
                .version
                .as_ref()
                .map_or_else(|| "latest".to_owned(), ToString::to_string);

            let meta_url = format!(
                "{}/v1/packages/{}/{}/{}",
                self.base_url, pkg_ref.namespace, pkg_ref.name, version_segment,
            );

            debug!("resolving package from {meta_url}");

            let manifest: Manifest = self
                .http
                .get(&meta_url)
                .send()
                .await?
                .error_for_status()
                .map_err(ClientError::Http)?
                .json()
                .await
                .map_err(|e| ClientError::Parse(e.to_string()))?;

            let dl_url = format!(
                "{}/v1/download/{}/{}/{}",
                self.base_url, pkg_ref.namespace, pkg_ref.name, manifest.version,
            );

            let tarball = self
                .http
                .get(&dl_url)
                .send()
                .await?
                .error_for_status()
                .map_err(ClientError::Http)?
                .bytes()
                .await?
                .to_vec();
            let sig_url = format!("{dl_url}/sig");
            let signature = self
                .http
                .get(&sig_url)
                .send()
                .await?
                .error_for_status()
                .map_err(ClientError::Http)?
                .bytes()
                .await?
                .to_vec();

            Ok(ResolvedVersion {
                manifest,
                tarball,
                signature,
            })
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        trusted_only: bool,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, ClientError>> {
        #[derive(serde::Deserialize)]
        struct SearchResponse {
            packages: Vec<SearchResult>,
        }

        Box::pin(async move {
            let url = format!("{}/v1/search", self.base_url);
            debug!("searching registry: {url}?q={query} trusted_only={trusted_only}");

            let mut req = self.http.get(&url).query(&[("q", query)]);
            if trusted_only {
                req = req.query(&[("trusted", "true")]);
            }

            let resp: SearchResponse = req
                .send()
                .await?
                .error_for_status()
                .map_err(ClientError::Http)?
                .json()
                .await
                .map_err(|e| ClientError::Parse(e.to_string()))?;

            Ok(resp.packages)
        })
    }
}
