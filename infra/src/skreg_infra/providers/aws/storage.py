"""S3-backed object store with static IAM credentials.

Provisions:
- S3 bucket named ``skreg-packages`` (physical name fixed at construction time so
  the string is known without Output resolution)
- IAM user scoped to that bucket
- IAM access key pair
- Kubernetes Secret ``skreg-minio`` (namespace ``skreg``) with keys
  AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, matching the existing MinIO secret
  shape so the app is unchanged

Yields ObjectStoreContract(
    endpoint="https://s3.us-west-2.amazonaws.com",
    bucket="skreg-packages",
    credentials_secret_name="skreg-minio",
).

NOTE — IRSA path deferred:
The ObjectStoreContract mandates a credentials_secret_name; IRSA has no credentials
secret. This is the headline contract-fit issue surfaced by the AWS pathfinder. A
follow-up task (S2-T5b in the design spec) will extend ObjectStoreContract with an
optional auth_mode and service_account_annotations field and switch this component
to IRSA. For now, static access keys keep the seam valid without contract churn.
"""

from __future__ import annotations

import json

import pulumi
import pulumi_aws as aws
import pulumi_kubernetes as k8s

from skreg_infra.contracts import ObjectStoreContract

_BUCKET_NAME = "skreg-packages"
_SECRET_NAME = "skreg-minio"  # noqa: S105
_ENDPOINT = "https://s3.us-west-2.amazonaws.com"
_NAMESPACE = "skreg"


class S3ObjectStore(pulumi.ComponentResource):
    """S3 bucket + scoped IAM user + Kubernetes credentials secret.

    Args:
        name: Pulumi resource name prefix.
        opts: Pulumi resource options.
    """

    def __init__(
        self,
        name: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:S3ObjectStore", name, {}, opts)
        child_opts = pulumi.ResourceOptions(parent=self)

        # ── S3 bucket ─────────────────────────────────────────────────────────
        bucket = aws.s3.Bucket(
            f"{name}-bucket",
            bucket=_BUCKET_NAME,
            versioning=aws.s3.BucketVersioningArgs(enabled=True),
            server_side_encryption_configuration=aws.s3.BucketServerSideEncryptionConfigurationArgs(
                rule=aws.s3.BucketServerSideEncryptionConfigurationRuleArgs(
                    apply_server_side_encryption_by_default=aws.s3.BucketServerSideEncryptionConfigurationRuleApplyServerSideEncryptionByDefaultArgs(
                        sse_algorithm="AES256",
                    )
                )
            ),
            tags={"Name": _BUCKET_NAME},
            opts=child_opts,
        )

        aws.s3.BucketPublicAccessBlock(
            f"{name}-public-access-block",
            bucket=bucket.id,
            block_public_acls=True,
            block_public_policy=True,
            ignore_public_acls=True,
            restrict_public_buckets=True,
            opts=child_opts,
        )

        # ── IAM user + bucket-scoped policy ───────────────────────────────────
        iam_user = aws.iam.User(
            f"{name}-user",
            name=f"skreg-{name}-s3",
            tags={"Name": f"skreg-{name}-s3"},
            opts=child_opts,
        )

        bucket_policy_doc = pulumi.Output.all(bucket.arn).apply(
            lambda args: json.dumps(
                {
                    "Version": "2012-10-17",
                    "Statement": [
                        {
                            "Effect": "Allow",
                            "Action": [
                                "s3:GetObject",
                                "s3:PutObject",
                                "s3:DeleteObject",
                                "s3:ListBucket",
                            ],
                            "Resource": [
                                args[0],
                                f"{args[0]}/*",
                            ],
                        }
                    ],
                }
            )
        )

        aws.iam.UserPolicy(
            f"{name}-user-policy",
            user=iam_user.name,
            policy=bucket_policy_doc,
            opts=child_opts,
        )

        # ── Access key ────────────────────────────────────────────────────────
        access_key = aws.iam.AccessKey(
            f"{name}-access-key",
            user=iam_user.name,
            opts=child_opts,
        )

        # ── Kubernetes Secret ─────────────────────────────────────────────────
        k8s.core.v1.Secret(
            f"{name}-k8s-secret",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_SECRET_NAME,
                namespace=_NAMESPACE,
            ),
            string_data={
                "AWS_ACCESS_KEY_ID": access_key.id,
                "AWS_SECRET_ACCESS_KEY": access_key.secret,
            },
            opts=child_opts,
        )

        self._contract = ObjectStoreContract(
            endpoint=_ENDPOINT,
            bucket=_BUCKET_NAME,
            credentials_secret_name=_SECRET_NAME,
        )
        self.register_outputs(
            {
                "bucket": _BUCKET_NAME,
                "endpoint": _ENDPOINT,
                "credentials_secret_name": _SECRET_NAME,
            }
        )

    @property
    def contract(self) -> ObjectStoreContract:
        return self._contract
