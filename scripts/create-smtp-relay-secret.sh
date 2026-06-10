#!/usr/bin/env bash
#
# Create/replace the `skreg-smtp-relay` Secret consumed by the postfix relay
# (see infra/src/skreg_infra/providers/k8s/email.py).
#
# Postfix forwards all outbound mail through this upstream smarthost on the
# submission port (587) because the cluster cannot reach recipient MX servers
# on port 25.
#
# Defaults target Namecheap Private Email for the skreg.ai domain. Override via
# the environment if you switch providers, e.g.:
#   RELAYHOST='[smtp.gmail.com]:587' SMTP_USERNAME='you@gmail.com' ./create-smtp-relay-secret.sh
#
# The password is read interactively (never passed on the command line, never
# echoed) so it stays out of shell history and process listings.
set -euo pipefail

NAMESPACE="${NAMESPACE:-skreg}"
SECRET_NAME="${SECRET_NAME:-skreg-smtp-relay}"
RELAYHOST="${RELAYHOST:-[mail.privateemail.com]:587}"
SMTP_USERNAME="${SMTP_USERNAME:-skreg@skreg.ai}"

printf 'SMTP password for %s: ' "$SMTP_USERNAME" >&2
read -rs SMTP_PASSWORD
printf '\n' >&2

if [[ -z "${SMTP_PASSWORD}" ]]; then
  echo "error: empty password" >&2
  exit 1
fi

kubectl create secret generic "$SECRET_NAME" \
  --namespace "$NAMESPACE" \
  --from-literal=relayhost="$RELAYHOST" \
  --from-literal=username="$SMTP_USERNAME" \
  --from-literal=password="$SMTP_PASSWORD" \
  --dry-run=client -o yaml | kubectl apply -f -

echo "secret/$SECRET_NAME applied in namespace $NAMESPACE (relayhost=$RELAYHOST, username=$SMTP_USERNAME)" >&2
