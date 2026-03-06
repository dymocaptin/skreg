# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is skreg?

skreg is a package registry for AI coding assistant skills — like npm for prompts. Users browse, install, and publish reusable instruction sets (`.skill` packages). The system enforces cryptographic trust from a Root CA through to signed packages.

## Commands

### Before every commit
```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

A pre-commit hook at `.githooks/pre-commit` enforces this automatically. Activate once per clone:
```bash
git config core.hooksPath .githooks
```

### Rust (primary codebase)
```bash
cargo build --workspace
cargo test --workspace
cargo test -p <crate-name>                      # Single crate
cargo test <test_name>                          # Single test (by name pattern)
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo fmt --all --check
```

### Python infra
```bash
cd infra
uv sync --extra dev
uv run pytest
uv run pytest tests/test_foo.py::test_bar       # Single test
uv run mypy src/
uv run ruff check src/
uv run black src/
```

### Pulumi deployment
```bash
cd infra
pulumi stack select main
pulumi up
```

## Architecture

### Rust Workspace Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `skreg-core` | lib | Domain types: `Manifest`, `PackageRef`, `Namespace`, `Sha256Digest` |
| `skreg-crypto` | lib | X.509 cert validation, signature verification, CRL handling |
| `skreg-pack` | lib | Pack/unpack `.skill` tarballs (gzip'd tar) |
| `skreg-client` | lib | `RegistryClient` trait + HTTP adapter |
| `skreg-cli` | bin | User-facing CLI (`skreg install`, `pack`, `publish`, `search`, `login`) |
| `skreg-api` | bin | Axum HTTP server (PostgreSQL + S3 backend) |
| `skreg-worker` | bin | Background vetting/signing pipeline |

### Package Format

`.skill` files are gzip'd tarballs containing:
- `SKILL.md` — Skill content with YAML frontmatter
- `manifest.json` — Name, version, description, sha256, signature, cert chain
- `references/` — Optional reference markdown files

Packages are stored content-addressed by SHA-256: `/{namespace}/{name}/{version}/{sha256}.skill`

### Trust Model

```
Root CA (offline, HSM-backed)  [certs/root-ca.pem, embedded in CLI at compile time]
├── Registry Intermediate CA   [signs packages post-vetting]
└── Publisher Intermediate CA  [issues leaf certs to verified orgs]
```

Every install verifies the full certificate chain against the root CA compiled into the binary.

### API Routes (`skreg-api`)

```
GET  /healthz
GET  /v1/search
POST /v1/namespaces
POST /v1/auth/login
POST /v1/auth/token
POST /v1/publish
GET  /v1/jobs/:id
GET  /v1/packages/:ns/:name/:version
GET  /v1/download/:ns/:name/:version
GET  /v1/download/:ns/:name/:version/sig
```

### Worker Pipeline (`skreg-worker`)

Stages in `skreg-worker/src/stages/`:
1. `content.rs` — Content validation
2. `safety.rs` — Safety checks (ClamAV)
3. `structure.rs` — Structure validation
4. `signing.rs` — Package signing with registry CA

### Infrastructure (`infra/`)

Python/Pulumi IaC targeting AWS:
- **Compute**: ECS Fargate (`skreg-api` + `skreg-worker`)
- **Database**: RDS PostgreSQL 16
- **Storage**: S3 + CloudFront CDN
- **PKI**: ACM + AWS Secrets Manager for CA keys
- **Email**: AWS SES v2

Components are provider-agnostic interfaces in `infra/src/skreg_infra/components/`; AWS implementations live in `infra/src/skreg_infra/providers/aws/`.

### CLI Config

- Config stored at `~/.skreg/config.toml`
- Packages installed to `~/.skreg/packages/`

## CI/CD

`.github/workflows/ci.yml` runs on every PR and push to `main`:
1. Rust: test, clippy, fmt
2. Python infra: mypy, ruff, black, pytest (90% coverage threshold enforced)
3. On `main` with Rust changes: build Docker images → push to ECR → `pulumi up`
