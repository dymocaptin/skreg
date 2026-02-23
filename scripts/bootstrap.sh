#!/usr/bin/env bash
# bootstrap.sh — Create the SSE-S3 encrypted Pulumi state bucket for skreg.
# Usage: ./scripts/bootstrap.sh <aws-account-id> [region]
set -euo pipefail

ACCOUNT_ID="${1:?Usage: bootstrap.sh <aws-account-id> [region]}"
REGION="${2:-us-west-2}"
BUCKET="skreg-pulumi-state-${ACCOUNT_ID}"

echo "Creating Pulumi state bucket: s3://${BUCKET} in ${REGION}"

if [[ "${REGION}" == "us-east-1" ]]; then
    aws s3api create-bucket --bucket "${BUCKET}" --region "${REGION}"
else
    aws s3api create-bucket \
        --bucket "${BUCKET}" \
        --region "${REGION}" \
        --create-bucket-configuration LocationConstraint="${REGION}"
fi

aws s3api put-bucket-encryption \
    --bucket "${BUCKET}" \
    --server-side-encryption-configuration '{
        "Rules": [{
            "ApplyServerSideEncryptionByDefault": {"SSEAlgorithm": "AES256"},
            "BucketKeyEnabled": true
        }]
    }'

aws s3api put-public-access-block \
    --bucket "${BUCKET}" \
    --public-access-block-configuration \
        "BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true"

aws s3api put-bucket-versioning \
    --bucket "${BUCKET}" \
    --versioning-configuration Status=Enabled

echo ""
echo "Done. Run the following to configure Pulumi:"
echo "  pulumi login s3://${BUCKET}"
