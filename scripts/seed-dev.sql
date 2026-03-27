-- scripts/seed-dev.sql
-- Idempotent dev database seed. Called by postgres-seed service on compose up.

-- 'individual' is the only valid kind for a personal dev namespace
-- (CHECK constraint: kind IN ('individual', 'org')).
INSERT INTO namespaces (slug, kind, created_at)
VALUES ('devuser', 'individual', now())
ON CONFLICT (slug) DO NOTHING;

-- api_keys has no unique constraint other than id, so ON CONFLICT DO NOTHING
-- is ineffective. Use WHERE NOT EXISTS to guard idempotency.
INSERT INTO api_keys (namespace_id, key_hash, email, created_at)
SELECT n.id, :'dev_key_hash', 'dev@localhost', now()
FROM namespaces n
WHERE n.slug = 'devuser'
  AND NOT EXISTS (
    SELECT 1 FROM api_keys ak WHERE ak.namespace_id = n.id AND ak.email = 'dev@localhost'
  );
