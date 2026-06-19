# skreg yank (first-class registry removal) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `skreg yank` command and registry endpoints that soft-remove a published skill (set `versions.yanked_at`), making published packages unavailable for install/search/download while keeping artifacts in place.

**Architecture:** Three layers mirroring the existing publish path. New Axum handler `handlers/yank.rs` exposes `POST /v1/packages/:ns/:name/yank` (all versions) and `POST /v1/packages/:ns/:name/:version/yank` (one version), gated on the bearer key's namespace matching `:ns`. `skreg-client` gains a `yank` trait method; `skreg-cli` gains a `yank` command. The DB column `versions.yanked_at` and all read-path filters already exist — this only sets the column.

**Tech Stack:** Rust, Axum, sqlx (Postgres, runtime-checked queries), reqwest, clap, axum-test, tokio.

**Testing note:** The API test harness (`crates/skreg-api/tests/`) uses a *lazy* DB pool (`PgPool::connect_lazy`) and never reaches a live database. Automated tests therefore cover only pre-DB paths: path validation (`400`), missing auth (`401`), and route existence. DB-dependent behavior (`403`/`404`/idempotency/actual yanking) is verified manually against a running stack, exactly as the existing `publish` handler's DB logic is. Do **not** add tests that require a live Postgres — CI has none.

**Worktree:** Already created at `.claude/worktrees/skreg-yank` (branch `feat/skreg-yank`). All work happens there. `docs/` is gitignored in this repo, so plan/spec commits use `git add -f`.

---

## File Structure

- Create: `crates/skreg-api/src/handlers/yank.rs` — yank handlers + shared logic.
- Modify: `crates/skreg-api/src/handlers/mod.rs` — register `pub mod yank;`.
- Modify: `crates/skreg-api/src/router.rs` — add two routes.
- Modify: `crates/skreg-api/tests/packages_test.rs` — add yank routing/auth/validation tests (reuses `make_state`).
- Modify: `crates/skreg-client/src/client.rs` — add `yank` to trait + `HttpRegistryClient`.
- Modify: `crates/skreg-client/tests/client_test.rs` — add bad-URL error tests for `yank`.
- Create: `crates/skreg-cli/src/commands/yank.rs` — `run_yank` + ref-parse helper + unit tests.
- Modify: `crates/skreg-cli/src/commands/mod.rs` — register `pub mod yank;`.
- Modify: `crates/skreg-cli/src/main.rs` — add `Yank` subcommand + dispatch.
- Modify: `CLAUDE.md` — add yank routes to the API Routes list.

---

## Task 1: API handler module with routing, validation, and auth (no DB paths)

**Files:**
- Create: `crates/skreg-api/src/handlers/yank.rs`
- Modify: `crates/skreg-api/src/handlers/mod.rs`
- Modify: `crates/skreg-api/src/router.rs`
- Test: `crates/skreg-api/tests/packages_test.rs`

- [ ] **Step 1: Write the failing routing/auth/validation tests**

Append to `crates/skreg-api/tests/packages_test.rs`:

```rust
#[tokio::test]
async fn yank_all_route_exists() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.post("/v1/packages/acme/my-skill/yank").await;
    assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    assert_ne!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn yank_version_route_exists() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.post("/v1/packages/acme/my-skill/1.0.0/yank").await;
    assert_ne!(response.status_code(), StatusCode::NOT_FOUND);
    assert_ne!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn yank_all_requires_auth() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.post("/v1/packages/acme/my-skill/yank").await;
    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn yank_version_requires_auth() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.post("/v1/packages/acme/my-skill/1.0.0/yank").await;
    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn yank_rejects_invalid_namespace() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server.post("/v1/packages/ACME/my-skill/yank").await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn yank_version_rejects_invalid_version() {
    let app = build_router(make_state().await);
    let server = TestServer::new(app).unwrap();
    let response = server
        .post("/v1/packages/acme/my-skill/1.0@bad/yank")
        .await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p skreg-api --test packages_test yank`
Expected: FAIL — routes return 404 (handlers/routes not yet defined), tests for 401/400 fail.

- [ ] **Step 3: Create the handler module**

Create `crates/skreg-api/src/handlers/yank.rs`. This step implements validation + auth + ownership and stubs the DB step to return `200 {yanked:0}` so the no-DB tests pass; the real DB logic lands in Task 2. Note validation runs **before** auth so invalid-input tests need no DB.

```rust
//! POST /v1/packages/:ns/:name/yank — yank all versions of a package
//! POST /v1/packages/:ns/:name/:version/yank — yank a single version
//!
//! "Yank" is a soft, reversible removal: it sets `versions.yanked_at = now()`.
//! Yanked versions are filtered out of every read path (metadata, download,
//! search), so they become uninstallable while the artifact stays in S3.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use log::error;
use serde::Serialize;
use skreg_core::types::{Namespace, PackageName};

use crate::handlers::packages::validate_version;
use crate::middleware::{extract_bearer, resolve_namespace};
use crate::router::{AppState, SharedState};

/// Response body for the yank endpoints.
#[derive(Debug, Serialize)]
pub struct YankResponse {
    /// Number of versions newly yanked by this call (0 if already yanked).
    pub yanked: i64,
}

/// Handle `POST /v1/packages/:ns/:name/yank` — yank all non-yanked versions.
///
/// # Errors
///
/// `400` invalid namespace/name, `401` missing/invalid key, `403` key namespace
/// mismatch, `404` package not found, `500` on DB error.
pub async fn yank_all_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    do_yank(&state, &ns_raw, &name_raw, None, &headers).await
}

/// Handle `POST /v1/packages/:ns/:name/:version/yank` — yank one version.
///
/// # Errors
///
/// `400` invalid namespace/name/version, `401` missing/invalid key, `403` key
/// namespace mismatch, `404` package/version not found, `500` on DB error.
pub async fn yank_version_handler(
    State(state): State<SharedState>,
    Path((ns_raw, name_raw, version_raw)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    do_yank(&state, &ns_raw, &name_raw, Some(&version_raw), &headers).await
}

/// Shared yank logic. `version = None` yanks all versions of the package.
async fn do_yank(
    state: &AppState,
    ns_raw: &str,
    name_raw: &str,
    version: Option<&str>,
    headers: &HeaderMap,
) -> Result<Json<YankResponse>, StatusCode> {
    // 1. Validate path params first (cheap, no DB).
    let ns = Namespace::new(ns_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pkg_name = PackageName::new(name_raw).map_err(|_| StatusCode::BAD_REQUEST)?;
    if let Some(v) = version {
        if !validate_version(v) || v == "latest" {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // 2. Authenticate.
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let raw_key = extract_bearer(auth).ok_or(StatusCode::UNAUTHORIZED)?;
    let (ns_id, ns_slug) = resolve_namespace(&state.pool, &raw_key).await?;

    // 3. Ownership: key must belong to the target namespace.
    if ns_slug != ns.as_str() {
        return Err(StatusCode::FORBIDDEN);
    }

    // 4. DB step — implemented in Task 2.
    let yanked = yank_versions(state, ns_id, pkg_name.as_str(), version).await?;
    Ok(Json(YankResponse { yanked }))
}

/// Set `yanked_at` on the target version(s); returns count newly yanked.
/// Stubbed in Task 1, implemented in Task 2.
async fn yank_versions(
    _state: &AppState,
    _ns_id: uuid::Uuid,
    _name: &str,
    _version: Option<&str>,
) -> Result<i64, StatusCode> {
    Ok(0)
}
```

Note: `validate_version` is declared `pub(crate)` in `packages.rs`, so it is reachable from this sibling module via `crate::handlers::packages::validate_version` with no visibility change. (`validate_version` already accepts `"latest"`; this handler additionally rejects `"latest"` since yanking a moving alias is meaningless.)

- [ ] **Step 4: Register the module**

In `crates/skreg-api/src/handlers/mod.rs`, add (keep alphabetical with siblings):

```rust
pub mod yank;
```

- [ ] **Step 5: Wire the routes**

In `crates/skreg-api/src/router.rs`, add the import alongside the other handler imports:

```rust
use crate::handlers::yank::{yank_all_handler, yank_version_handler};
```

Then add the two routes inside `Router::new()...`, immediately after the existing `/v1/download/.../sig` route and before `.nest_service("/", ...)`:

```rust
        .route("/v1/packages/:ns/:name/yank", post(yank_all_handler))
        .route(
            "/v1/packages/:ns/:name/:version/yank",
            post(yank_version_handler),
        )
```

(`post` is already imported in `router.rs`.)

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p skreg-api --test packages_test yank`
Expected: PASS for all six new tests (`*_route_exists` → not 404/405; `*_requires_auth` → 401; `*_rejects_invalid_*` → 400).

- [ ] **Step 7: Commit**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add crates/skreg-api/src/handlers/yank.rs crates/skreg-api/src/handlers/mod.rs crates/skreg-api/src/router.rs crates/skreg-api/tests/packages_test.rs
git commit -m "feat(api): yank routes with validation, auth, and ownership checks"
```

---

## Task 2: API yank DB logic

**Files:**
- Modify: `crates/skreg-api/src/handlers/yank.rs:yank_versions`

This is the SQL that sets `yanked_at`. It cannot be covered by automated tests (no live DB in CI), so there is no failing-test step; correctness is verified manually in Task 6 / Operational follow-up B. Keep the logic small and obviously correct.

- [ ] **Step 1: Replace the stub with real logic**

Replace the `yank_versions` stub in `crates/skreg-api/src/handlers/yank.rs` with:

```rust
/// Set `yanked_at` on the target version(s); returns the number of versions
/// newly yanked (already-yanked versions are not recounted). Idempotent.
async fn yank_versions(
    state: &AppState,
    ns_id: uuid::Uuid,
    name: &str,
    version: Option<&str>,
) -> Result<i64, StatusCode> {
    // Resolve the package id within the (already ownership-checked) namespace.
    let pkg_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM packages WHERE namespace_id = $1 AND name = $2",
    )
    .bind(ns_id)
    .bind(name)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let pkg_id = pkg_id.ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = version {
        // 404 if the version row does not exist at all.
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM versions WHERE package_id = $1 AND version = $2)",
        )
        .bind(pkg_id)
        .bind(v)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if !exists {
            return Err(StatusCode::NOT_FOUND);
        }

        let affected = sqlx::query(
            "UPDATE versions SET yanked_at = now()
             WHERE package_id = $1 AND version = $2 AND yanked_at IS NULL",
        )
        .bind(pkg_id)
        .bind(v)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .rows_affected();
        return Ok(i64::try_from(affected).unwrap_or(i64::MAX));
    }

    // 404 if the package has no versions at all.
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM versions WHERE package_id = $1")
        .bind(pkg_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("db: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if total == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    let affected = sqlx::query(
        "UPDATE versions SET yanked_at = now()
         WHERE package_id = $1 AND yanked_at IS NULL",
    )
    .bind(pkg_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        error!("db: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .rows_affected();
    Ok(i64::try_from(affected).unwrap_or(i64::MAX))
}
```

- [ ] **Step 2: Verify it compiles and the Task 1 tests still pass**

Run: `cargo test -p skreg-api --test packages_test yank`
Expected: PASS (the no-DB paths are unchanged; DB paths aren't exercised here).

Run: `cargo clippy -p skreg-api --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Commit**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add crates/skreg-api/src/handlers/yank.rs
git commit -m "feat(api): implement yank DB logic (all + single version, idempotent)"
```

---

## Task 3: Client `yank` method

**Files:**
- Modify: `crates/skreg-client/src/client.rs`
- Test: `crates/skreg-client/tests/client_test.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/skreg-client/tests/client_test.rs`:

```rust
#[tokio::test]
async fn http_client_yank_all_returns_error_on_bad_url() {
    let client = skreg_client::client::HttpRegistryClient::new("http://127.0.0.1:1");
    let result = client
        .yank("skreg_key", "acme", "my-skill", None)
        .await;
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p skreg-client --test client_test yank`
Expected: FAIL — `no method named yank`.

- [ ] **Step 3: Add the trait method**

In `crates/skreg-client/src/client.rs`, add to the `RegistryClient` trait (after `preview_package`):

```rust
    /// Yank a published package from the registry (soft removal).
    ///
    /// `version = None` yanks all versions; `Some(v)` yanks one version.
    /// Authenticates with `api_key` as a bearer token. Returns the number of
    /// versions newly yanked.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure or a non-success status.
    fn yank<'a>(
        &'a self,
        api_key: &'a str,
        ns: &'a str,
        name: &'a str,
        version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<u64, ClientError>>;
```

- [ ] **Step 4: Implement it for `HttpRegistryClient`**

In the `impl RegistryClient for HttpRegistryClient` block, after `preview_package`, add:

```rust
    fn yank<'a>(
        &'a self,
        api_key: &'a str,
        ns: &'a str,
        name: &'a str,
        version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<u64, ClientError>> {
        #[derive(serde::Deserialize)]
        struct YankResponse {
            yanked: u64,
        }

        Box::pin(async move {
            let url = match version {
                Some(v) => format!("{}/v1/packages/{ns}/{name}/{v}/yank", self.base_url),
                None => format!("{}/v1/packages/{ns}/{name}/yank", self.base_url),
            };
            debug!("yanking via {url}");

            let resp: YankResponse = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await?
                .error_for_status()
                .map_err(ClientError::Http)?
                .json()
                .await
                .map_err(|e| ClientError::Parse(e.to_string()))?;

            Ok(resp.yanked)
        })
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p skreg-client --test client_test yank`
Expected: PASS (connection refused → `Err`).

- [ ] **Step 6: Commit**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add crates/skreg-client/src/client.rs crates/skreg-client/tests/client_test.rs
git commit -m "feat(client): add RegistryClient::yank"
```

---

## Task 4: CLI `yank` command

**Files:**
- Create: `crates/skreg-cli/src/commands/yank.rs`
- Modify: `crates/skreg-cli/src/commands/mod.rs`
- Modify: `crates/skreg-cli/src/main.rs`

- [ ] **Step 1: Write the failing unit test for ref parsing**

Create `crates/skreg-cli/src/commands/yank.rs` with the parser and its test only (the `run_yank` body comes next):

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p skreg-cli yank`
Expected: FAIL — module `yank` not declared (compile error) until Step 4 wires `mod.rs`. (If you prefer a green compile first, do Step 4's `mod.rs` edit before running.)

- [ ] **Step 3: Add `run_yank`**

Append to `crates/skreg-cli/src/commands/yank.rs` (above the `#[cfg(test)]` block):

```rust
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
            println!("{}/{}@{v} was already yanked (nothing to do)", target.namespace, target.name);
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
            println!("Yanked {}/{} ({n} version(s))", target.namespace, target.name);
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Register the module**

In `crates/skreg-cli/src/commands/mod.rs`, add (keep alphabetical with siblings):

```rust
pub mod yank;
```

- [ ] **Step 5: Wire the CLI subcommand**

In `crates/skreg-cli/src/main.rs`, add a variant to the `Commands` enum (place after `Uninstall { ... }`):

```rust
    /// Remove a published skill from the registry (soft yank)
    Yank {
        /// Package reference (namespace/name or namespace/name@version)
        #[arg(value_name = "PACKAGE")]
        package_ref: String,
    },
```

And add the dispatch arm in `main` (place after the `Commands::Uninstall { .. }` arm):

```rust
        Commands::Yank { package_ref } => {
            skreg_cli::commands::yank::run_yank(&package_ref, cli.context.as_deref()).await?;
        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p skreg-cli yank`
Expected: PASS (3 parser tests).

Run: `cargo run -p skreg-cli -- yank --help`
Expected: help text shows the `PACKAGE` argument and the "soft yank" about line.

- [ ] **Step 7: Commit**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add crates/skreg-cli/src/commands/yank.rs crates/skreg-cli/src/commands/mod.rs crates/skreg-cli/src/main.rs
git commit -m "feat(cli): add 'skreg yank' command"
```

---

## Task 5: Documentation

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add the yank routes to the API Routes list**

In `CLAUDE.md`, in the `### API Routes (skreg-api)` code block, add these two lines after the `POST /v1/publish` line:

```
POST /v1/packages/:ns/:name/yank
POST /v1/packages/:ns/:name/:version/yank
```

- [ ] **Step 2: Commit**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add CLAUDE.md
git commit -m "docs: list yank routes in CLAUDE.md"
```

Note: the `using-skreg` skill's Quick Reference table is published from the `skreg` namespace, not this repo. Adding a `Yank` row there is part of a future skill re-publish (Operational follow-up), not this code change.

---

## Task 6: Full verification

**Files:** none (gates only)

- [ ] **Step 1: Format check**

Run: `cargo fmt --all --check`
Expected: no output (clean). If it fails, run `cargo fmt --all` and re-commit.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Full test suite**

Run: `cargo test --workspace`
Expected: all tests pass, including the new yank tests.

- [ ] **Step 4: Manual smoke against a local stack (optional but recommended)**

If a local API + Postgres is available (e.g. the `local` context), publish a throwaway package, then:

```bash
skreg --context local yank <ns>/<throwaway>          # expect: Yanked ... (N version(s))
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:18080/v1/packages/<ns>/<throwaway>/latest
# expect: 404 (yanked version no longer resolves)
skreg --context local yank <ns>/<throwaway>          # expect: already yanked (nothing to do)
```

- [ ] **Step 5: Final commit if anything changed during verification**

```bash
cd /home/dymo/skreg/.claude/worktrees/skreg-yank
git add -A
git commit -m "chore: verification fixups" || echo "nothing to commit"
```

---

## Post-merge operational follow-up (not code; see spec § Operational follow-ups B)

After this branch merges to `main` and CI runs `pulumi up` to deploy:

1. Authenticate to prod as `dymocaptin` (the local config only has a `dymocaptin`
   key for localhost): `skreg login dymocaptin` against `api.skreg.ai` (email OTP).
2. `skreg yank dymocaptin/using-skreg`
3. Verify: `curl -s -o /dev/null -w "%{http_code}\n" https://api.skreg.ai/v1/packages/dymocaptin/using-skreg/latest` → `404`,
   and `skreg search using-skreg` no longer lists the `dymocaptin` entry.

(Operational follow-up A — republishing `skreg/using-skreg` — is already done:
`skreg/using-skreg@1.0.3` is live and passing.)
