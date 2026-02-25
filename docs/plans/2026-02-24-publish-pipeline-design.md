# skreg Publish Pipeline — Design Document

**Date:** 2026-02-24
**Status:** Approved

---

## Overview

End-to-end skill packaging and publishing: a `skillpkg pack` command that produces a `.skill`
tarball, a `skillpkg login` command backed by API key auth with email-OTP recovery, a
`skillpkg publish` command that uploads and polls vetting, and the server-side publish pipeline
(API endpoints, vetting worker stages 2–4, Registry CA signing, S3 storage).

OIDC (GitHub/Google) login is deferred to a future iteration.

---

## End-to-End Flow

```
skillpkg login dymocaptin
  → POST /v1/namespaces {slug, email}          # new namespace
  → POST /v1/auth/login {namespace, email}      # existing namespace: sends OTP via SES
  → POST /v1/auth/token {namespace, otp}        # existing namespace: exchange OTP for key
  → API key saved to ~/.skillpkg/config.toml

skillpkg pack
  → Validates SKILL.md (exists, valid YAML frontmatter: name + description required)
  → Validates manifest.json (exists, name matches frontmatter)
  → Computes sha256 over tarball bytes, injects into manifest.json
  → Writes {name}-{version}.skill

skillpkg publish
  → POST /v1/publish  (Authorization: Bearer <key>, multipart .skill tarball)
  → 202 Accepted + {job_id}
  → CLI polls GET /v1/jobs/{id} every 2s
  → Worker: Stage 1 → Stage 2 → Stage 3 → Stage 4 (sign)
  → .sig written to S3, version marked published
  → CLI prints: ✓ Published dymocaptin/color-analysis@1.0.0
```

---

## Components

### CLI (`skillpkg-cli`)

| Command | Description |
|---------|-------------|
| `skillpkg pack` | Pack cwd into `{name}-{version}.skill` |
| `skillpkg login <namespace>` | Register namespace or re-auth via email OTP |
| `skillpkg publish` | Pack + upload + poll vetting result |

### API (`skreg-api`) — new endpoints

| Endpoint | Description |
|----------|-------------|
| `POST /v1/namespaces` | Create namespace, issue API key (returned once) |
| `POST /v1/auth/login` | Send OTP to registered email |
| `POST /v1/auth/token` | Exchange OTP for new API key |
| `POST /v1/publish` | Accept `.skill` tarball, validate, upload to S3, enqueue vetting |
| `GET /v1/jobs/{id}` | Return vetting job status |

### Worker (`skreg-worker`) — new vetting stages

| Stage | Checks |
|-------|--------|
| Stage 1 | Structure (already implemented) |
| Stage 2 | Content: description ≥ 20 chars, no hardcoded secrets (regex), `references/` contains `.md` only |
| Stage 3 | Safety: name not within Levenshtein-2 of existing packages, not a re-upload of yanked package |
| Stage 4 | Sign: load RSA-4096 CA key from Secrets Manager, sign sha256, write `.sig` to S3, mark published |

---

## Database Migrations

```sql
CREATE TABLE api_keys (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID REFERENCES namespaces(id) NOT NULL,
    key_hash     TEXT NOT NULL,
    email        TEXT NOT NULL,
    created_at   TIMESTAMPTZ DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE TABLE otps (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID REFERENCES namespaces(id) NOT NULL,
    code_hash    TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    used_at      TIMESTAMPTZ
);
```

---

## Data Flow & Error Handling

### Pack
- Fails with clear error if `SKILL.md` or `manifest.json` is missing or malformed
- Sha256 computed over tarball bytes, written into `manifest.json` before archiving

### Publish
| Condition | Response |
|-----------|----------|
| Missing/invalid API key | 401 |
| Manifest sha256 mismatch | 422 |
| Namespace mismatch (key ≠ manifest namespace) | 403 |
| Version already exists | 409 |
| S3 upload failure | 503 (not indexed) |
| Vetting failure | Job status `fail` + reason; CLI exits non-zero |
| Secrets Manager unavailable (Stage 4) | Retry ×3 then `fail` |

### Login
- Namespace slug validated: `^[a-z0-9-]{3,32}$`; 409 if already taken
- OTP: 6-digit code, SHA-256 hashed in DB, 10-minute TTL, single-use
- SES failure → 503 "could not send verification email"

---

## Testing Strategy

| Layer | Approach |
|-------|----------|
| CLI | Unit tests for pack/manifest validation; integration tests mock API with `wiremock` |
| API | Handler unit tests (existing pattern); S3 calls mocked with `aws-sdk` test doubles |
| Worker | Stage 1 pattern extended for Stages 2–3; Stage 4 uses a software RSA key fixture |

---

## Deferred

- OIDC login (GitHub / Google)
- ClamAV antivirus scan (Stage 3)
- `skillpkg yank`, `skillpkg verify`, `skillpkg audit`
- Web UI
