-- Migration: trust enforcement tables and columns

-- Namespace-level publisher key pinning (SPKI SHA-256 fingerprint hex)
ALTER TABLE namespaces
  ADD COLUMN pinned_publisher_key TEXT;

-- Nonce replay prevention for key rotation requests
CREATE TABLE rotation_nonces (
    nonce      TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);

-- Ensure publisher_certs has created_at (idempotent on fresh schemas)
ALTER TABLE publisher_certs
  ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Self-signed key revocation (registry admin use)
CREATE TABLE revoked_self_signed_keys (
    fingerprint TEXT PRIMARY KEY,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    reason      TEXT
);

-- Pending key rotations (email-confirmed, 30-minute window)
CREATE TABLE pending_rotations (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id   UUID NOT NULL REFERENCES namespaces(id),
    rotation_token JSONB NOT NULL,
    old_sig        TEXT NOT NULL,
    new_sig        TEXT NOT NULL,
    confirm_token  TEXT NOT NULL UNIQUE,
    expires_at     TIMESTAMPTZ NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- PKI audit log
CREATE TABLE pki_audit_log (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    namespace_id UUID REFERENCES namespaces(id),
    operation    TEXT NOT NULL,
    outcome      TEXT NOT NULL,
    detail       JSONB,
    client_ip    TEXT
);

-- Update signer check constraint to include publisher trust tiers
ALTER TABLE versions DROP CONSTRAINT IF EXISTS versions_signer_check;
ALTER TABLE versions
  ADD CONSTRAINT versions_signer_check
  CHECK (signer IN ('registry', 'self_signed', 'publisher'));
