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
