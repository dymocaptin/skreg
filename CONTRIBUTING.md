# Contributing to skreg

## Repository layout

| Crate / module | Language | Purpose |
|---|---|---|
| `crates/skreg-core` | Rust | Domain types: `Manifest`, `PackageRef`, `Namespace`, `Sha256Digest` |
| `crates/skreg-crypto` | Rust | Signature verification, revocation (CRL) |
| `crates/skreg-pack` | Rust | Pack/unpack `.skill` tarballs |
| `crates/skreg-client` | Rust | `RegistryClient` trait + HTTP adapter |
| `crates/skreg-cli` | Rust | `skreg` binary — install command |
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

The Root CA public certificate is committed to the repository at
`certs/root-ca.pem`. The `skreg` CLI embeds this certificate at build time and
uses it to verify the full cert chain on every install. The private key is kept
offline (HSM-backed) and is never committed.

To verify you have the correct certificate:

```sh
openssl x509 -in certs/root-ca.pem -noout -fingerprint -sha256
# Expected:
# SHA256 Fingerprint=B3:35:5B:0D:EB:F5:8D:35:8A:2E:7B:CA:39:FB:C2:2D:8D:60:29:B7:22:D1:30:37:B0:5C:78:AF:5E:80:94:F0
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

## Submitting changes

1. Fork the repository and create a branch from `main`.
2. Make your changes with tests where applicable.
3. Ensure `cargo test --workspace` and `uv run pytest` (in `infra/`) pass.
4. Open a pull request.
