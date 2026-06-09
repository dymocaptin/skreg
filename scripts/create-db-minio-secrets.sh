#!/usr/bin/env bash
# Creates skreg-db and skreg-minio K8s secrets with strong random credentials.
# Usage: ./scripts/create-db-minio-secrets.sh <user@host>
#   <user@host>  SSH target where kubectl is available (e.g. pk@myserver)
set -euo pipefail

REMOTE="${1:?Usage: $0 <user@host>}"
KUBECTL="~/.local/bin/kubectl"

TMPDIR="$(mktemp -d)"
trap 'shred -u "$TMPDIR"/* 2>/dev/null; rm -rf "$TMPDIR"' EXIT

echo "Generating credentials..."
# Use printf to avoid trailing newlines; tr+cut strips base64 padding chars
openssl rand -base64 32 | tr -d '/+=' | cut -c1-32 | tr -d '\n' > "$TMPDIR/pg_admin_pw"
openssl rand -base64 32 | tr -d '/+=' | cut -c1-32 | tr -d '\n' > "$TMPDIR/pg_user_pw"
openssl rand -base64 32 | tr -d '/+=' | cut -c1-32 | tr -d '\n' > "$TMPDIR/minio_access"
openssl rand -base64 32 | tr -d '/+=' | cut -c1-32 | tr -d '\n' > "$TMPDIR/minio_secret"

PG_USER_PW="$(cat "$TMPDIR/pg_user_pw")"
printf "postgresql://skreg:%s@skreg-db-postgresql.skreg-infra.svc:5432/skreg?sslmode=disable" \
    "$PG_USER_PW" > "$TMPDIR/database_url"

echo "Transferring to ${REMOTE}..."
ssh "$REMOTE" "mkdir -p /tmp/skreg-secrets-tmp && chmod 700 /tmp/skreg-secrets-tmp"

scp -q "$TMPDIR/pg_admin_pw"  "$REMOTE:/tmp/skreg-secrets-tmp/postgres-password"
scp -q "$TMPDIR/pg_user_pw"   "$REMOTE:/tmp/skreg-secrets-tmp/password"
scp -q "$TMPDIR/database_url" "$REMOTE:/tmp/skreg-secrets-tmp/DATABASE_URL"
scp -q "$TMPDIR/minio_access" "$REMOTE:/tmp/skreg-secrets-tmp/rootUser"
scp -q "$TMPDIR/minio_secret" "$REMOTE:/tmp/skreg-secrets-tmp/rootPassword"
scp -q "$TMPDIR/minio_access" "$REMOTE:/tmp/skreg-secrets-tmp/AWS_ACCESS_KEY_ID"
scp -q "$TMPDIR/minio_secret" "$REMOTE:/tmp/skreg-secrets-tmp/AWS_SECRET_ACCESS_KEY"

# Use --from-literal (via command substitution) so kubectl does not add trailing
# newlines the way --from-file does. The files have no trailing newlines either,
# but this is an extra belt-and-suspenders guard.
ssh "$REMOTE" "
    set -e
    PG_ADMIN=\$(cat /tmp/skreg-secrets-tmp/postgres-password)
    PG_USER=\$(cat /tmp/skreg-secrets-tmp/password)
    DB_URL=\$(cat /tmp/skreg-secrets-tmp/DATABASE_URL)
    MINIO_USER=\$(cat /tmp/skreg-secrets-tmp/rootUser)
    MINIO_PASS=\$(cat /tmp/skreg-secrets-tmp/rootPassword)

    $KUBECTL delete secret skreg-db --namespace skreg-infra --ignore-not-found
    $KUBECTL create secret generic skreg-db --namespace skreg-infra \
        --from-literal=postgres-password=\"\$PG_ADMIN\" \
        --from-literal=password=\"\$PG_USER\" \
        --from-literal=DATABASE_URL=\"\$DB_URL\"

    $KUBECTL delete secret skreg-minio --namespace skreg-infra --ignore-not-found
    $KUBECTL create secret generic skreg-minio --namespace skreg-infra \
        --from-literal=rootUser=\"\$MINIO_USER\" \
        --from-literal=rootPassword=\"\$MINIO_PASS\" \
        --from-literal=AWS_ACCESS_KEY_ID=\"\$MINIO_USER\" \
        --from-literal=AWS_SECRET_ACCESS_KEY=\"\$MINIO_PASS\"

    # Mirror into the skreg namespace so the API pod can read it
    $KUBECTL delete secret skreg-minio --namespace skreg --ignore-not-found 2>/dev/null || true
    $KUBECTL create secret generic skreg-minio --namespace skreg \
        --from-literal=rootUser=\"\$MINIO_USER\" \
        --from-literal=rootPassword=\"\$MINIO_PASS\" \
        --from-literal=AWS_ACCESS_KEY_ID=\"\$MINIO_USER\" \
        --from-literal=AWS_SECRET_ACCESS_KEY=\"\$MINIO_PASS\"

    shred -u /tmp/skreg-secrets-tmp/* 2>/dev/null
    rm -rf /tmp/skreg-secrets-tmp

    echo 'Secrets created:'
    $KUBECTL get secret skreg-db --namespace skreg-infra
    $KUBECTL get secret skreg-minio --namespace skreg-infra
    $KUBECTL get secret skreg-minio --namespace skreg
"

echo "Done. Local credentials shredded on exit."
