# skreg

A public skills package registry for AI coding tools — analogous to npm or crates.io for Claude Code skills (and any tool that adopts the format). Packages are vetted and cryptographically signed, so you know exactly what you're installing and where it came from.

## What's in this repo

| Crate / module | Language | Purpose |
|---|---|---|
| `crates/skillpkg-core` | Rust | Domain types: `Manifest`, `PackageRef`, `Namespace`, `Sha256Digest` |
| `crates/skillpkg-crypto` | Rust | Signature verification, revocation (CRL) |
| `crates/skillpkg-pack` | Rust | Pack/unpack `.skill` tarballs |
| `crates/skillpkg-client` | Rust | `RegistryClient` trait + HTTP adapter |
| `crates/skillpkg-cli` | Rust | `skillpkg` binary — install command |
| `crates/skreg-api` | Rust | Registry HTTP API (Axum + SQLx + Tokio) |
| `crates/skreg-worker` | Rust | Async vetting worker (structure checks, ClamAV) |
| `infra/` | Python | Pulumi IaC — provider-agnostic components, AWS implementation |

## Package format

A `.skill` file is a gzip'd tarball containing:

```
SKILL.md          # skill content with YAML frontmatter
manifest.json     # name, version, description, author, sha256, signature, cert chain
references/       # optional reference markdown files
```

Packages are content-addressed in storage: `/{namespace}/{name}/{version}/{sha256}.skill`

## Trust model

```
Root CA  (offline, HSM-backed)
    ├── Registry Intermediate CA
    │       Signs packages for individual publishers after vetting
    └── Publisher Intermediate CA
            Issues leaf certs to verified org accounts
            Orgs sign locally; registry verifies on ingest
```

The Root CA cert is embedded in the `skillpkg` CLI binary at build time. On install the CLI verifies the sha256 digest, the cert chain, the detached signature, and checks the CRL (cached 24 h).

## Install a skill

```sh
skillpkg install acme/my-skill
skillpkg install acme/my-skill@1.2.0
```

## Self-hosting

The registry API is a stateless Rust binary configured entirely via environment variables. The `infra/` Pulumi project provides provider-agnostic component interfaces with an AWS implementation (RDS + S3 + CloudFront). GCP and Azure implementations follow the same interface.

```sh
cd infra
uv sync --extra dev
pulumi up
```

## Development

**Prerequisites:** Rust stable, Python 3.12, [uv](https://github.com/astral-sh/uv), PostgreSQL 16

```sh
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings

# Python infra
cd infra
uv sync --extra dev
uv run pytest
uv run mypy src/
```

## License

TBD
