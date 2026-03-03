#!/usr/bin/env bash
# bootstrap.sh — One-time setup: Pulumi state bucket, ECR repos, and GitHub OIDC roles.
# Usage: ./scripts/bootstrap.sh <aws-account-id> [region]
set -euo pipefail

ACCOUNT_ID="${1:?Usage: bootstrap.sh <aws-account-id> [region]}"
REGION="${2:-us-west-2}"
BUCKET="skreg-pulumi-state-${ACCOUNT_ID}"
GITHUB_REPO="dymocaptin/skreg"

echo "Creating Pulumi state bucket: s3://${BUCKET} in ${REGION}"

if aws s3api head-bucket --bucket "${BUCKET}" 2>/dev/null; then
    echo "  Bucket already exists, skipping creation"
elif [[ "${REGION}" == "us-east-1" ]]; then
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

echo "Creating ECR repositories..."
aws ecr create-repository \
    --repository-name skreg-api \
    --image-tag-mutability MUTABLE \
    --region "${REGION}" 2>/dev/null && echo "  Created skreg-api" || echo "  skreg-api already exists"
aws ecr create-repository \
    --repository-name skreg-worker \
    --image-tag-mutability MUTABLE \
    --region "${REGION}" 2>/dev/null && echo "  Created skreg-worker" || echo "  skreg-worker already exists"

echo "Setting up GitHub OIDC..."

OIDC_PROVIDER_ARN="arn:aws:iam::${ACCOUNT_ID}:oidc-provider/token.actions.githubusercontent.com"

if aws iam get-open-id-connect-provider \
        --open-id-connect-provider-arn "${OIDC_PROVIDER_ARN}" &>/dev/null; then
    echo "  GitHub OIDC provider already exists"
else
    aws iam create-open-id-connect-provider \
        --url "https://token.actions.githubusercontent.com" \
        --client-id-list "sts.amazonaws.com" \
        --thumbprint-list \
            "6938fd4d98bab03faadb97b34396831e3780aea1" \
            "1c58a3a8518e8759bf075b76b750d4f2df264fcd" \
        >/dev/null
    echo "  Created GitHub OIDC provider"
fi

TRUST_POLICY=$(cat <<EOF
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {"Federated": "${OIDC_PROVIDER_ARN}"},
    "Action": "sts:AssumeRoleWithWebIdentity",
    "Condition": {
      "StringEquals": {
        "token.actions.githubusercontent.com:aud": "sts.amazonaws.com",
        "token.actions.githubusercontent.com:sub": "repo:${GITHUB_REPO}:ref:refs/heads/main"
      }
    }
  }]
}
EOF
)

ECR_POLICY=$(cat <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["ecr:GetAuthorizationToken"],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "ecr:BatchCheckLayerAvailability",
        "ecr:PutImage",
        "ecr:InitiateLayerUpload",
        "ecr:UploadLayerPart",
        "ecr:CompleteLayerUpload"
      ],
      "Resource": "arn:aws:ecr:${REGION}:${ACCOUNT_ID}:repository/skreg-*"
    }
  ]
}
EOF
)

ECR_ROLE="skreg-oidc-gh-role"
if aws iam get-role --role-name "${ECR_ROLE}" &>/dev/null; then
    echo "  ${ECR_ROLE} already exists"
else
    aws iam create-role \
        --role-name "${ECR_ROLE}" \
        --assume-role-policy-document "${TRUST_POLICY}" \
        >/dev/null
    aws iam put-role-policy \
        --role-name "${ECR_ROLE}" \
        --policy-name "skreg-ecr-push" \
        --policy-document "${ECR_POLICY}"
    echo "  Created ${ECR_ROLE}"
fi

DEPLOY_ROLE="skreg-oidc-deploy-role"
if aws iam get-role --role-name "${DEPLOY_ROLE}" &>/dev/null; then
    echo "  ${DEPLOY_ROLE} already exists"
else
    aws iam create-role \
        --role-name "${DEPLOY_ROLE}" \
        --assume-role-policy-document "${TRUST_POLICY}" \
        >/dev/null
    aws iam attach-role-policy \
        --role-name "${DEPLOY_ROLE}" \
        --policy-arn "arn:aws:iam::aws:policy/AdministratorAccess"
    echo "  Created ${DEPLOY_ROLE}"
fi

ECR_ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/${ECR_ROLE}"
DEPLOY_ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/${DEPLOY_ROLE}"

echo ""
echo "Done. Run the following to configure Pulumi:"
echo "  pulumi login s3://${BUCKET}"
echo ""
echo "Set these GitHub Actions variables:"
echo "  AWS_ROLE_ARN=${ECR_ROLE_ARN}"
echo "  AWS_DEPLOY_ROLE_ARN=${DEPLOY_ROLE_ARN}"
