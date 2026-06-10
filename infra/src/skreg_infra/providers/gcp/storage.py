"""GCS object store with HMAC key for S3-compatible access.

Provisions a GCS bucket, a service account, and an HMAC key so the skreg
API/worker can use their S3-compatible client unchanged. Credentials are
injected via a Kubernetes Secret using the same key names as the MinIO secret.
"""

from __future__ import annotations

import pulumi
import pulumi_gcp as gcp
import pulumi_kubernetes as k8s

from skreg_infra.contracts import ObjectStoreContract

_DEFAULT_BUCKET = "skreg-packages"
_SECRET_NAME = "skreg-minio"  # noqa: S105
_NAMESPACE = "skreg"
_GCS_ENDPOINT = "https://storage.googleapis.com"


class GcsObjectStore(pulumi.ComponentResource):
    """GCS bucket with HMAC key and a Kubernetes Secret for S3-compatible access."""

    def __init__(
        self,
        name: str,
        project: str,
        bucket_name: str = _DEFAULT_BUCKET,
        location: str = "US",
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:gcp:GcsObjectStore", name, {}, opts)

        self._bucket = gcp.storage.Bucket(
            f"{name}-bucket",
            project=project,
            name=bucket_name,
            location=location,
            uniform_bucket_level_access=True,
            force_destroy=False,
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._service_account = gcp.serviceaccount.Account(
            f"{name}-sa",
            project=project,
            account_id=f"{name}-gcs-sa",
            display_name="skreg GCS HMAC service account",
            opts=pulumi.ResourceOptions(parent=self),
        )

        # Grant the service account object admin access to the bucket
        gcp.storage.BucketIAMMember(
            f"{name}-bucket-iam",
            bucket=self._bucket.name,
            role="roles/storage.objectAdmin",
            member=self._service_account.email.apply(lambda email: f"serviceAccount:{email}"),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[self._service_account]),
        )

        self._hmac_key = gcp.storage.HmacKey(
            f"{name}-hmac",
            project=project,
            service_account_email=self._service_account.email,
            opts=pulumi.ResourceOptions(parent=self, depends_on=[self._service_account]),
        )

        self._secret = k8s.core.v1.Secret(
            f"{name}-k8s-secret",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_SECRET_NAME,
                namespace=_NAMESPACE,
            ),
            string_data={
                "AWS_ACCESS_KEY_ID": self._hmac_key.access_id,
                "AWS_SECRET_ACCESS_KEY": self._hmac_key.secret,
            },
            opts=pulumi.ResourceOptions(
                parent=self,
                depends_on=[self._hmac_key],
            ),
        )

        self._contract = ObjectStoreContract(
            endpoint=_GCS_ENDPOINT,
            bucket=bucket_name,
            credentials_secret_name=_SECRET_NAME,
        )

        self.register_outputs(
            {
                "bucket_name": bucket_name,
                "secret_name": _SECRET_NAME,
            }
        )

    @property
    def contract(self) -> ObjectStoreContract:
        return self._contract
