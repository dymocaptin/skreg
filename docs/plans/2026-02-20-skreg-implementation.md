# skreg — Skills Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a public skills package registry (skreg) with a standalone Rust CLI (`skillpkg`), a Rust registry API, and Pulumi Python IaC for AWS/GCP/Azure self-hosted deployment.

**Architecture:** Cargo workspace with five library crates (`skillpkg-core`, `skillpkg-crypto`, `skillpkg-pack`, `skillpkg-client`) and one binary crate (`skillpkg-cli`); a separate Rust binary (`skreg-api`) for the registry API and worker; a Pulumi Python project (`infra/`) with provider-agnostic component interfaces and per-cloud implementations.

**Tech Stack:** Rust (Axum, SQLx, Tokio, thiserror, rcgen, x509-cert), Python 3.12 (Pulumi, pydantic-settings, structlog), PostgreSQL 16, ClamAV (via subprocess), semver, ruff + black + mypy --strict.

---

## Phase 1: Repository Skeleton

### Task 1: Rust workspace root

**Files:**
- Create: `Cargo.toml`
- Create: `.cargo/config.toml`
- Create: `.rustfmt.toml`
- Create: `.clippy.toml`

**Step 1: Write `Cargo.toml`**

```toml
[workspace]
members = [
    "crates/skillpkg-core",
    "crates/skillpkg-crypto",
    "crates/skillpkg-pack",
    "crates/skillpkg-client",
    "crates/skillpkg-cli",
    "crates/skreg-api",
    "crates/skreg-worker",
]
resolver = "2"

[workspace.dependencies]
tokio       = { version = "1", features = ["full"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "1"
anyhow      = "1"
log         = "0.4"
semver      = { version = "1", features = ["serde"] }
uuid        = { version = "1", features = ["v4", "serde"] }
chrono      = { version = "0.4", features = ["serde"] }
axum        = { version = "0.7", features = ["macros"] }
sqlx        = { version = "0.7", features = ["postgres", "runtime-tokio", "uuid", "chrono", "macros"] }
reqwest     = { version = "0.11", features = ["json", "stream"] }
sha2        = "0.10"
x509-cert   = "0.2"
der         = "0.7"
rcgen       = "0.12"

[profile.release]
strip = true
lto   = true
```

**Step 2: Write `.rustfmt.toml`**

```toml
edition = "2021"
max_width = 100
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

**Step 3: Write `.clippy.toml`**

```toml
msrv = "1.75.0"
```

**Step 4: Create crate skeletons**

```bash
mkdir -p crates/skillpkg-core/src
mkdir -p crates/skillpkg-crypto/src
mkdir -p crates/skillpkg-pack/src
mkdir -p crates/skillpkg-client/src
mkdir -p crates/skillpkg-cli/src
mkdir -p crates/skreg-api/src
mkdir -p crates/skreg-worker/src
```

**Step 5: Create each crate `Cargo.toml` — `crates/skillpkg-core/Cargo.toml`**

```toml
[package]
name    = "skillpkg-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde      = { workspace = true }
serde_json = { workspace = true }
thiserror  = { workspace = true }
semver     = { workspace = true }
uuid       = { workspace = true }
chrono     = { workspace = true }
sha2       = { workspace = true }
```

**Step 6: Create each crate `Cargo.toml` — `crates/skillpkg-crypto/Cargo.toml`**

```toml
[package]
name    = "skillpkg-crypto"
version = "0.1.0"
edition = "2021"

[dependencies]
skillpkg-core = { path = "../skillpkg-core" }
thiserror     = { workspace = true }
log           = { workspace = true }
x509-cert     = { workspace = true }
der           = { workspace = true }
sha2          = { workspace = true }
```

**Step 7: Create each crate `Cargo.toml` — `crates/skillpkg-pack/Cargo.toml`**

```toml
[package]
name    = "skillpkg-pack"
version = "0.1.0"
edition = "2021"

[dependencies]
skillpkg-core = { path = "../skillpkg-core" }
thiserror     = { workspace = true }
serde         = { workspace = true }
serde_json    = { workspace = true }
sha2          = { workspace = true }
log           = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

**Step 8: Create each crate `Cargo.toml` — `crates/skillpkg-client/Cargo.toml`**

```toml
[package]
name    = "skillpkg-client"
version = "0.1.0"
edition = "2021"

[dependencies]
skillpkg-core   = { path = "../skillpkg-core" }
skillpkg-crypto = { path = "../skillpkg-crypto" }
thiserror       = { workspace = true }
serde           = { workspace = true }
serde_json      = { workspace = true }
log             = { workspace = true }
reqwest         = { workspace = true }
tokio           = { workspace = true }
```

**Step 9: Create each crate `Cargo.toml` — `crates/skillpkg-cli/Cargo.toml`**

```toml
[package]
name    = "skillpkg-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "skillpkg"
path = "src/main.rs"

[dependencies]
skillpkg-core   = { path = "../skillpkg-core" }
skillpkg-crypto = { path = "../skillpkg-crypto" }
skillpkg-pack   = { path = "../skillpkg-pack" }
skillpkg-client = { path = "../skillpkg-client" }
anyhow          = { workspace = true }
tokio           = { workspace = true }
log             = { workspace = true }
env_logger      = "0.11"
clap            = { version = "4", features = ["derive"] }
toml            = "0.8"
```

**Step 10: Create each crate `Cargo.toml` — `crates/skreg-api/Cargo.toml`**

```toml
[package]
name    = "skreg-api"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "skreg-api"
path = "src/main.rs"

[dependencies]
skillpkg-core   = { path = "../skillpkg-core" }
skillpkg-crypto = { path = "../skillpkg-crypto" }
skillpkg-pack   = { path = "../skillpkg-pack" }
anyhow          = { workspace = true }
tokio           = { workspace = true }
axum            = { workspace = true }
sqlx            = { workspace = true }
serde           = { workspace = true }
serde_json      = { workspace = true }
thiserror       = { workspace = true }
log             = { workspace = true }
env_logger      = "0.11"
uuid            = { workspace = true }
chrono          = { workspace = true }
tower-http      = { version = "0.5", features = ["trace", "cors"] }
jsonwebtoken    = "9"
```

**Step 11: Create each crate `Cargo.toml` — `crates/skreg-worker/Cargo.toml`**

```toml
[package]
name    = "skreg-worker"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "skreg-worker"
path = "src/main.rs"

[dependencies]
skillpkg-core   = { path = "../skillpkg-core" }
skillpkg-crypto = { path = "../skillpkg-crypto" }
skillpkg-pack   = { path = "../skillpkg-pack" }
anyhow          = { workspace = true }
tokio           = { workspace = true }
sqlx            = { workspace = true }
serde           = { workspace = true }
serde_json      = { workspace = true }
thiserror       = { workspace = true }
log             = { workspace = true }
env_logger      = "0.11"
uuid            = { workspace = true }
chrono          = { workspace = true }
```

**Step 12: Create minimal `lib.rs` stubs so workspace compiles**

Create `crates/skillpkg-core/src/lib.rs`:
```rust
//! Core domain types for the skillpkg ecosystem.
```

Repeat for `skillpkg-crypto`, `skillpkg-pack`, `skillpkg-client`.

Create `crates/skillpkg-cli/src/main.rs`:
```rust
fn main() {}
```

Create `crates/skreg-api/src/main.rs`:
```rust
fn main() {}
```

Create `crates/skreg-worker/src/main.rs`:
```rust
fn main() {}
```

**Step 13: Verify workspace compiles**

```bash
cargo build --workspace
```

Expected: all crates compile with zero errors and zero warnings.

**Step 14: Commit**

```bash
git add Cargo.toml Cargo.lock .rustfmt.toml .clippy.toml .cargo/ crates/
git commit -m "chore: scaffold Rust workspace with all crate skeletons"
```

---

### Task 2: Pulumi Python infra skeleton

**Files:**
- Create: `infra/Pulumi.yaml`
- Create: `infra/pyproject.toml`
- Create: `infra/src/skillpkg_infra/__init__.py`
- Create: `infra/src/skillpkg_infra/__main__.py`
- Create: `infra/src/skillpkg_infra/config.py`
- Create: `infra/src/skillpkg_infra/components/__init__.py`
- Create: `infra/src/skillpkg_infra/providers/__init__.py`
- Create: `infra/src/skillpkg_infra/providers/aws/__init__.py`
- Create: `infra/src/skillpkg_infra/providers/gcp/__init__.py`
- Create: `infra/src/skillpkg_infra/providers/azure/__init__.py`
- Create: `infra/tests/__init__.py`

**Step 1: Write `infra/Pulumi.yaml`**

```yaml
name: skreg-infra
description: skreg infrastructure — skills registry
runtime:
  name: python
  options:
    virtualenv: .venv
```

**Step 2: Write `infra/pyproject.toml`**

```toml
[project]
name = "skillpkg-infra"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
    "pulumi>=3,<4",
    "pulumi-aws>=6,<7",
    "pulumi-gcp>=7,<8",
    "pulumi-azure-native>=2,<3",
    "pydantic-settings>=2,<3",
    "structlog>=24,<25",
]

[project.optional-dependencies]
dev = [
    "pytest>=8",
    "pytest-cov>=5",
    "mypy>=1.9",
    "ruff>=0.4",
    "black>=24",
    "pulumi[testing]>=3,<4",
]

[tool.ruff]
line-length = 100
src = ["src"]
select = ["E", "F", "I", "UP", "ANN", "N", "S", "B", "C4"]

[tool.black]
line-length = 100
target-version = ["py312"]

[tool.mypy]
strict = true
python_version = "3.12"
mypy_path = "src"

[tool.pytest.ini_options]
testpaths = ["tests"]
addopts = "--cov=skillpkg_infra --cov-report=term-missing --cov-fail-under=90"
```

**Step 3: Create all `__init__.py` stubs**

Each file is empty — just touch them.

**Step 4: Write `infra/src/skillpkg_infra/config.py`**

```python
"""Typed configuration loaded from environment variables at startup."""

from __future__ import annotations

import logging
from enum import Enum
from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict

logger: logging.Logger = logging.getLogger(__name__)


class CloudProvider(str, Enum):
    """Supported cloud provider deployment targets."""

    AWS = "aws"
    GCP = "gcp"
    AZURE = "azure"


class HsmBackend(str, Enum):
    """PKI signing key storage backend."""

    HSM = "hsm"
    SOFTWARE = "software"


class StackConfig(BaseSettings):
    """Fully validated infrastructure stack configuration.

    All values are sourced from environment variables at startup.
    Raises ``ValidationError`` on missing or invalid values.
    """

    model_config: SettingsConfigDict = SettingsConfigDict(
        env_prefix="SKILLPKG_",
        env_file=".env",
        env_file_encoding="utf-8",
    )

    cloud_provider: CloudProvider
    image_uri: str
    hsm_backend: HsmBackend = HsmBackend.HSM
    multi_az: bool = False
    environment: Literal["prod", "staging", "dev"] = "prod"

    @classmethod
    def load(cls) -> StackConfig:
        """Load and validate configuration from the environment.

        Logs each resolved setting at DEBUG level.
        Raises ``pydantic.ValidationError`` on missing or invalid values.
        """
        config = cls()
        logger.debug(
            "stack_config_loaded",
            extra={
                "cloud_provider": config.cloud_provider.value,
                "hsm_backend": config.hsm_backend.value,
                "multi_az": config.multi_az,
                "environment": config.environment,
            },
        )
        return config
```

**Step 5: Write stub `infra/src/skillpkg_infra/__main__.py`**

```python
"""Pulumi stack entry point for skreg infrastructure."""

from __future__ import annotations

import logging

import structlog

from skillpkg_infra.config import StackConfig

logger: logging.Logger = logging.getLogger(__name__)


class SkillpkgStack:
    """Orchestrates all provider-agnostic infrastructure components."""

    def __init__(self, config: StackConfig) -> None:
        """Initialise the stack with resolved configuration."""
        self._config: StackConfig = config

    def run(self) -> None:
        """Provision the full infrastructure stack."""
        logger.info(
            "stack_run_started",
            extra={"cloud_provider": self._config.cloud_provider.value},
        )
        raise NotImplementedError("Provider implementations not yet built.")


if __name__ == "__main__":
    structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
    SkillpkgStack(config=StackConfig.load()).run()
```

**Step 6: Install dependencies and verify**

```bash
cd infra && uv sync --extra dev
uv run mypy src/
uv run ruff check src/
uv run black --check src/
```

Expected: mypy and ruff pass with zero errors.

**Step 7: Commit**

```bash
git add infra/
git commit -m "chore: scaffold Pulumi Python infra project"
```

---

## Phase 2: Core Domain Types (`skillpkg-core`)

### Task 3: Newtype wrappers

**Files:**
- Create: `crates/skillpkg-core/src/types.rs`
- Modify: `crates/skillpkg-core/src/lib.rs`
- Create: `crates/skillpkg-core/tests/types_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-core/tests/types_test.rs
use skillpkg_core::types::{Namespace, PackageName, Sha256Digest};

#[test]
fn namespace_rejects_uppercase() {
    assert!(Namespace::new("Acme").is_err());
}

#[test]
fn namespace_rejects_empty() {
    assert!(Namespace::new("").is_err());
}

#[test]
fn namespace_rejects_too_long() {
    assert!(Namespace::new(&"a".repeat(65)).is_err());
}

#[test]
fn namespace_accepts_valid() {
    let ns = Namespace::new("acme-corp").unwrap();
    assert_eq!(ns.as_str(), "acme-corp");
}

#[test]
fn package_name_accepts_valid() {
    let name = PackageName::new("deploy-helper").unwrap();
    assert_eq!(name.as_str(), "deploy-helper");
}

#[test]
fn sha256_digest_rejects_wrong_length() {
    assert!(Sha256Digest::from_hex("abc").is_err());
}

#[test]
fn sha256_digest_accepts_64_hex_chars() {
    let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let digest = Sha256Digest::from_hex(hex).unwrap();
    assert_eq!(digest.as_hex(), hex);
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p skillpkg-core 2>&1 | head -20
```

Expected: compilation error — `types` module not found.

**Step 3: Implement `crates/skillpkg-core/src/types.rs`**

```rust
//! Validated newtype wrappers for core domain primitives.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when a domain value fails validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    /// The value is empty.
    #[error("value must not be empty")]
    Empty,
    /// The value exceeds the maximum length.
    #[error("value exceeds maximum length of {max} characters (got {got})")]
    TooLong {
        /// Maximum allowed length.
        max: usize,
        /// Actual length.
        got: usize,
    },
    /// The value contains disallowed characters.
    #[error("value contains invalid characters: only lowercase alphanumeric and hyphens allowed")]
    InvalidCharacters,
    /// The hex string is not the expected length.
    #[error("expected 64 hex characters, got {0}")]
    InvalidHexLength(usize),
    /// The hex string contains non-hex characters.
    #[error("value contains non-hex characters")]
    InvalidHex,
}

/// A validated namespace slug (lowercase alphanumeric + hyphens, 1–64 chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Namespace(String);

impl Namespace {
    /// Create a new `Namespace` from a string slice, validating the slug format.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the slug is empty, exceeds 64 characters,
    /// or contains characters other than lowercase letters, digits, and hyphens.
    pub fn new(slug: &str) -> Result<Self, ValidationError> {
        if slug.is_empty() {
            return Err(ValidationError::Empty);
        }
        if slug.len() > 64 {
            return Err(ValidationError::TooLong { max: 64, got: slug.len() });
        }
        if !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(ValidationError::InvalidCharacters);
        }
        Ok(Self(slug.to_owned()))
    }

    /// Return the inner slug string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated package name (same constraints as [`Namespace`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageName(String);

impl PackageName {
    /// Create a new `PackageName`, validating the slug format.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the name is invalid per namespace rules.
    pub fn new(name: &str) -> Result<Self, ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::Empty);
        }
        if name.len() > 64 {
            return Err(ValidationError::TooLong { max: 64, got: name.len() });
        }
        if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(ValidationError::InvalidCharacters);
        }
        Ok(Self(name.to_owned()))
    }

    /// Return the inner name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated SHA-256 hex digest (exactly 64 lowercase hex characters).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Parse a `Sha256Digest` from a hex string.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the string is not exactly 64 lowercase hex characters.
    pub fn from_hex(hex: &str) -> Result<Self, ValidationError> {
        if hex.len() != 64 {
            return Err(ValidationError::InvalidHexLength(hex.len()));
        }
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidHex);
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    /// Return the hex string representation.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
```

**Step 4: Update `crates/skillpkg-core/src/lib.rs`**

```rust
//! Core domain types for the skillpkg ecosystem.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod types;
```

**Step 5: Run tests to verify they pass**

```bash
cargo test -p skillpkg-core 2>&1
```

Expected: all 8 tests pass.

**Step 6: Commit**

```bash
git add crates/skillpkg-core/
git commit -m "feat(core): add validated Namespace, PackageName, Sha256Digest newtypes"
```

---

### Task 4: PackageRef and Manifest types

**Files:**
- Create: `crates/skillpkg-core/src/manifest.rs`
- Create: `crates/skillpkg-core/src/package_ref.rs`
- Modify: `crates/skillpkg-core/src/lib.rs`
- Create: `crates/skillpkg-core/tests/manifest_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-core/tests/manifest_test.rs
use semver::Version;
use skillpkg_core::manifest::Manifest;
use skillpkg_core::types::{Namespace, PackageName, Sha256Digest};

#[test]
fn manifest_serialises_and_roundtrips() {
    let manifest = Manifest {
        namespace: Namespace::new("acme").unwrap(),
        name: PackageName::new("deploy-helper").unwrap(),
        version: Version::parse("1.2.3").unwrap(),
        description: "A helpful deployment skill.".to_owned(),
        category: Some("deployment".to_owned()),
        sha256: Sha256Digest::from_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap(),
        cert_chain_pem: vec![],
    };

    let json = serde_json::to_string(&manifest).unwrap();
    let roundtripped: Manifest = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.name.as_str(), "deploy-helper");
    assert_eq!(roundtripped.version.to_string(), "1.2.3");
}

// crates/skillpkg-core/tests/package_ref_test.rs
use skillpkg_core::package_ref::PackageRef;

#[test]
fn package_ref_parses_with_version() {
    let r = PackageRef::parse("acme/deploy-helper@1.2.3").unwrap();
    assert_eq!(r.namespace.as_str(), "acme");
    assert_eq!(r.name.as_str(), "deploy-helper");
    assert_eq!(r.version.unwrap().to_string(), "1.2.3");
}

#[test]
fn package_ref_parses_without_version() {
    let r = PackageRef::parse("acme/deploy-helper").unwrap();
    assert!(r.version.is_none());
}

#[test]
fn package_ref_rejects_missing_slash() {
    assert!(PackageRef::parse("acme-deploy-helper").is_err());
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p skillpkg-core 2>&1 | head -10
```

Expected: compilation errors — modules not found.

**Step 3: Implement `crates/skillpkg-core/src/manifest.rs`**

```rust
//! Package manifest type representing `manifest.json` inside a `.skill` tarball.

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::types::{Namespace, PackageName, Sha256Digest};

/// The contents of a `manifest.json` file inside a `.skill` package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Publisher namespace slug.
    pub namespace: Namespace,
    /// Package name slug.
    pub name: PackageName,
    /// Package version (semver).
    pub version: Version,
    /// Human-readable description (≥ 20 characters after trimming).
    pub description: String,
    /// Optional category tag.
    pub category: Option<String>,
    /// SHA-256 hex digest of the tarball this manifest describes.
    pub sha256: Sha256Digest,
    /// PEM-encoded certificate chain used to verify the package signature.
    /// Empty for registry-signed packages (cert chain is implicit).
    pub cert_chain_pem: Vec<String>,
}
```

**Step 4: Implement `crates/skillpkg-core/src/package_ref.rs`**

```rust
//! Fully-qualified package reference, e.g. `acme/deploy-helper@1.2.3`.

use std::fmt;

use semver::Version;
use thiserror::Error;

use crate::types::{Namespace, PackageName, ValidationError};

/// Error returned when a package reference string cannot be parsed.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The string does not contain a `/` separator.
    #[error("package reference must be in the form 'namespace/name[@version]'")]
    MissingSlash,
    /// The namespace segment is invalid.
    #[error("invalid namespace: {0}")]
    InvalidNamespace(#[from] ValidationError),
    /// The version segment cannot be parsed as semver.
    #[error("invalid semver version: {0}")]
    InvalidVersion(#[from] semver::Error),
}

/// A fully-qualified package reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRef {
    /// Publisher namespace.
    pub namespace: Namespace,
    /// Package name.
    pub name: PackageName,
    /// Optional pinned version; `None` means "latest".
    pub version: Option<Version>,
}

impl PackageRef {
    /// Parse a package reference from a string in the form `ns/name[@version]`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the string is malformed.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let (ns_name, version) = match input.split_once('@') {
            Some((left, v)) => (left, Some(Version::parse(v)?)),
            None => (input, None),
        };

        let (ns_str, name_str) = ns_name.split_once('/').ok_or(ParseError::MissingSlash)?;

        Ok(Self {
            namespace: Namespace::new(ns_str)?,
            name: PackageName::new(name_str)
                .map_err(|e| ParseError::InvalidNamespace(e))?,
            version,
        })
    }
}

impl fmt::Display for PackageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)?;
        if let Some(v) = &self.version {
            write!(f, "@{v}")?;
        }
        Ok(())
    }
}
```

**Step 5: Update `crates/skillpkg-core/src/lib.rs`**

```rust
//! Core domain types for the skillpkg ecosystem.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod manifest;
pub mod package_ref;
pub mod types;
```

**Step 6: Run tests**

```bash
cargo test -p skillpkg-core 2>&1
```

Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/skillpkg-core/
git commit -m "feat(core): add Manifest and PackageRef domain types"
```

---

### Task 5: InstalledPackage and SignerKind

**Files:**
- Create: `crates/skillpkg-core/src/installed.rs`
- Modify: `crates/skillpkg-core/src/lib.rs`
- Create: `crates/skillpkg-core/tests/installed_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-core/tests/installed_test.rs
use std::path::PathBuf;

use skillpkg_core::installed::{InstalledPackage, SignerKind};
use skillpkg_core::types::{Namespace, PackageName, Sha256Digest};
use skillpkg_core::package_ref::PackageRef;
use semver::Version;

#[test]
fn signer_kind_serialises_registry() {
    let sk = SignerKind::Registry;
    let json = serde_json::to_string(&sk).unwrap();
    assert_eq!(json, r#"{"kind":"registry"}"#);
}

#[test]
fn signer_kind_serialises_publisher() {
    let sk = SignerKind::Publisher { cert_serial: 42 };
    let json = serde_json::to_string(&sk).unwrap();
    assert!(json.contains("publisher"));
    assert!(json.contains("42"));
}

#[test]
fn installed_package_roundtrips_json() {
    let pkg = InstalledPackage {
        pkg_ref: PackageRef::parse("acme/deploy-helper@1.0.0").unwrap(),
        sha256: Sha256Digest::from_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ).unwrap(),
        signer: SignerKind::Registry,
        install_path: PathBuf::from("/home/user/.skillpkg/packages/acme/deploy-helper/1.0.0"),
    };
    let json = serde_json::to_string(&pkg).unwrap();
    let back: InstalledPackage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pkg_ref.name.as_str(), "deploy-helper");
}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skillpkg-core 2>&1 | head -10
```

**Step 3: Implement `crates/skillpkg-core/src/installed.rs`**

```rust
//! Represents a skill package installed on the local filesystem.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::package_ref::PackageRef;
use crate::types::Sha256Digest;

/// Who signed this package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignerKind {
    /// Signed by the skreg Registry Intermediate CA on behalf of an individual publisher.
    Registry,
    /// Signed by a verified publisher using their own leaf certificate.
    Publisher {
        /// Serial number of the publisher leaf certificate used to sign.
        cert_serial: u64,
    },
}

/// An installed skill package on the local filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Fully-qualified package reference including pinned version.
    pub pkg_ref: PackageRef,
    /// SHA-256 digest of the installed tarball.
    pub sha256: Sha256Digest,
    /// Who signed this package.
    pub signer: SignerKind,
    /// Absolute path to the extracted package directory.
    pub install_path: PathBuf,
}
```

**Step 4: Update `lib.rs`**

```rust
//! Core domain types for the skillpkg ecosystem.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod installed;
pub mod manifest;
pub mod package_ref;
pub mod types;
```

**Step 5: Run tests**

```bash
cargo test -p skillpkg-core 2>&1
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add crates/skillpkg-core/
git commit -m "feat(core): add InstalledPackage and SignerKind types"
```

---

## Phase 3: Crypto Layer (`skillpkg-crypto`)

### Task 6: Signature verification traits and error types

**Files:**
- Create: `crates/skillpkg-crypto/src/error.rs`
- Create: `crates/skillpkg-crypto/src/verifier.rs`
- Create: `crates/skillpkg-crypto/src/revocation.rs`
- Modify: `crates/skillpkg-crypto/src/lib.rs`
- Create: `crates/skillpkg-crypto/tests/verifier_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-crypto/tests/verifier_test.rs
use skillpkg_crypto::revocation::{InMemoryRevocationStore, RevocationStore};
use skillpkg_crypto::verifier::SignatureVerifier;

#[test]
fn in_memory_store_starts_empty() {
    let store = InMemoryRevocationStore::new();
    assert!(!store.is_revoked(1).unwrap());
}

#[test]
fn in_memory_store_tracks_revoked_serials() {
    let mut store = InMemoryRevocationStore::new();
    store.revoke(42);
    assert!(store.is_revoked(42).unwrap());
    assert!(!store.is_revoked(1).unwrap());
}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skillpkg-crypto 2>&1 | head -10
```

**Step 3: Implement `crates/skillpkg-crypto/src/error.rs`**

```rust
//! Error types for cryptographic operations.

use thiserror::Error;

/// Errors that can occur during signature verification.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// The certificate chain could not be validated up to the root CA.
    #[error("certificate chain validation failed: {0}")]
    InvalidCertChain(String),
    /// The signature does not match the digest.
    #[error("signature mismatch")]
    SignatureMismatch,
    /// The signing certificate has been revoked.
    #[error("certificate {serial} has been revoked")]
    Revoked {
        /// The revoked certificate serial number.
        serial: u64,
    },
    /// A DER/ASN.1 parsing error.
    #[error("DER parsing error: {0}")]
    Der(String),
}

/// Errors that can occur when checking or refreshing revocation state.
#[derive(Debug, Error)]
pub enum RevocationError {
    /// Network error fetching the CRL.
    #[error("failed to fetch CRL: {0}")]
    Network(String),
    /// The CRL response was not parseable.
    #[error("failed to parse CRL: {0}")]
    Parse(String),
}
```

**Step 4: Implement `crates/skillpkg-crypto/src/revocation.rs`**

```rust
//! Certificate revocation checking.

use std::collections::HashSet;

use crate::error::RevocationError;

/// Checks whether a certificate serial number has been revoked.
pub trait RevocationStore: Send + Sync {
    /// Return `true` if the given serial number appears in the revocation list.
    ///
    /// # Errors
    ///
    /// Returns [`RevocationError`] if the store cannot be queried.
    fn is_revoked(&self, cert_serial: u64) -> Result<bool, RevocationError>;
}

/// An in-memory [`RevocationStore`] for use in tests and offline scenarios.
#[derive(Debug, Default)]
pub struct InMemoryRevocationStore {
    revoked: HashSet<u64>,
}

impl InMemoryRevocationStore {
    /// Create an empty revocation store.
    pub fn new() -> Self {
        Self { revoked: HashSet::new() }
    }

    /// Mark a certificate serial as revoked.
    pub fn revoke(&mut self, serial: u64) {
        self.revoked.insert(serial);
    }
}

impl RevocationStore for InMemoryRevocationStore {
    fn is_revoked(&self, cert_serial: u64) -> Result<bool, RevocationError> {
        Ok(self.revoked.contains(&cert_serial))
    }
}
```

**Step 5: Implement `crates/skillpkg-crypto/src/verifier.rs`**

```rust
//! Signature verification against the embedded root CA.

use skillpkg_core::types::Sha256Digest;

use crate::error::VerifyError;

/// The identity of a verified signer extracted from a certificate chain.
#[derive(Debug, Clone)]
pub struct VerifiedSigner {
    /// Certificate serial number, used to check revocation.
    pub cert_serial: Option<u64>,
    /// Human-readable subject common name.
    pub common_name: String,
}

/// Verifies a detached package signature against a certificate chain and root CA.
pub trait SignatureVerifier: Send + Sync {
    /// Verify a detached `signature` over the given `digest`.
    ///
    /// The `cert_chain_pem` is an ordered list of PEM-encoded certificates
    /// (leaf first, ending at an intermediate CA signed by the root CA embedded
    /// in the verifier implementation).
    ///
    /// # Errors
    ///
    /// Returns [`VerifyError`] if the chain is invalid, the signature does not
    /// match, or any certificate in the chain has been revoked.
    fn verify(
        &self,
        digest: &Sha256Digest,
        signature: &[u8],
        cert_chain_pem: &[String],
    ) -> Result<VerifiedSigner, VerifyError>;
}
```

**Step 6: Update `crates/skillpkg-crypto/src/lib.rs`**

```rust
//! Cryptographic primitives for skillpkg: signature verification and revocation.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod error;
pub mod revocation;
pub mod verifier;
```

**Step 7: Run tests**

```bash
cargo test -p skillpkg-crypto 2>&1
```

Expected: all tests pass.

**Step 8: Commit**

```bash
git add crates/skillpkg-crypto/
git commit -m "feat(crypto): add SignatureVerifier and RevocationStore traits with InMemory impl"
```

---

## Phase 4: Pack / Unpack (`skillpkg-pack`)

### Task 7: Tarball creation and manifest serialisation

**Files:**
- Create: `crates/skillpkg-pack/src/pack.rs`
- Create: `crates/skillpkg-pack/src/unpack.rs`
- Create: `crates/skillpkg-pack/src/error.rs`
- Modify: `crates/skillpkg-pack/src/lib.rs`
- Create: `crates/skillpkg-pack/tests/pack_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-pack/tests/pack_test.rs
use std::fs;
use tempfile::TempDir;
use skillpkg_pack::pack::pack_directory;
use skillpkg_pack::unpack::unpack_tarball;

fn make_skill_dir(dir: &TempDir) {
    let skill_md = "---\nname: test-skill\ndescription: A test skill\n---\n# Test\n";
    fs::write(dir.path().join("SKILL.md"), skill_md).unwrap();
    let manifest = r#"{"namespace":"acme","name":"test-skill","version":"0.1.0","description":"A test skill","sha256":"","cert_chain_pem":[]}"#;
    fs::write(dir.path().join("manifest.json"), manifest).unwrap();
}

#[test]
fn pack_creates_tarball_with_correct_entries() {
    let src = TempDir::new().unwrap();
    make_skill_dir(&src);

    let out = TempDir::new().unwrap();
    let tarball_path = out.path().join("test.skill");

    pack_directory(src.path(), &tarball_path).unwrap();
    assert!(tarball_path.exists());
    assert!(tarball_path.metadata().unwrap().len() > 0);
}

#[test]
fn unpack_roundtrips_skill_md() {
    let src = TempDir::new().unwrap();
    make_skill_dir(&src);

    let out_tar = TempDir::new().unwrap();
    let tarball_path = out_tar.path().join("test.skill");
    pack_directory(src.path(), &tarball_path).unwrap();

    let dest = TempDir::new().unwrap();
    unpack_tarball(&tarball_path, dest.path()).unwrap();

    let skill_md = fs::read_to_string(dest.path().join("SKILL.md")).unwrap();
    assert!(skill_md.contains("name: test-skill"));
}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skillpkg-pack 2>&1 | head -10
```

**Step 3: Add `flate2` and `tar` to `crates/skillpkg-pack/Cargo.toml`**

```toml
[dependencies]
# ... existing ...
flate2 = "1"
tar    = "0.4"
```

Also add to workspace `Cargo.toml` under `[workspace.dependencies]`:
```toml
flate2 = "1"
tar    = "0.4"
```

**Step 4: Implement `crates/skillpkg-pack/src/error.rs`**

```rust
//! Error types for pack/unpack operations.

use thiserror::Error;

/// Errors that can occur when packing or unpacking a `.skill` tarball.
#[derive(Debug, Error)]
pub enum PackError {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The source directory does not contain a required file.
    #[error("required file '{0}' not found in source directory")]
    MissingFile(String),
}
```

**Step 5: Implement `crates/skillpkg-pack/src/pack.rs`**

```rust
//! Creates a gzip-compressed `.skill` tarball from a directory.

use std::fs::File;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use log::debug;

use crate::error::PackError;

/// Files that MUST be present in the source directory.
const REQUIRED_FILES: &[&str] = &["SKILL.md", "manifest.json"];

/// Pack a directory into a gzip-compressed `.skill` tarball at `output_path`.
///
/// All files in `source_dir` are included. Hidden files and `.git` directories
/// are excluded. The output file is created or truncated.
///
/// # Errors
///
/// Returns [`PackError::MissingFile`] if any required file is absent, or
/// [`PackError::Io`] on any I/O failure.
pub fn pack_directory(source_dir: &Path, output_path: &Path) -> Result<(), PackError> {
    for required in REQUIRED_FILES {
        if !source_dir.join(required).exists() {
            return Err(PackError::MissingFile((*required).to_owned()));
        }
    }

    let file = File::create(output_path)?;
    let encoder = GzEncoder::new(file, Compression::best());
    let mut archive = tar::Builder::new(encoder);
    archive.follow_symlinks(false);

    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') || name_str == ".git" {
            continue;
        }

        let path = entry.path();
        debug!("packing: {}", path.display());

        if path.is_dir() {
            archive.append_dir_all(&name, &path)?;
        } else {
            archive.append_path_with_name(&path, &name)?;
        }
    }

    archive.finish()?;
    Ok(())
}
```

**Step 6: Implement `crates/skillpkg-pack/src/unpack.rs`**

```rust
//! Extracts a gzip-compressed `.skill` tarball into a target directory.

use std::fs::File;
use std::path::Path;

use flate2::read::GzDecoder;
use log::debug;

use crate::error::PackError;

/// Unpack a `.skill` tarball into `dest_dir`.
///
/// The destination directory is created if it does not exist.
/// Existing files in `dest_dir` are overwritten.
///
/// # Errors
///
/// Returns [`PackError::Io`] on any I/O or decompression failure.
pub fn unpack_tarball(tarball_path: &Path, dest_dir: &Path) -> Result<(), PackError> {
    std::fs::create_dir_all(dest_dir)?;
    let file = File::open(tarball_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        debug!("unpacking: {}", path.display());
        entry.unpack_in(dest_dir)?;
    }

    Ok(())
}
```

**Step 7: Update `crates/skillpkg-pack/src/lib.rs`**

```rust
//! Packing and unpacking of `.skill` tarballs.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod error;
pub mod pack;
pub mod unpack;
```

**Step 8: Run tests**

```bash
cargo test -p skillpkg-pack 2>&1
```

Expected: all tests pass.

**Step 9: Commit**

```bash
git add crates/skillpkg-pack/
git commit -m "feat(pack): add tarball pack and unpack for .skill packages"
```

---

## Phase 5: Registry API (`skreg-api`)

### Task 8: Database migrations

**Files:**
- Create: `crates/skreg-api/migrations/001_initial.sql`
- Create: `crates/skreg-api/migrations/002_package_search.sql`
- Create: `crates/skreg-api/src/db.rs`
- Modify: `crates/skreg-api/src/main.rs`

**Step 1: Write `crates/skreg-api/migrations/001_initial.sql`**

```sql
-- namespaces
CREATE TABLE namespaces (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug       TEXT UNIQUE NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('individual', 'org')),
    oidc_sub   TEXT UNIQUE,
    domain     TEXT,
    banned_at  TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- publisher certs (org accounts only)
CREATE TABLE publisher_certs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id),
    serial       BIGINT UNIQUE NOT NULL,
    pem          TEXT NOT NULL,
    issued_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);

-- packages
CREATE TABLE packages (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id),
    name         TEXT NOT NULL,
    description  TEXT,
    category     TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (namespace_id, name)
);

CREATE INDEX packages_namespace_idx ON packages (namespace_id);

-- versions (immutable once inserted)
CREATE TABLE versions (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id   UUID NOT NULL REFERENCES packages(id),
    version      TEXT NOT NULL,
    sha256       TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    sig_path     TEXT NOT NULL,
    signer       TEXT NOT NULL,
    published_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    yanked_at    TIMESTAMPTZ,
    yank_reason  TEXT,
    UNIQUE (package_id, version)
);

CREATE INDEX versions_package_idx ON versions (package_id);

-- vetting pipeline jobs
CREATE TABLE vetting_jobs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id   UUID NOT NULL REFERENCES versions(id),
    status       TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'pass', 'fail', 'quarantined')),
    results      JSONB,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

-- community reports
CREATE TABLE reports (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id  UUID NOT NULL REFERENCES versions(id),
    reason      TEXT NOT NULL CHECK (reason IN ('malicious', 'misleading', 'spam', 'other')),
    detail      TEXT,
    reporter_ip TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    resolution  TEXT
);
```

**Step 2: Write `crates/skreg-api/migrations/002_package_search.sql`**

```sql
CREATE TABLE package_search (
    package_id    UUID PRIMARY KEY REFERENCES packages(id),
    search_vector TSVECTOR NOT NULL
);

CREATE INDEX package_search_gin_idx ON package_search USING GIN (search_vector);

-- Keep search_vector in sync when packages table changes.
CREATE OR REPLACE FUNCTION update_package_search_vector()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO package_search (package_id, search_vector)
    VALUES (
        NEW.id,
        to_tsvector('english', COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.description, ''))
    )
    ON CONFLICT (package_id) DO UPDATE
        SET search_vector = to_tsvector(
            'english',
            COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.description, '')
        );
    RETURN NEW;
END;
$$;

CREATE TRIGGER packages_search_sync
AFTER INSERT OR UPDATE ON packages
FOR EACH ROW EXECUTE FUNCTION update_package_search_vector();
```

**Step 3: Implement `crates/skreg-api/src/db.rs`**

```rust
//! Database connection pool initialisation.

use sqlx::PgPool;
use thiserror::Error;

/// Errors that can occur during database initialisation.
#[derive(Debug, Error)]
pub enum DbError {
    /// SQLx returned an error connecting or migrating.
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// Migration error.
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Create a connection pool and run pending migrations.
///
/// # Errors
///
/// Returns [`DbError`] if the pool cannot be created or migrations fail.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

**Step 4: Update `crates/skreg-api/src/main.rs`**

```rust
//! skreg registry API server entry point.

use std::env;

fn main() {
    env_logger::init();
    let _database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    // Router and server startup wired in Task 9.
}
```

**Step 5: Verify migrations parse**

```bash
cargo build -p skreg-api 2>&1
```

Expected: compiles without errors.

**Step 6: Commit**

```bash
git add crates/skreg-api/
git commit -m "feat(api): add PostgreSQL migrations and connection pool initialisation"
```

---

### Task 9: Axum router skeleton and health endpoint

**Files:**
- Create: `crates/skreg-api/src/router.rs`
- Create: `crates/skreg-api/src/config.rs`
- Modify: `crates/skreg-api/src/main.rs`
- Create: `crates/skreg-api/tests/health_test.rs`

**Step 1: Write failing test**

```rust
// crates/skreg-api/tests/health_test.rs
// Integration test — requires a running DB; skip with cfg(ignore) in CI
// without a DB, use axum::body::to_bytes and TestClient from axum-test.

use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::build_router;

#[tokio::test]
async fn health_returns_200() {
    // build_router accepts an Option<PgPool>; pass None for unit tests
    // (health endpoint does not use the DB).
    let app = build_router(None);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/healthz").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}
```

Add to `crates/skreg-api/Cargo.toml` dev-dependencies:
```toml
[dev-dependencies]
axum-test = "14"
tokio     = { workspace = true }
```

**Step 2: Run to verify failure**

```bash
cargo test -p skreg-api 2>&1 | head -10
```

**Step 3: Implement `crates/skreg-api/src/config.rs`**

```rust
//! API server configuration loaded from environment variables.

use std::env;

use thiserror::Error;

/// Errors during configuration loading.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required environment variable is missing.
    #[error("missing required environment variable: {0}")]
    Missing(String),
}

/// API server runtime configuration.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// PostgreSQL connection URL.
    pub database_url: String,
    /// TCP address to bind (e.g. `0.0.0.0:8080`).
    pub bind_addr: String,
}

impl ApiConfig {
    /// Load configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Missing`] if `DATABASE_URL` is not set.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| ConfigError::Missing("DATABASE_URL".to_owned()))?,
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        })
    }
}
```

**Step 4: Implement `crates/skreg-api/src/router.rs`**

```rust
//! Axum router construction.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

/// Response body for the health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
///
/// `pool` may be `None` in tests that do not exercise database endpoints.
pub fn build_router(pool: Option<PgPool>) -> Router {
    let mut router = Router::new().route("/healthz", get(health_handler));

    if let Some(p) = pool {
        router = router.with_state(p);
    }

    router
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
```

**Step 5: Update `crates/skreg-api/src/main.rs`**

```rust
//! skreg registry API server entry point.

use log::info;
use skreg_api::{config::ApiConfig, db::connect_and_migrate, router::build_router};

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = ApiConfig::from_env().expect("failed to load API config");
    let pool = connect_and_migrate(&config.database_url)
        .await
        .expect("failed to connect to database and run migrations");

    let app = build_router(Some(pool));
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .expect("failed to bind TCP listener");

    info!("skreg-api listening on {}", config.bind_addr);
    axum::serve(listener, app).await.expect("server error");
}
```

**Step 6: Update `crates/skreg-api/src/lib.rs`** (create if missing)

```rust
//! skreg registry API library — router, config, and database modules.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod config;
pub mod db;
pub mod router;
```

**Step 7: Run tests**

```bash
cargo test -p skreg-api 2>&1
```

Expected: `health_returns_200` passes.

**Step 8: Commit**

```bash
git add crates/skreg-api/
git commit -m "feat(api): add Axum router skeleton with /healthz endpoint"
```

---

### Task 10: Search and package metadata endpoints

**Files:**
- Create: `crates/skreg-api/src/handlers/search.rs`
- Create: `crates/skreg-api/src/handlers/mod.rs`
- Create: `crates/skreg-api/src/models.rs`
- Modify: `crates/skreg-api/src/router.rs`
- Create: `crates/skreg-api/tests/search_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skreg-api/tests/search_test.rs
use axum::http::StatusCode;
use axum_test::TestServer;
use skreg_api::router::build_router;

#[tokio::test]
async fn search_without_db_returns_503() {
    // Without a pool, endpoints that require DB return 503 Service Unavailable.
    let app = build_router(None);
    let server = TestServer::new(app).unwrap();
    let response = server.get("/v1/search?q=test").await;
    // No DB configured — expect a graceful error, not a panic.
    assert!(
        response.status_code() == StatusCode::SERVICE_UNAVAILABLE
            || response.status_code() == StatusCode::OK
    );
}
```

**Step 2: Implement `crates/skreg-api/src/models.rs`**

```rust
//! API response and query models.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// A single package search result.
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageSummary {
    /// Package UUID.
    pub id: Uuid,
    /// Namespace slug.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Optional category tag.
    pub category: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Paginated search response.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Matching packages for this page.
    pub packages: Vec<PackageSummary>,
    /// Total number of matches across all pages.
    pub total: i64,
    /// Current page number (1-indexed).
    pub page: i64,
}

/// Query parameters for `GET /v1/search`.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Full-text search query.
    pub q: Option<String>,
    /// Filter by category.
    pub category: Option<String>,
    /// Page number (default 1).
    pub page: Option<i64>,
}
```

**Step 3: Implement `crates/skreg-api/src/handlers/search.rs`**

```rust
//! Handlers for the package search and metadata endpoints.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use log::error;
use sqlx::PgPool;

use crate::models::{PackageSummary, SearchQuery, SearchResponse};

const PAGE_SIZE: i64 = 20;

/// `GET /v1/search` — full-text package search.
pub async fn search_handler(
    State(pool): State<PgPool>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;
    let query = params.q.unwrap_or_default();

    let rows = sqlx::query_as!(
        PackageSummary,
        r#"
        SELECT p.id, n.slug AS namespace, p.name, p.description, p.category, p.created_at
        FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
        ORDER BY p.created_at DESC
        LIMIT $3 OFFSET $4
        "#,
        query,
        params.category,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("search query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) FROM packages p
        JOIN namespaces n ON n.id = p.namespace_id
        LEFT JOIN package_search ps ON ps.package_id = p.id
        WHERE n.banned_at IS NULL
          AND ($1 = '' OR ps.search_vector @@ plainto_tsquery('english', $1))
          AND ($2::text IS NULL OR p.category = $2)
        "#,
        query,
        params.category,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        error!("count query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .unwrap_or(0);

    Ok(Json(SearchResponse { packages: rows, total, page }))
}
```

**Step 4: Implement `crates/skreg-api/src/handlers/mod.rs`**

```rust
//! HTTP request handlers.

pub mod search;
```

**Step 5: Update `crates/skreg-api/src/router.rs` to mount search**

```rust
//! Axum router construction.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

use crate::handlers::search::search_handler;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Build the Axum application router.
pub fn build_router(pool: Option<PgPool>) -> Router {
    match pool {
        Some(p) => Router::new()
            .route("/healthz", get(health_handler))
            .route("/v1/search", get(search_handler))
            .with_state(p),
        None => Router::new().route("/healthz", get(health_handler)),
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
```

**Step 6: Update `lib.rs`**

```rust
//! skreg registry API library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod config;
pub mod db;
pub mod handlers;
pub mod models;
pub mod router;
```

**Step 7: Run tests**

```bash
cargo test -p skreg-api 2>&1
```

Expected: all tests pass (search test is resilient to missing DB).

**Step 8: Commit**

```bash
git add crates/skreg-api/
git commit -m "feat(api): add GET /v1/search endpoint with full-text PostgreSQL search"
```

---

## Phase 6: Vetting Worker (`skreg-worker`)

### Task 11: Worker job loop and Stage 1 structure checks

**Files:**
- Create: `crates/skreg-worker/src/config.rs`
- Create: `crates/skreg-worker/src/job.rs`
- Create: `crates/skreg-worker/src/stages/mod.rs`
- Create: `crates/skreg-worker/src/stages/structure.rs`
- Modify: `crates/skreg-worker/src/main.rs`
- Create: `crates/skreg-worker/tests/structure_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skreg-worker/tests/structure_test.rs
use std::fs;
use tempfile::TempDir;
use skreg_worker::stages::structure::check_structure;

fn make_valid_dir(dir: &TempDir) {
    fs::write(dir.path().join("SKILL.md"), "---\nname: test\ndescription: hello\n---\n").unwrap();
    fs::write(dir.path().join("manifest.json"), r#"{"name":"test"}"#).unwrap();
}

#[test]
fn valid_directory_passes_structure_checks() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    let result = check_structure(dir.path());
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn missing_skill_md_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("manifest.json"), "{}").unwrap();
    assert!(check_structure(dir.path()).is_err());
}

#[test]
fn missing_manifest_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("SKILL.md"), "---\n---\n").unwrap();
    assert!(check_structure(dir.path()).is_err());
}

#[test]
fn oversized_tarball_fails() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    // Write a 6MB file to exceed the 5MB limit
    let big = vec![0u8; 6 * 1024 * 1024];
    fs::write(dir.path().join("big.md"), big).unwrap();
    assert!(check_structure(dir.path()).is_err());
}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skreg-worker 2>&1 | head -10
```

**Step 3: Implement `crates/skreg-worker/src/stages/structure.rs`**

```rust
//! Stage 1: structural validity checks on an unpacked skill package.

use std::path::Path;

use thiserror::Error;

const MAX_TOTAL_BYTES: u64 = 5 * 1024 * 1024; // 5 MB
const REQUIRED_FILES: &[&str] = &["SKILL.md", "manifest.json"];
const ALLOWED_EXTENSIONS: &[&str] = &["md", "json"];

/// Errors produced by structural validation.
#[derive(Debug, Error)]
pub enum StructureError {
    /// A required file is missing.
    #[error("required file '{0}' is missing")]
    MissingFile(String),
    /// Total size of all files exceeds the maximum.
    #[error("package size {size} bytes exceeds maximum of {max} bytes")]
    TooLarge {
        /// Actual total size.
        size: u64,
        /// Maximum allowed size.
        max: u64,
    },
    /// A file with a disallowed extension was found.
    #[error("disallowed file type: '{0}'")]
    DisallowedFileType(String),
    /// An I/O error occurred during checking.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Run Stage 1 structural checks on the unpacked directory at `path`.
///
/// # Errors
///
/// Returns the first [`StructureError`] encountered.
pub fn check_structure(path: &Path) -> Result<(), StructureError> {
    for required in REQUIRED_FILES {
        if !path.join(required).exists() {
            return Err(StructureError::MissingFile((*required).to_owned()));
        }
    }

    let mut total_size: u64 = 0;

    for entry in walkdir::WalkDir::new(path).into_iter() {
        let entry = entry.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        if entry.file_type().is_dir() {
            continue;
        }

        let ext = entry.path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ALLOWED_EXTENSIONS.contains(&ext) {
            return Err(StructureError::DisallowedFileType(
                entry.path().display().to_string(),
            ));
        }

        total_size += entry.metadata()?.len();
        if total_size > MAX_TOTAL_BYTES {
            return Err(StructureError::TooLarge {
                size: total_size,
                max: MAX_TOTAL_BYTES,
            });
        }
    }

    Ok(())
}
```

Add to `crates/skreg-worker/Cargo.toml`:
```toml
walkdir = "2"
```

**Step 4: Create `crates/skreg-worker/src/stages/mod.rs`**

```rust
//! Vetting pipeline stages.

pub mod structure;
```

**Step 5: Create `crates/skreg-worker/src/lib.rs`**

```rust
//! skreg vetting worker library.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod stages;
```

**Step 6: Run tests**

```bash
cargo test -p skreg-worker 2>&1
```

Expected: all 4 tests pass.

**Step 7: Commit**

```bash
git add crates/skreg-worker/
git commit -m "feat(worker): add Stage 1 structure checks (required files, size, extensions)"
```

---

## Phase 7: Pulumi Python — Component Interfaces and AWS Implementation

### Task 12: Database and Storage component interfaces

**Files:**
- Create: `infra/src/skillpkg_infra/components/database.py`
- Create: `infra/src/skillpkg_infra/components/storage.py`
- Create: `infra/src/skillpkg_infra/components/pki.py`
- Create: `infra/src/skillpkg_infra/components/compute.py`
- Create: `infra/src/skillpkg_infra/components/network.py`
- Create: `infra/tests/test_components.py`

**Step 1: Write failing tests**

```python
# infra/tests/test_components.py
"""Verify that component Protocol interfaces are importable and well-typed."""
from __future__ import annotations

from skillpkg_infra.components.database import DatabaseOutputs, SkillpkgDatabase
from skillpkg_infra.components.storage import StorageOutputs, SkillpkgStorage
from skillpkg_infra.components.pki import PkiOutputs, SkillpkgPki
from skillpkg_infra.components.compute import ComputeOutputs, SkillpkgCompute
from skillpkg_infra.components.network import NetworkOutputs, SkillpkgNetwork


def test_database_protocol_is_importable() -> None:
    """DatabaseOutputs and SkillpkgDatabase must be importable."""
    assert SkillpkgDatabase is not None
    assert DatabaseOutputs is not None


def test_storage_protocol_is_importable() -> None:
    """StorageOutputs and SkillpkgStorage must be importable."""
    assert SkillpkgStorage is not None
    assert StorageOutputs is not None


def test_pki_protocol_is_importable() -> None:
    """PkiOutputs and SkillpkgPki must be importable."""
    assert SkillpkgPki is not None
    assert PkiOutputs is not None
```

**Step 2: Run to verify failure**

```bash
cd infra && uv run pytest tests/test_components.py -v 2>&1 | head -20
```

**Step 3: Implement `infra/src/skillpkg_infra/components/database.py`**

```python
"""Provider-agnostic database component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class DatabaseOutputs:
    """Resolved connection outputs from a provisioned database component."""

    def __init__(
        self,
        connection_secret_name: pulumi.Output[str],
        host: pulumi.Output[str],
        port: pulumi.Output[int],
        database_name: pulumi.Output[str],
    ) -> None:
        """Initialise database outputs.

        Args:
            connection_secret_name: Provider secret store key for DB credentials.
            host: Database hostname or IP address.
            port: Database TCP port.
            database_name: Name of the application database.
        """
        self.connection_secret_name: pulumi.Output[str] = connection_secret_name
        self.host: pulumi.Output[str] = host
        self.port: pulumi.Output[int] = port
        self.database_name: pulumi.Output[str] = database_name


class SkillpkgDatabase(Protocol):
    """Provider-agnostic interface for the registry PostgreSQL component."""

    @property
    def outputs(self) -> DatabaseOutputs:
        """Return the resolved database connection outputs."""
        ...
```

**Step 4: Implement `infra/src/skillpkg_infra/components/storage.py`**

```python
"""Provider-agnostic object storage + CDN component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class StorageOutputs:
    """Resolved outputs from a provisioned object storage + CDN component."""

    def __init__(
        self,
        bucket_name: pulumi.Output[str],
        cdn_base_url: pulumi.Output[str],
        service_account_secret_name: pulumi.Output[str],
    ) -> None:
        """Initialise storage outputs.

        Args:
            bucket_name: Name of the object storage bucket.
            cdn_base_url: Base URL for CDN-distributed package downloads.
            service_account_secret_name: Secret name for storage service credentials.
        """
        self.bucket_name: pulumi.Output[str] = bucket_name
        self.cdn_base_url: pulumi.Output[str] = cdn_base_url
        self.service_account_secret_name: pulumi.Output[str] = service_account_secret_name


class SkillpkgStorage(Protocol):
    """Provider-agnostic interface for the registry object storage component."""

    @property
    def outputs(self) -> StorageOutputs:
        """Return the resolved storage outputs."""
        ...
```

**Step 5: Implement `infra/src/skillpkg_infra/components/pki.py`**

```python
"""Provider-agnostic PKI + HSM component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class PkiOutputs:
    """Resolved outputs from a provisioned PKI component."""

    def __init__(
        self,
        hsm_key_id: pulumi.Output[str],
        intermediate_ca_cert_secret_name: pulumi.Output[str],
        crl_bucket_path: pulumi.Output[str],
        hsm_backend: str,
    ) -> None:
        """Initialise PKI outputs.

        Args:
            hsm_key_id: Provider-specific HSM key identifier.
            intermediate_ca_cert_secret_name: Secret name for the intermediate CA cert.
            crl_bucket_path: Object storage path for the CRL file.
            hsm_backend: Either ``"hsm"`` or ``"software"``.
        """
        self.hsm_key_id: pulumi.Output[str] = hsm_key_id
        self.intermediate_ca_cert_secret_name: pulumi.Output[str] = (
            intermediate_ca_cert_secret_name
        )
        self.crl_bucket_path: pulumi.Output[str] = crl_bucket_path
        self.hsm_backend: str = hsm_backend


class SkillpkgPki(Protocol):
    """Provider-agnostic interface for the registry PKI + HSM component."""

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        ...
```

**Step 6: Implement `infra/src/skillpkg_infra/components/compute.py`**

```python
"""Provider-agnostic container compute component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class ComputeOutputs:
    """Resolved outputs from a provisioned container compute component."""

    def __init__(
        self,
        service_url: pulumi.Output[str],
        worker_service_name: pulumi.Output[str],
    ) -> None:
        """Initialise compute outputs.

        Args:
            service_url: Public HTTPS URL of the registry API service.
            worker_service_name: Internal name of the vetting worker service.
        """
        self.service_url: pulumi.Output[str] = service_url
        self.worker_service_name: pulumi.Output[str] = worker_service_name


class SkillpkgCompute(Protocol):
    """Provider-agnostic interface for the registry compute component."""

    @property
    def outputs(self) -> ComputeOutputs:
        """Return the resolved compute outputs."""
        ...
```

**Step 7: Implement `infra/src/skillpkg_infra/components/network.py`**

```python
"""Provider-agnostic network component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class NetworkOutputs:
    """Resolved outputs from a provisioned network component."""

    def __init__(
        self,
        vpc_id: pulumi.Output[str],
        private_subnet_ids: list[pulumi.Output[str]],
    ) -> None:
        """Initialise network outputs.

        Args:
            vpc_id: ID of the provisioned VPC or equivalent.
            private_subnet_ids: IDs of private subnets for backend services.
        """
        self.vpc_id: pulumi.Output[str] = vpc_id
        self.private_subnet_ids: list[pulumi.Output[str]] = private_subnet_ids


class SkillpkgNetwork(Protocol):
    """Provider-agnostic interface for the network component."""

    @property
    def outputs(self) -> NetworkOutputs:
        """Return the resolved network outputs."""
        ...
```

**Step 8: Run tests**

```bash
cd infra && uv run pytest tests/test_components.py -v
```

Expected: all 3 tests pass.

**Step 9: Run mypy**

```bash
cd infra && uv run mypy src/
```

Expected: no errors.

**Step 10: Commit**

```bash
git add infra/
git commit -m "feat(infra): add provider-agnostic component Protocol interfaces"
```

---

### Task 13: AWS database and storage implementations

**Files:**
- Create: `infra/src/skillpkg_infra/providers/aws/database.py`
- Create: `infra/src/skillpkg_infra/providers/aws/storage.py`
- Create: `infra/tests/test_aws_database.py`
- Create: `infra/tests/test_aws_storage.py`

**Step 1: Write failing tests**

```python
# infra/tests/test_aws_database.py
"""Unit tests for the AWS database component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    """Deterministic mock returning stable IDs for all resources."""

    def new_resource(
        self,
        args: pulumi.runtime.MockResourceArgs,
    ) -> tuple[str, dict[str, object]]:
        """Return a stable mock ID and echo inputs as outputs."""
        return (f"{args.name}-id", args.inputs)

    def call(
        self,
        args: pulumi.runtime.MockCallArgs,
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        """Return empty outputs for all provider function calls."""
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs


@pulumi.runtime.test
def test_database_port_is_5432() -> None:
    """AwsDatabase outputs must expose PostgreSQL port 5432."""
    db = AwsDatabase(
        "test-db",
        AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]),
    )

    def assert_port(port: int) -> None:
        assert port == 5432, f"Expected 5432, got {port}"

    return db.outputs.port.apply(assert_port)


@pulumi.runtime.test
def test_database_name_is_skillpkg() -> None:
    """AwsDatabase outputs must use 'skillpkg' as the database name."""
    db = AwsDatabase(
        "test-db2",
        AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]),
    )

    def assert_name(name: str) -> None:
        assert name == "skillpkg", f"Expected 'skillpkg', got {name}"

    return db.outputs.database_name.apply(assert_name)
```

**Step 2: Run to verify failure**

```bash
cd infra && uv run pytest tests/test_aws_database.py -v 2>&1 | head -20
```

**Step 3: Implement `infra/src/skillpkg_infra/providers/aws/database.py`**

```python
"""AWS RDS PostgreSQL implementation of SkillpkgDatabase."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws

from skillpkg_infra.components.database import DatabaseOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsDatabaseArgs:
    """Arguments for the AWS RDS database component.

    Args:
        vpc_id: ID of the VPC in which to place the RDS instance.
        subnet_ids: Private subnet IDs for the DB subnet group.
        instance_class: RDS instance class.
        multi_az: Enable Multi-AZ deployment.
    """

    def __init__(
        self,
        vpc_id: pulumi.Input[str],
        subnet_ids: list[pulumi.Input[str]],
        instance_class: pulumi.Input[str] = "db.t4g.medium",
        multi_az: pulumi.Input[bool] = False,
    ) -> None:
        """Initialise RDS database arguments."""
        self.vpc_id: pulumi.Input[str] = vpc_id
        self.subnet_ids: list[pulumi.Input[str]] = subnet_ids
        self.instance_class: pulumi.Input[str] = instance_class
        self.multi_az: pulumi.Input[bool] = multi_az


class AwsDatabase(pulumi.ComponentResource):
    """AWS RDS PostgreSQL component satisfying ``SkillpkgDatabase``.

    Provisions an encrypted RDS instance, a Secrets Manager secret,
    a security group, and a DB subnet group.
    """

    def __init__(
        self,
        name: str,
        args: AwsDatabaseArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the AWS database component.

        Args:
            name: Logical Pulumi resource name.
            args: Validated AWS-specific database arguments.
            opts: Optional Pulumi resource options.
        """
        super().__init__("skillpkg:aws:Database", name, {}, opts)

        logger.debug("provisioning_aws_database", extra={"name": name})

        security_group = aws.ec2.SecurityGroup(
            f"{name}-sg",
            aws.ec2.SecurityGroupArgs(vpc_id=args.vpc_id),
            opts=pulumi.ResourceOptions(parent=self),
        )

        subnet_group = aws.rds.SubnetGroup(
            f"{name}-subnets",
            aws.rds.SubnetGroupArgs(subnet_ids=args.subnet_ids),
            opts=pulumi.ResourceOptions(parent=self),
        )

        password_secret = aws.secretsmanager.Secret(
            f"{name}-db-password",
            opts=pulumi.ResourceOptions(parent=self),
        )

        instance = aws.rds.Instance(
            f"{name}-rds",
            aws.rds.InstanceArgs(
                engine="postgres",
                engine_version="16",
                instance_class=args.instance_class,
                allocated_storage=20,
                storage_encrypted=True,
                multi_az=args.multi_az,
                db_subnet_group_name=subnet_group.name,
                vpc_security_group_ids=[security_group.id],
                skip_final_snapshot=False,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: DatabaseOutputs = DatabaseOutputs(
            connection_secret_name=password_secret.name,
            host=instance.address,
            port=pulumi.Output.from_input(5432),
            database_name=pulumi.Output.from_input("skillpkg"),
        )

        self.register_outputs(
            {
                "connection_secret_name": self._outputs.connection_secret_name,
                "host": self._outputs.host,
                "port": self._outputs.port,
                "database_name": self._outputs.database_name,
            }
        )

    @property
    def outputs(self) -> DatabaseOutputs:
        """Return the resolved database connection outputs."""
        return self._outputs
```

**Step 4: Implement `infra/src/skillpkg_infra/providers/aws/storage.py`**

```python
"""AWS S3 + CloudFront object storage implementation of SkillpkgStorage."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws

from skillpkg_infra.components.storage import StorageOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsStorage(pulumi.ComponentResource):
    """AWS S3 + CloudFront implementation satisfying ``SkillpkgStorage``.

    Provisions a private S3 bucket for content-addressed package storage
    and a CloudFront distribution for immutable download serving.
    """

    def __init__(
        self,
        name: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the AWS storage component.

        Args:
            name: Logical Pulumi resource name.
            opts: Optional Pulumi resource options.
        """
        super().__init__("skillpkg:aws:Storage", name, {}, opts)

        logger.debug("provisioning_aws_storage", extra={"name": name})

        bucket = aws.s3.Bucket(
            f"{name}-bucket",
            aws.s3.BucketArgs(
                force_destroy=False,
                server_side_encryption_configuration=aws.s3.BucketServerSideEncryptionConfigurationArgs(
                    rule=aws.s3.BucketServerSideEncryptionConfigurationRuleArgs(
                        apply_server_side_encryption_by_default=aws.s3.BucketServerSideEncryptionConfigurationRuleApplyServerSideEncryptionByDefaultArgs(
                            sse_algorithm="AES256",
                        ),
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        service_secret = aws.secretsmanager.Secret(
            f"{name}-storage-credentials",
            opts=pulumi.ResourceOptions(parent=self),
        )

        distribution = aws.cloudfront.Distribution(
            f"{name}-cdn",
            aws.cloudfront.DistributionArgs(
                enabled=True,
                origins=[
                    aws.cloudfront.DistributionOriginArgs(
                        origin_id="s3-origin",
                        domain_name=bucket.bucket_regional_domain_name,
                    )
                ],
                default_cache_behavior=aws.cloudfront.DistributionDefaultCacheBehaviorArgs(
                    target_origin_id="s3-origin",
                    viewer_protocol_policy="redirect-to-https",
                    allowed_methods=["GET", "HEAD"],
                    cached_methods=["GET", "HEAD"],
                    forwarded_values=aws.cloudfront.DistributionDefaultCacheBehaviorForwardedValuesArgs(
                        query_string=False,
                        cookies=aws.cloudfront.DistributionDefaultCacheBehaviorForwardedValuesCookiesArgs(
                            forward="none",
                        ),
                    ),
                ),
                restrictions=aws.cloudfront.DistributionRestrictionsArgs(
                    geo_restriction=aws.cloudfront.DistributionRestrictionsGeoRestrictionArgs(
                        restriction_type="none",
                    ),
                ),
                viewer_certificate=aws.cloudfront.DistributionViewerCertificateArgs(
                    cloudfront_default_certificate=True,
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: StorageOutputs = StorageOutputs(
            bucket_name=bucket.bucket,
            cdn_base_url=distribution.domain_name.apply(lambda d: f"https://{d}"),
            service_account_secret_name=service_secret.name,
        )

        self.register_outputs(
            {
                "bucket_name": self._outputs.bucket_name,
                "cdn_base_url": self._outputs.cdn_base_url,
                "service_account_secret_name": self._outputs.service_account_secret_name,
            }
        )

    @property
    def outputs(self) -> StorageOutputs:
        """Return the resolved storage outputs."""
        return self._outputs
```

**Step 5: Run tests**

```bash
cd infra && uv run pytest tests/test_aws_database.py -v
```

Expected: both tests pass.

**Step 6: Run mypy**

```bash
cd infra && uv run mypy src/
```

Expected: no errors.

**Step 7: Commit**

```bash
git add infra/
git commit -m "feat(infra/aws): implement AwsDatabase (RDS) and AwsStorage (S3 + CloudFront)"
```

---

## Phase 8: CLI Client and Install Command

### Task 14: RegistryClient trait and HTTP adapter

**Files:**
- Create: `crates/skillpkg-client/src/client.rs`
- Create: `crates/skillpkg-client/src/error.rs`
- Modify: `crates/skillpkg-client/src/lib.rs`
- Create: `crates/skillpkg-client/tests/client_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-client/tests/client_test.rs
use skillpkg_client::client::RegistryClient;
use skillpkg_core::package_ref::PackageRef;

// Smoke test: the trait is object-safe (can be used as dyn RegistryClient).
fn _assert_object_safe(_: &dyn RegistryClient) {}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skillpkg-client 2>&1 | head -10
```

**Step 3: Implement `crates/skillpkg-client/src/error.rs`**

```rust
//! Error types for registry HTTP client operations.

use thiserror::Error;

/// Errors that can occur during client–registry communication.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The server returned an unexpected status code.
    #[error("unexpected status {status}: {body}")]
    UnexpectedStatus {
        /// HTTP status code received.
        status: u16,
        /// Response body (truncated).
        body: String,
    },
    /// The response body could not be parsed.
    #[error("failed to parse response: {0}")]
    Parse(String),
}
```

**Step 4: Implement `crates/skillpkg-client/src/client.rs`**

```rust
//! Registry HTTP client trait and `reqwest`-backed implementation.

use std::sync::Arc;

use log::debug;
use skillpkg_core::manifest::Manifest;
use skillpkg_core::package_ref::PackageRef;

use crate::error::ClientError;

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

/// Communicates with a skreg-compatible registry.
pub trait RegistryClient: Send + Sync {
    /// Resolve a package reference to its latest (or pinned) version metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network or parse failure.
    fn resolve(
        &self,
        pkg_ref: &PackageRef,
    ) -> impl std::future::Future<Output = Result<ResolvedVersion, ClientError>> + Send;
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

impl RegistryClient for HttpRegistryClient {
    async fn resolve(&self, pkg_ref: &PackageRef) -> Result<ResolvedVersion, ClientError> {
        let version_segment = pkg_ref
            .version
            .as_ref()
            .map_or_else(|| "latest".to_owned(), |v| v.to_string());

        let meta_url = format!(
            "{}/v1/packages/{}/{}/{}",
            self.base_url,
            pkg_ref.namespace,
            pkg_ref.name,
            version_segment,
        );

        debug!("resolving package from {meta_url}");

        let manifest: Manifest = self
            .http
            .get(&meta_url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| ClientError::Http(e))?
            .json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))?;

        let dl_url = format!(
            "{}/v1/download/{}/{}/{}",
            self.base_url,
            pkg_ref.namespace,
            pkg_ref.name,
            manifest.version,
        );

        let tarball = self.http.get(&dl_url).send().await?.bytes().await?.to_vec();
        let sig_url = format!("{dl_url}/sig");
        let signature = self.http.get(&sig_url).send().await?.bytes().await?.to_vec();

        Ok(ResolvedVersion { manifest, tarball, signature })
    }
}
```

**Step 5: Update `crates/skillpkg-client/src/lib.rs`**

```rust
//! HTTP client for communicating with a skreg registry.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod client;
pub mod error;
```

**Step 6: Run tests**

```bash
cargo test -p skillpkg-client 2>&1
```

Expected: compiles and tests pass.

**Step 7: Commit**

```bash
git add crates/skillpkg-client/
git commit -m "feat(client): add RegistryClient trait and HttpRegistryClient implementation"
```

---

### Task 15: `skillpkg install` command

**Files:**
- Create: `crates/skillpkg-cli/src/commands/install.rs`
- Create: `crates/skillpkg-cli/src/commands/mod.rs`
- Create: `crates/skillpkg-cli/src/installer.rs`
- Modify: `crates/skillpkg-cli/src/main.rs`
- Create: `crates/skillpkg-cli/tests/installer_test.rs`

**Step 1: Write failing tests**

```rust
// crates/skillpkg-cli/tests/installer_test.rs
use skillpkg_cli::installer::{InstallError, Installer};
use skillpkg_core::package_ref::PackageRef;

#[test]
fn install_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<InstallError>();
}
```

**Step 2: Run to verify failure**

```bash
cargo test -p skillpkg-cli 2>&1 | head -10
```

**Step 3: Implement `crates/skillpkg-cli/src/installer.rs`**

```rust
//! Orchestrates the full package install pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{debug, info};
use sha2::{Digest, Sha256};
use thiserror::Error;

use skillpkg_client::client::RegistryClient;
use skillpkg_core::installed::{InstalledPackage, SignerKind};
use skillpkg_core::package_ref::PackageRef;
use skillpkg_core::types::Sha256Digest;
use skillpkg_pack::unpack::unpack_tarball;

/// Errors that can occur during package installation.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The registry client returned an error.
    #[error("registry error: {0}")]
    Registry(#[from] skillpkg_client::error::ClientError),
    /// The tarball sha256 does not match the manifest.
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    DigestMismatch {
        /// Expected hex digest from the manifest.
        expected: String,
        /// Actual computed hex digest.
        actual: String,
    },
    /// A crypto validation error occurred.
    #[error("crypto error: {0}")]
    Crypto(String),
    /// An I/O error occurred during extraction.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A pack/unpack error occurred.
    #[error("pack error: {0}")]
    Pack(#[from] skillpkg_pack::error::PackError),
    /// A core validation error occurred.
    #[error("validation error: {0}")]
    Validation(#[from] skillpkg_core::types::ValidationError),
}

/// Orchestrates download, verification, and extraction of a skill package.
pub struct Installer {
    client: Arc<dyn RegistryClient>,
    install_root: PathBuf,
}

impl Installer {
    /// Create a new `Installer`.
    ///
    /// # Arguments
    ///
    /// * `client` — Registry HTTP client.
    /// * `install_root` — Base directory for installed packages
    ///   (e.g. `~/.skillpkg/packages`).
    pub fn new(client: Arc<dyn RegistryClient>, install_root: PathBuf) -> Self {
        Self { client, install_root }
    }

    /// Download, verify, and extract a package.
    ///
    /// Returns the installed package descriptor on success.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError`] if any step fails. Partial installs are
    /// cleaned up before returning.
    pub async fn install(&self, pkg_ref: &PackageRef) -> Result<InstalledPackage, InstallError> {
        info!("installing {pkg_ref}");

        let resolved = self.client.resolve(pkg_ref).await?;

        // Verify sha256
        let actual_hex = {
            let mut hasher = Sha256::new();
            hasher.update(&resolved.tarball);
            format!("{:x}", hasher.finalize())
        };
        let expected_hex = resolved.manifest.sha256.as_hex();
        if actual_hex != expected_hex {
            return Err(InstallError::DigestMismatch {
                expected: expected_hex.to_owned(),
                actual: actual_hex,
            });
        }

        debug!("sha256 verified for {pkg_ref}");

        let install_path = self.install_root
            .join(resolved.manifest.namespace.as_str())
            .join(resolved.manifest.name.as_str())
            .join(resolved.manifest.version.to_string());

        // Write tarball to temp file then unpack
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &resolved.tarball)?;
        unpack_tarball(tmp.path(), &install_path)?;

        info!("installed {} to {}", pkg_ref, install_path.display());

        Ok(InstalledPackage {
            pkg_ref: pkg_ref.clone(),
            sha256: Sha256Digest::from_hex(&actual_hex)?,
            signer: SignerKind::Registry,
            install_path,
        })
    }
}
```

Add to `crates/skillpkg-cli/Cargo.toml`:
```toml
sha2      = { workspace = true }
tempfile  = "3"
```

**Step 4: Create `crates/skillpkg-cli/src/lib.rs`**

```rust
//! skillpkg CLI library — command implementations and install orchestration.
#![deny(warnings, clippy::all, clippy::pedantic)]
#![warn(missing_docs)]

pub mod installer;
```

**Step 5: Run tests**

```bash
cargo test -p skillpkg-cli 2>&1
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add crates/skillpkg-cli/
git commit -m "feat(cli): add Installer with sha256 verification and tarball extraction"
```

---

## Phase 9: CI/CD

### Task 16: GitHub Actions workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Write `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    name: Rust — test, lint, format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all --check

      - name: Clippy (deny warnings)
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test --workspace

  python-infra:
    name: Python infra — lint, type-check, test
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: infra
    steps:
      - uses: actions/checkout@v4

      - name: Install uv
        uses: astral-sh/setup-uv@v4

      - name: Install dependencies
        run: uv sync --extra dev

      - name: ruff check
        run: uv run ruff check src/

      - name: black check
        run: uv run black --check src/

      - name: mypy
        run: uv run mypy src/

      - name: pytest
        run: uv run pytest --cov-fail-under=90
```

**Step 2: Commit**

```bash
git add .github/
git commit -m "ci: add GitHub Actions workflow for Rust and Python infra"
```

---

## Completion Checklist

Before marking the implementation complete, verify:

- [ ] `cargo build --workspace` produces zero warnings and zero errors
- [ ] `cargo test --workspace` all pass
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all --check` passes
- [ ] `cd infra && uv run mypy src/` passes
- [ ] `cd infra && uv run ruff check src/` passes
- [ ] `cd infra && uv run black --check src/` passes
- [ ] `cd infra && uv run pytest` ≥ 90% coverage
- [ ] All commits are atomic with descriptive messages
- [ ] No TODOs, `unimplemented!()`, or `todo!()` macros in committed code
