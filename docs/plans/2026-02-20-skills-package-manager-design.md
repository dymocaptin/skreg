# skreg — Skills Registry: Design Document

**Date:** 2026-02-20
**Status:** Approved

---

## Overview

`skreg` is a public skills package registry — analogous to npm or crates.io — for AI coding tool
skills (Claude Code, and any other tool that adopts the format). It provides vetted, cryptographically
signed skill packages with a standalone CLI (`skillpkg`) and multi-cloud self-hosted deployment via
Pulumi.

**Value proposition:** Skills can be published, discovered, and consumed with integrity guarantees,
avoiding the risks of unknown or tampered skill sources propagating into unsuspecting environments.

---

## Decisions

| Concern | Decision | Rationale |
|---|---|---|
| Access model | Public registry + free self-hosted option | Maximum adoption; mirrors npm model |
| PKI trust | Tiered: registry-signed for individuals, publisher-signed for orgs | Low friction for early adopters; auditable for enterprise |
| Multi-cloud | Self-host parity via Pulumi (AWS / GCP / Azure) | Orgs deploy on their existing cloud |
| Vetting | Automated checks + community flagging (reactive moderation) | Fast publish; scales without large moderation team |
| Client | Standalone `skillpkg` CLI (Rust) | Not coupled to Claude Code; supports any AI tool |
| Registry API | Rust (Axum + SQLx + Tokio) | Single language across application layer |
| IaC | Pulumi Python (strict type-annotated, mypy --strict) | Provider-agnostic component model; Python SDK maturity |

---

## Section 1: Overall Architecture

The system has five logical layers:

```
┌─────────────────────────────────────────────────────┐
│  CLI (skillpkg)       Web UI (registry.skreg.dev)   │
└───────────────────────┬─────────────────────────────┘
                        │ HTTPS
┌───────────────────────▼─────────────────────────────┐
│              Registry API (stateless Rust service)   │
│   /publish  /install  /search  /yank  /audit        │
└────┬──────────────────┬──────────────────────┬──────┘
     │                  │                      │
┌────▼────┐    ┌────────▼──────┐    ┌──────────▼─────┐
│Postgres  │    │ Object Store  │    │  PKI Service    │
│(meta,   │    │ (S3/GCS/Blob) │    │  (CA + verify)  │
│index,   │    │ content-addr  │    │  HSM-backed key │
│keys)    │    │ by sha256     │    └────────────────-┘
└─────────┘    └───────────────┘
                        │
               ┌────────▼──────┐
               │   CDN layer    │
               │ (immutable     │
               │  package dist) │
               └───────────────┘
```

**Canonical public registry:** Hosted on AWS (primary), fronted by CloudFront for package downloads.

**Self-hosted deployments:** Pulumi modules for each provider wire up equivalent managed services.
The Registry API binary is cloud-agnostic, configured entirely via environment variables.

**Package format:** A `.skill` tarball (gzip'd tar) containing:
- `SKILL.md` — skill content with YAML frontmatter
- `references/` — optional reference markdown files
- `manifest.json` — name, version, description, author, sha256 digest, signature, cert chain

Content-addressed storage path: `/{namespace}/{name}/{version}/{sha256}.skill`

---

## Section 2: PKI & Trust Model

### CA Hierarchy

```
Root CA  (offline, HSM-backed, air-gapped)
    │
    ├── Registry Intermediate CA
    │       Signs packages on behalf of individual publishers
    │       after automated vetting passes
    │
    └── Publisher Intermediate CA
            Issues leaf certs to verified org accounts
                │
                └── Publisher Leaf Cert
                        Publisher signs locally before upload;
                        registry verifies on ingest
```

The Root CA private key never touches an online system. In the canonical deployment it lives in a
cloud HSM (CloudHSM / Cloud HSM / Azure Dedicated HSM). For self-hosted deployments a software key
stored in the provider's secret store is an accepted alternative. The Root CA cert is embedded in
the `skillpkg` CLI binary at build time.

### Publisher Tiers

| | Individual | Company / Org |
|---|---|---|
| Account type | Email + OIDC (GitHub / Google) | Domain-verified org account |
| Signing | Registry Intermediate CA signs on publish | Publisher signs with leaf cert before upload |
| Key custody | Registry holds signing key | Publisher owns and manages their key pair |
| Cert issuance | None — registry signs directly | PKI service issues leaf cert from Publisher Intermediate CA, valid 1 year |

### Signature Format

Detached signature over the package sha256 digest, stored alongside the tarball:
- `/{namespace}/{name}/{version}/{sha256}.sig`
- `manifest.json` embeds the signing cert chain

### CLI Verification Flow (on install)

1. Download `.skill` tarball + `.sig`
2. Verify tarball sha256 matches manifest
3. Load signing cert chain from manifest
4. Verify chain up to Root CA (embedded in CLI binary)
5. Verify detached signature
6. Check CRL (cached 24h; fetched fresh if stale)
7. Install

### Revocation

The Registry API serves a CRL refreshed on every yank or publisher ban. The CLI caches the last
known CRL for offline use with a 24-hour TTL.

### Key Rotation Schedule

| Key | Rotation period |
|---|---|
| Root CA | 10 years |
| Intermediates | 2 years |
| Publisher leaf certs | 1 year |

A `/.well-known/skreg-pki` endpoint serves the current cert bundle so clients can fetch updated
trust anchors without a CLI update.

---

## Section 3: Registry API

**Technology:** Rust — Axum (HTTP), SQLx (PostgreSQL), Tokio (async runtime). Single stateless
binary, horizontally scalable.

**Authentication:** OIDC (GitHub, Google, Microsoft). JWT access tokens (15 min TTL) + refresh
tokens in Postgres. API keys available for CI/CD, scoped to namespace.

### Endpoints

```
# Discovery
GET  /.well-known/skreg-pki
GET  /v1/search?q=&category=&page=
GET  /v1/packages/{namespace}/{name}
GET  /v1/packages/{namespace}/{name}/{version}

# Download (redirect or proxy to CDN)
GET  /v1/download/{namespace}/{name}/{version}
GET  /v1/download/{namespace}/{name}/{version}/sig

# Publish (authenticated)
POST   /v1/publish
DELETE /v1/packages/{namespace}/{name}/{version}   # yank

# Identity & PKI
POST /v1/auth/login
POST /v1/auth/token
POST /v1/namespaces
POST /v1/namespaces/{ns}/certs/issue
GET  /v1/namespaces/{ns}/certs/crl

# Moderation
POST /v1/packages/{ns}/{name}/report
GET  /v1/admin/reports
POST /v1/admin/yank
POST /v1/admin/ban
```

### Publish Flow

```
POST /v1/publish
    │
    ├── Auth check (JWT or API key)
    ├── Manifest schema validation
    ├── sha256 digest verification
    ├── If org: verify detached .sig against publisher cert chain
    ├── Write tarball + sig to object storage
    └── Enqueue vetting job → 202 Accepted + polling URL
            │
            └── On vetting pass:
                    If individual: Registry CA signs → write .sig
                    Mark version published → appears in search
```

### Immutability & Yanking

Versions are immutable once published. Yank soft-deletes (hides from search, returns 410 on
download with a message) but preserves content in object storage for audit purposes.

---

## Section 4: Storage Layer

### Object Storage

Content-addressed layout:

```
/{namespace}/{name}/{version}/
    {sha256}.skill      # gzip'd tarball
    {sha256}.sig        # detached signature
    manifest.json       # metadata + cert chain
```

| Concern | Policy |
|---|---|
| Versioning | Disabled (content-addressed) |
| Lifecycle | Never delete (yank = metadata flag only) |
| Access | Public read on download prefix; all writes via API service role |
| Encryption | SSE with provider-managed keys |
| CDN cache | `.skill` and `.sig`: `immutable, max-age=31536000`; `manifest.json`: `no-cache` |

### PostgreSQL Schema

```sql
CREATE TABLE namespaces (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT UNIQUE NOT NULL,
    kind        TEXT NOT NULL,          -- 'individual' | 'org'
    oidc_sub    TEXT UNIQUE,
    domain      TEXT,
    banned_at   TIMESTAMPTZ,
    created_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE publisher_certs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id    UUID REFERENCES namespaces(id),
    serial          BIGINT UNIQUE NOT NULL,
    pem             TEXT NOT NULL,
    issued_at       TIMESTAMPTZ DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ
);

CREATE TABLE packages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id    UUID REFERENCES namespaces(id),
    name            TEXT NOT NULL,
    description     TEXT,
    category        TEXT,
    created_at      TIMESTAMPTZ DEFAULT now(),
    UNIQUE (namespace_id, name)
);

CREATE TABLE versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id      UUID REFERENCES packages(id),
    version         TEXT NOT NULL,         -- semver
    sha256          TEXT NOT NULL,
    storage_path    TEXT NOT NULL,
    sig_path        TEXT NOT NULL,
    signer          TEXT NOT NULL,         -- 'registry' | cert serial
    published_at    TIMESTAMPTZ DEFAULT now(),
    yanked_at       TIMESTAMPTZ,
    yank_reason     TEXT,
    UNIQUE (package_id, version)
);

CREATE TABLE package_search (
    package_id      UUID REFERENCES packages(id) PRIMARY KEY,
    search_vector   TSVECTOR
);

CREATE TABLE vetting_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id      UUID REFERENCES versions(id),
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending|pass|fail|quarantined
    results         JSONB,
    created_at      TIMESTAMPTZ DEFAULT now(),
    completed_at    TIMESTAMPTZ
);

CREATE TABLE reports (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id      UUID REFERENCES versions(id),
    reason          TEXT NOT NULL,        -- malicious|misleading|spam|other
    detail          TEXT,
    reporter_ip     TEXT,
    created_at      TIMESTAMPTZ DEFAULT now(),
    resolved_at     TIMESTAMPTZ,
    resolution      TEXT
);
```

**Key indexes:** `packages(namespace_id, name)`, `versions(package_id, version)`,
GIN on `package_search.search_vector`.

**Migrations:** Managed by `sqlx migrate`. The API binary refuses to start if schema version does
not match expected — no silent drift.

---

## Section 5: Vetting Pipeline

**Architecture:** Separate `skreg-worker` process (same Rust binary, different entrypoint). Jobs
enqueued via `pg_notify` — no separate message queue in v1. Workers are stateless; Postgres
advisory locks prevent double-processing.

```
POST /v1/publish
    │
    ├── Synchronous (< 1s): schema, sha256, auth, sig
    └── 202 Accepted → job enqueued
            │
            ▼
    skreg-worker picks up job
            │
            ├── Stage 1: Structure (~1s)
            │       manifest.json required fields
            │       SKILL.md exists + valid YAML frontmatter
            │       name matches namespace/name in manifest
            │       no unexpected file types
            │       tarball size ≤ 5 MB
            │
            ├── Stage 2: Content (~5s)
            │       description field > 20 chars
            │       no hardcoded secrets (truffleHog patterns)
            │       no embedded binaries or compiled artifacts
            │       references/ contains .md files only
            │
            ├── Stage 3: Safety (~20s)
            │       ClamAV antivirus scan
            │       name squatting: Levenshtein ≤ 2 from top-100 → mod review
            │       exact name of yanked/banned package → auto-reject
            │
            └── Stage 4: Sign (individual publishers only)
                    Registry Intermediate CA signs sha256
                    .sig written to object storage
                    version status → published
```

### Vetting Outcomes

| Result | Action |
|---|---|
| All stages pass | Version published; visible in search |
| Stage 1–2 fail | Rejected; reason returned via polling URL |
| ClamAV hit | Quarantined; auto-reported to mod queue; publisher notified |
| Name squatting flag | Held `pending_review`; moderator notified; 48h SLA |
| Infrastructure error | Retry ×3 then page on-call |

### Community Flagging

`POST /v1/packages/{ns}/{name}/report` — no auth required, rate-limited (5 reports/IP/hour).
Three unique-IP reports within 24h → auto-surfaced to moderator queue at high priority.

Moderators act via the admin UI (server-rendered, served by the same API binary under `/admin`).

---

## Section 6: CLI Tool (`skillpkg`) — Rust

**Crate workspace:**

```
skillpkg/
├── Cargo.toml          # workspace root
├── Cargo.lock          # committed
├── crates/
│   ├── skillpkg-cli/   # binary: arg parsing, UX, command dispatch
│   ├── skillpkg-core/  # domain types, business logic
│   ├── skillpkg-crypto/# PKI, signing, verification
│   ├── skillpkg-client/# registry HTTP client adapter
│   └── skillpkg-pack/  # tarball pack/unpack, manifest
├── tests/
└── examples/
```

### Commands

```
skillpkg search <query>
skillpkg info <namespace/name>
skillpkg install <namespace/name[@version]>
skillpkg update
skillpkg list
skillpkg uninstall <namespace/name>

skillpkg publish
skillpkg yank <namespace/name@version>
skillpkg login
skillpkg logout
skillpkg whoami

skillpkg verify <namespace/name@version>
skillpkg audit

skillpkg org create <slug>
skillpkg org cert issue
skillpkg org cert rotate
skillpkg org cert revoke
```

### Install Verification Steps

1. Resolve latest non-yanked version
2. Download `.skill` tarball
3. Download `.sig`
4. Verify sha256 vs manifest
5. Verify cert chain (manifest → Root CA embedded in binary)
6. Verify detached signature
7. Check CRL (24h cached TTL)
8. Extract to install directory
9. Write lock entry

Any failure aborts the install; no partial installs.

### Lock File (`skillpkg.lock`)

```json
{
  "lockfileVersion": 1,
  "packages": {
    "acme/deploy-helper": {
      "version": "1.3.0",
      "sha256": "e3b0c44298fc1c149afb...",
      "signerKind": "publisher",
      "signerSerial": "42"
    },
    "jsmith/stripe-patterns": {
      "version": "2.0.1",
      "sha256": "a665a45920422f9d417e...",
      "signerKind": "registry"
    }
  }
}
```

### Rust Design Principles Applied

- Newtype wrappers (`Namespace`, `PackageName`, `Sha256Digest`) — invalid values unrepresentable
- Traits for all I/O boundaries (`SignatureVerifier`, `RevocationStore`, `RegistryClient`,
  `PackageStore`) — all injected via constructors; fully mockable in tests
- `thiserror` error enums per crate; `anyhow` only in the CLI binary
- Root CA cert embedded via `include_bytes!` at compile time; validated by build script
- `log` facade in library crates; `env_logger` backend in CLI binary
- `#![deny(warnings, clippy::all, clippy::pedantic)]` in all library crates
- No `.unwrap()` or `expect()` outside tests
- CI fails on warnings, unused imports, dead code

### Configuration (`~/.skillpkg/config.toml`)

```toml
registry = "https://registry.skreg.dev"
default_namespace = "jsmith"

[auth]
token_path = "~/.skillpkg/token"   # OS keychain if available

[verify]
crl_ttl_hours = 24
offline_mode = false
```

Self-hosted users point `registry` at their own endpoint — no other changes needed.

---

## Section 7: Infrastructure as Code (Pulumi Python)

**Language:** Python 3.12, mypy --strict, ruff + black, uv for dependency management.

### Project Structure

```
infra/
├── Pulumi.yaml
├── pyproject.toml
├── uv.lock                    # committed
├── src/
│   └── skillpkg_infra/
│       ├── __init__.py
│       ├── __main__.py        # SkillpkgStack().run() entry point
│       ├── config.py          # StackConfig via pydantic-settings (env vars)
│       ├── components/        # Protocol interfaces + output classes
│       │   ├── database.py
│       │   ├── storage.py
│       │   ├── pki.py
│       │   ├── compute.py
│       │   └── network.py
│       └── providers/
│           ├── aws/
│           ├── gcp/
│           └── azure/
├── tests/
└── stacks/
    ├── Pulumi.aws-prod.yaml
    ├── Pulumi.gcp-prod.yaml
    └── Pulumi.azure-prod.yaml
```

### Design Principles Applied

- `Protocol` classes define provider-agnostic component contracts; implementations satisfy
  them structurally (no inheritance)
- All instance attributes defined exclusively in `__init__`
- Configuration sourced entirely from environment variables via `pydantic-settings`
  (`SKILLPKG_` prefix); no `pulumi.Config()` calls in domain classes
- Structured logging via `structlog`; no `print` statements
- `SkillpkgStack` class owns composition and dispatch; `__main__.py` contains only imports,
  the logger, and `if __name__ == "__main__": SkillpkgStack(config=StackConfig.load()).run()`
- ≥ 90% test coverage enforced in CI via `pytest-cov`; Pulumi mock framework for unit tests
  without real cloud calls

### Per-Provider Managed Service Equivalents

| Component | AWS | GCP | Azure |
|---|---|---|---|
| PostgreSQL | RDS 16 | Cloud SQL 16 | Azure DB for PostgreSQL Flexible |
| Object storage | S3 | GCS | Azure Blob Storage |
| CDN | CloudFront | Cloud CDN | Azure Front Door |
| HSM | CloudHSM (PKCS#11) | Cloud HSM | Azure Dedicated HSM |
| Container runtime | ECS Fargate | Cloud Run | Azure Container Apps |
| Secrets | Secrets Manager | Secret Manager | Azure Key Vault |
| Container registry | ECR | Artifact Registry | Azure Container Registry |
| DNS | Route 53 | Cloud DNS | Azure DNS |

### Software HSM Fallback (Self-Hosted)

```yaml
# stacks/Pulumi.aws-selfhost.yaml
config:
  SKILLPKG_CLOUD_PROVIDER: aws
  SKILLPKG_HSM_BACKEND: software   # encrypted key in Secrets Manager
  SKILLPKG_IMAGE_URI: ghcr.io/skreg/registry:latest
```

`SKILLPKG_HSM_BACKEND=software` swaps CloudHSM for an encrypted key in the provider's secret
store — acceptable for private registries. HSM is the default for the canonical public registry.

---

## Deferred (Future Phases)

| Feature | Notes |
|---|---|
| OCI / ORAS artifact storage | Migrate package format to OCI artifacts; adopt Sigstore/cosign for publisher-first signing at scale |
| Layered countersignatures | Registry countersigns after vetting (Notary v2 style) |
| Namespace federation | Allow self-hosted registries to mirror from the public root |
| Web UI | Server-rendered package discovery and documentation pages |
| Webhooks | Publisher notifications on vetting outcomes, mod actions |
