# skreg AWS Deployment Design

**Date:** 2026-02-21
**Status:** Approved

---

## Overview

Complete the Pulumi Python infra to deploy skreg into AWS `us-west-2`. Adds the four missing
infrastructure components (`AwsNetwork`, `AwsPki`, `AwsCompute`, `AwsOidc`), wires them together
in `SkillpkgStack.run()`, adds multi-stage Dockerfiles for both binaries, and extends CI to build
and push images to ECR via GitHub Actions OIDC.

---

## Decisions

| Concern | Decision |
|---|---|
| PKI backend | Software — RSA-4096 root CA key in Secrets Manager; upgrade to CloudHSM later |
| Pulumi state | S3 bucket, SSE-S3 encrypted, bootstrapped by a one-time shell script |
| State bucket bootstrap | `scripts/bootstrap.sh` via AWS CLI — bucket lives outside any Pulumi stack |
| VPC | New dedicated VPC, 10.0.0.0/16, us-west-2a + us-west-2b |
| NAT Gateway | Single (us-west-2a) — enable per-AZ later when `multi_az = true` |
| Compute | ECS Fargate — no EC2 management |
| CI credentials | GitHub Actions OIDC — no long-lived access keys |
| Pulumi deployments | Manual (`pulumi up` run locally) — CI automation deferred |

---

## Section 1: Architecture

Eight pieces of new work, built in dependency order:

| # | What | Where |
|---|---|---|
| 1 | `scripts/bootstrap.sh` | creates SSE-S3 state bucket via AWS CLI, run once |
| 2 | `AwsNetwork` | VPC, public/private subnets, IGW, NAT GW |
| 3 | `AwsPki` (software) | RSA-4096 root CA key + cert in Secrets Manager |
| 4 | `AwsCompute` | ECR repos, ECS cluster, ALB, two Fargate services |
| 5 | `AwsOidc` | GitHub OIDC identity provider + IAM role scoped to ECR push |
| 6 | `SkillpkgStack.run()` | wires all components, exports stack outputs |
| 7 | Dockerfiles × 2 | multi-stage Rust builds for `skreg-api` and `skreg-worker` |
| 8 | CI update | `build-push` job; `StackConfig` gains `api_image_uri` + `worker_image_uri` |

---

## Section 2: Network (`AwsNetwork`)

```
VPC: 10.0.0.0/16  (us-west-2)

Public subnets (IGW route):       Private subnets (NAT route):
  10.0.1.0/24  us-west-2a           10.0.10.0/24  us-west-2a
  10.0.2.0/24  us-west-2b           10.0.20.0/24  us-west-2b

NAT Gateway → us-west-2a public subnet (single, cost-optimised)
ALB          → public subnets
RDS, ECS     → private subnets
```

**File:** `infra/src/skillpkg_infra/providers/aws/network.py`

---

## Section 3: PKI — software backend (`AwsPki`)

RSA-4096 root CA generated with Python's `cryptography` library on first `pulumi up`.
Cert validity: 10 years. `ignore_changes` prevents accidental key rotation on subsequent runs.

**Secrets Manager secrets created:**
- `skreg/pki/root-ca-key` — PEM private key
- `skreg/pki/root-ca-cert` — PEM certificate

**CRL:** empty initial file at `/.well-known/crl.pem` in the packages S3 bucket.

**Stack output:** `root_ca_cert` — PEM cert for embedding in `skillpkg` CLI binary at build time.

**File:** `infra/src/skillpkg_infra/providers/aws/pki.py`

---

## Section 4: Compute (`AwsCompute`)

```
ALB (public subnets, port 80 → redirect 443, port 443 → 8080)
  └── ECS Service: skreg-api
        Task: 512 CPU / 1024 MB
        Port: 8080
        Env:  DATABASE_URL (from Secrets Manager), BIND_ADDR=0.0.0.0:8080

ECS Service: skreg-worker (private, no ALB)
        Task: 256 CPU / 512 MB
        Env:  DATABASE_URL (from Secrets Manager)
```

Both services use ECS secrets injection — no plaintext env vars in task definitions.
ALB uses the default CloudFront certificate initially; custom domain + ACM deferred.

**File:** `infra/src/skillpkg_infra/providers/aws/compute.py`

---

## Section 5: OIDC (`AwsOidc`)

GitHub Actions OIDC identity provider and IAM role scoped to ECR push from this repo only.

```
Trust policy:
  Principal:  oidc.token.actions.githubusercontent.com
  Condition:  sub = repo:dymocaptin/skreg:ref:refs/heads/main

Permissions (least-privilege):
  ecr:GetAuthorizationToken          (account-level)
  ecr:BatchCheckLayerAvailability    (skreg-api + skreg-worker repos)
  ecr:PutImage
  ecr:InitiateLayerUpload
  ecr:UploadLayerPart
  ecr:CompleteLayerUpload
```

Role ARN exported as `oidc_role_arn`. After first `pulumi up`, add to GitHub Actions as
repository variable `AWS_ROLE_ARN`.

**File:** `infra/src/skillpkg_infra/providers/aws/oidc.py`

---

## Section 6: `SkillpkgStack.run()` wiring

```python
network  = AwsNetwork("skreg-network")
storage  = AwsStorage("skreg-storage")
pki      = AwsPki("skreg-pki", bucket_name=storage.outputs.bucket_name)
database = AwsDatabase("skreg-db", AwsDatabaseArgs(
               vpc_id=network.outputs.vpc_id,
               subnet_ids=network.outputs.private_subnet_ids,
               multi_az=config.multi_az))
compute  = AwsCompute("skreg-compute", AwsComputeArgs(
               vpc_id=network.outputs.vpc_id,
               public_subnet_ids=network.outputs.public_subnet_ids,
               private_subnet_ids=network.outputs.private_subnet_ids,
               db_secret_name=database.outputs.connection_secret_name,
               api_image_uri=config.api_image_uri,
               worker_image_uri=config.worker_image_uri))
oidc     = AwsOidc("skreg-oidc", github_repo="dymocaptin/skreg")

pulumi.export("api_url",          compute.outputs.service_url)
pulumi.export("cdn_base_url",     storage.outputs.cdn_base_url)
pulumi.export("root_ca_cert",     pki.outputs.root_ca_cert_pem)
pulumi.export("ecr_api_repo",     compute.outputs.ecr_api_repo)
pulumi.export("ecr_worker_repo",  compute.outputs.ecr_worker_repo)
pulumi.export("oidc_role_arn",    oidc.outputs.role_arn)
```

`StackConfig` gains two new optional fields: `api_image_uri` and `worker_image_uri`
(default: `""` — ECS task definition uses a placeholder until first image push).

---

## Section 7: Dockerfiles

Multi-stage builds. Compile in `rust:1.85-slim`, run in `debian:bookworm-slim`.

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p <crate-name>

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/<binary> /usr/local/bin/<binary>
ENTRYPOINT ["/usr/local/bin/<binary>"]
```

**Files:**
- `crates/skreg-api/Dockerfile`
- `crates/skreg-worker/Dockerfile`

---

## Section 8: CI — `build-push` job

New job added to `.github/workflows/ci.yml`, runs on push to `main` after `rust` and
`python-infra` pass.

```yaml
build-push:
  needs: [rust, python-infra]
  permissions:
    id-token: write
    contents: read
  steps:
    - Checkout
    - Configure AWS credentials via OIDC (AWS_ROLE_ARN repository variable)
    - Login to ECR
    - Build & push skreg-api   → tagged :latest + :<git-sha>
    - Build & push skreg-worker → tagged :latest + :<git-sha>
```

---

## Bootstrap Procedure (one-time, run locally)

```bash
# 1. Create Pulumi state bucket
./scripts/bootstrap.sh <aws-account-id> us-west-2

# 2. Configure Pulumi to use S3 backend
pulumi login s3://skreg-pulumi-state-<aws-account-id>

# 3. Set required env vars
export SKILLPKG_CLOUD_PROVIDER=aws
export SKILLPKG_API_IMAGE_URI=""
export SKILLPKG_WORKER_IMAGE_URI=""

# 4. Deploy
cd infra && pulumi up

# 5. Copy oidc_role_arn output → GitHub repo variable AWS_ROLE_ARN
```
