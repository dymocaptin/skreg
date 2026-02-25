-- api_keys: one row per issued key; key_hash is SHA-256(plaintext_key) hex-encoded
CREATE TABLE api_keys (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id),
    key_hash     TEXT NOT NULL,
    email        TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX api_keys_namespace_idx ON api_keys (namespace_id);
CREATE INDEX api_keys_hash_idx      ON api_keys (key_hash);

-- otps: single-use 6-digit codes for re-authentication
CREATE TABLE otps (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id),
    code_hash    TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    used_at      TIMESTAMPTZ
);

-- sig_path is empty until Stage 4 signing completes
ALTER TABLE versions ALTER COLUMN sig_path DROP NOT NULL;
ALTER TABLE versions ALTER COLUMN sig_path SET DEFAULT '';
