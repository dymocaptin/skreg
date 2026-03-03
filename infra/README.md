# skreg Infrastructure

Pulumi IaC for deploying a self-hosted skreg registry on AWS (ECS Fargate +
RDS + S3 + CloudFront + ACM).

## Prerequisites

- [Pulumi CLI](https://www.pulumi.com/docs/install/)
- [uv](https://github.com/astral-sh/uv)
- AWS credentials configured in your environment
- A Pulumi backend (S3 bucket or Pulumi Cloud)

## Configuration

All configuration is via environment variables with the `SKREG_` prefix.

| Variable | Required | Description |
|---|---|---|
| `SKREG_CLOUD_PROVIDER` | Yes | Cloud provider. Currently only `aws` is supported. |
| `SKREG_DOMAIN_NAME` | No | Custom domain for the registry (e.g. `registry.example.com`). If omitted, the ALB DNS name is used. |
| `SKREG_EXISTING_CERT_ARN` | No | ARN of an existing ACM certificate to import instead of creating a new one. |
| `SKREG_MULTI_AZ` | No | Set to `true` for a multi-AZ RDS deployment. Defaults to `false`. |
| `SKREG_ENVIRONMENT` | No | One of `prod`, `staging`, `dev`. Defaults to `prod`. |

You will also need:

| Variable | Description |
|---|---|
| `PULUMI_BACKEND_URL` | S3 backend URL, e.g. `s3://my-pulumi-state-bucket` |
| `PULUMI_CONFIG_PASSPHRASE` | Passphrase for encrypting stack secrets |

## Deploy

```bash
cd infra
uv sync
pulumi stack select main   # or: pulumi stack init main
pulumi up
```

## Custom domain

If you set `SKREG_DOMAIN_NAME`, Pulumi will provision an ACM certificate via
DNS validation. After the first `pulumi up`, export the CNAME record:

```bash
pulumi stack output cert_validation_cname
```

Add that CNAME to your DNS provider, then re-run `pulumi up` to complete
certificate validation.

To reuse an existing validated certificate, set `SKREG_EXISTING_CERT_ARN` to
its ARN before running `pulumi up`.

## Outputs

| Output | Description |
|---|---|
| `api_url` | URL of the deployed registry API |
| `alb_dns_name` | ALB DNS name (useful for DNS CNAME targets) |
| `cert_validation_cname` | CNAME record needed to validate a new ACM certificate |
| `ecr_api_repo` | ECR repository URL for the API image |
| `ecr_worker_repo` | ECR repository URL for the worker image |
| `oidc_role_arn` | IAM role ARN for GitHub Actions ECR push |
| `deploy_role_arn` | IAM role ARN for GitHub Actions Pulumi deploy |

## CI/CD

The root `.github/workflows/ci.yml` builds and pushes Docker images to ECR,
then runs `pulumi up` on every push to `main`. Required GitHub secrets and
variables are documented in the workflow file.
