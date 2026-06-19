# Design: First-class registry removal (`skreg yank`)

Date: 2026-06-16
Status: Approved

## Problem

`skreg uninstall` removes a skill from the local machine, but there is **no
first-class way to remove a _published_ skill from the registry**. A publisher
who ships a broken or wrong package cannot retract it.

The database schema already anticipates this: `versions.yanked_at` exists
(migration `001_initial.sql`), and every read path already filters it out
(`resolve_version_row` for metadata/download, and the search query). Nothing in
the API or CLI ever sets `yanked_at`, so the capability is half-built and
unreachable.

Concrete triggers (operational, see "Operational follow-ups" below):
- `dymocaptin/using-skreg` needs to be retired (the skill moved to the `skreg`
  namespace), and there is no command to do it.
- `skreg/using-skreg` exists as a package but has **no passing version**
  (`latest_version: null`, `/latest` → 404) — the published version failed
  vetting. It must be replaced with a working version.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Removal semantics | **Soft yank** (`yanked_at = now()`) | Reversible, preserves content-addressed immutability and the trust model; schema already supports it. Matches crates.io. |
| Authorization | **Namespace owner only** | A valid API key for `:ns` may yank that namespace's packages. Mirrors the existing publish auth model exactly. No admin role introduced. |
| CLI command name | **`skreg yank`** | crates.io convention; clearly distinct from local `skreg uninstall`; soft/reversible connotation. |
| Local cleanup of `dymocaptin/using-skreg` | **Out of scope** | This task is registry-only; the local install and verified-skills lists are left untouched. |

## Non-goals (YAGNI)

- No hard delete / S3 purge.
- No `unyank` command (the server stays capable of it via the column, but no CLI/endpoint is exposed).
- No admin/superuser override.
- No schema migration (the `yanked_at` column already exists).

## Architecture

Three layers, mirroring the existing publish path.

### 1. API — `skreg-api`

New handler module `crates/skreg-api/src/handlers/yank.rs`, wired in `router.rs`:

- `POST /v1/packages/:ns/:name/yank` — yank **all** currently non-yanked
  versions of the package ("remove the skill").
- `POST /v1/packages/:ns/:name/:version/yank` — yank a single version.

Behavior, shared by both:

1. Extract `Authorization: Bearer <key>` (`extract_bearer`); `401` if missing/empty.
2. Resolve namespace via `resolve_namespace(pool, key)` → `(ns_id, ns_slug)`;
   `401` if the key is unknown.
3. **Ownership check:** `403 FORBIDDEN` unless `ns_slug == :ns`.
4. Validate `:name` (`PackageName::new`) → `400`; validate `:version` with
   existing `validate_version` (single-version route only) → `400`.
5. Resolve the package row by `(ns_id, name)`; `404 NOT_FOUND` if absent.
6. `UPDATE versions SET yanked_at = now() WHERE package_id = $1 AND yanked_at IS NULL`
   (single-version route adds `AND version = $2`). Use the returned/affected row
   count.
   - Single-version route: if the version does not exist at all → `404`.
   - All-versions route: if the package exists but has zero versions → `404`.
7. `200 OK` with body `{ "yanked": N }` where `N` is the number of versions
   newly yanked (idempotent: re-yanking an already-yanked version yields
   `yanked: 0` and still `200`).

Effect: yanked versions immediately disappear from `GET /v1/packages/...`,
`GET /v1/download/...`, and search, because those queries already filter
`yanked_at IS NULL`.

Response struct:

```rust
#[derive(Debug, Serialize)]
pub struct YankResponse {
    pub yanked: i64,
}
```

### 2. Client — `skreg-client`

Add to the `RegistryClient` trait and `HttpRegistryClient`:

```rust
fn yank<'a>(
    &'a self,
    api_key: &'a str,
    ns: &'a str,
    name: &'a str,
    version: Option<&'a str>,
) -> BoxFuture<'a, Result<u64, ClientError>>;
```

- `version = Some(v)` → `POST {base}/v1/packages/{ns}/{name}/{v}/yank`.
- `version = None` → `POST {base}/v1/packages/{ns}/{name}/yank`.
- Sends `Authorization: Bearer {api_key}`; returns the `yanked` count.
- Maps non-2xx to `ClientError::Http` via `error_for_status`.

### 3. CLI — `skreg-cli`

New command module `crates/skreg-cli/src/commands/yank.rs`, wired in `main.rs`:

```
skreg yank <PACKAGE>     # namespace/name        -> yank all versions
skreg yank <PACKAGE>     # namespace/name@version -> yank one version
```

`run_yank(package_ref, context)`:

1. Load config (`load_config` + `apply_context`); error "not logged in — run
   `skreg login <namespace>` first" if absent.
2. Parse `package_ref` via `PackageRef::parse`; derive `namespace`, `name`,
   optional `version`.
3. Call the client `yank` with `cfg.api_key()`, against `cfg.registry()`.
4. Print confirmation, e.g. `Yanked dymocaptin/using-skreg (3 version(s))` or,
   for a single version, `Yanked dymocaptin/using-skreg@1.0.2`. If the server
   reports `yanked: 0`, print a note that nothing changed (already yanked).

Clap wiring in `main.rs`:

```rust
/// Remove a published skill from the registry (soft yank)
Yank {
    /// Package reference (namespace/name or namespace/name@version)
    #[arg(value_name = "PACKAGE")]
    package_ref: String,
},
```

## Error handling

| Condition | API status | CLI behavior |
|-----------|-----------|--------------|
| Missing/empty bearer token | 401 | error: not logged in |
| Unknown API key | 401 | error: authentication failed |
| Key namespace ≠ `:ns` | 403 | error: you can only yank packages in your own namespace |
| Bad name/version | 400 | error: invalid package reference |
| Package/version not found | 404 | error: not found in registry |
| Already fully yanked | 200, `{yanked:0}` | note: nothing to yank (already removed) |
| DB error | 500 | error: registry error |

## Testing

**API** (`crates/skreg-api/tests/` + handler unit tests):
- Yank a version → subsequent `package_meta_handler` / download returns 404.
- Yank all versions of a multi-version package → all hidden; `yanked` == count.
- Wrong-namespace key → 403, `yanked_at` unchanged.
- Missing package / missing version → 404.
- Idempotency: second yank returns `{yanked:0}`, still 200.
- Missing/invalid auth → 401.

**Client** (`crates/skreg-client/tests/`):
- `yank` builds the correct URL for `Some(version)` vs `None`, sends bearer
  header, returns the count (mock server).

**CLI** (unit tests in `commands/yank.rs`):
- Package-ref parsing splits `namespace/name@version` vs `namespace/name`.

All gates must pass: `cargo fmt --all --check`, `cargo clippy --all-targets -D
warnings`, `cargo test --workspace`.

## Operational follow-ups

These are registry operations against prod (`api.skreg.ai`), separate from the
code change. They are sequenced because the yank op depends on a deploy.

### A. Fix `skreg/using-skreg` (no code dependency — can run now)

The content is known-good (the current `dymocaptin/using-skreg@1.0.2` /
`~/.claude/skills/using-skreg/SKILL.md`, valid frontmatter incl.
`metadata.version`). The failure reason is not visible via the public API
(failed versions are filtered), but `skreg publish` polls the vetting job and
prints `job.message` on failure — so a republish attempt is itself the
diagnostic.

Steps:
1. Build a package dir: the known-good `SKILL.md` + a `manifest.json` with
   `namespace: skreg`, `name: using-skreg`, a version, matching description.
   Bump the version (e.g. `1.0.3`) so it is distinct from the failed upload, and
   set `metadata.version` in the frontmatter to match.
2. `skreg publish` using the **`skreg` prod credentials** (already present as the
   `default` context → `api.skreg.ai`).
3. If vetting fails, read the printed message, correct, and retry. If it passes,
   verify `GET /v1/packages/skreg/using-skreg/latest` → 200.

### B. Yank `dymocaptin/using-skreg` (depends on deploy of this feature)

The production registry runs the deployed container image, so `skreg yank` only
affects prod **after this branch merges to `main` and CI runs `pulumi up`**.

Steps:
1. Land this feature (PR → merge → CI deploy).
2. Authenticate to prod as `dymocaptin`. **Credential note:** the local config
   currently has a `dymocaptin` API key only for `localhost`; the `default`
   (prod) context is the `skreg` namespace. Yanking a `dymocaptin` package
   requires a `dymocaptin` key for prod — i.e. `skreg login dymocaptin`
   (email OTP) against `api.skreg.ai` first.
3. Run `skreg yank dymocaptin/using-skreg`.
4. Verify it no longer resolves: `skreg search using-skreg` /
   `GET /v1/packages/dymocaptin/using-skreg/latest` → 404.

Ordering: do **A** first (so a working `using-skreg` exists in `skreg`) before
**B** removes the `dymocaptin` copy, to avoid a window with no usable
`using-skreg` in the registry.

## Documentation

Update `CLAUDE.md` API routes list and the `using-skreg` skill's Quick Reference
table to mention `skreg yank`.
