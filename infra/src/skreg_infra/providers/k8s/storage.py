"""MinIO Helm release — S3-compatible package storage."""

from __future__ import annotations

import pulumi
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs, RepositoryOptsArgs


class K8sStorageOutputs:
    def __init__(self, bucket_name: str, endpoint: str, secret_name: str) -> None:
        self.bucket_name = bucket_name
        self.endpoint = endpoint
        self.secret_name = secret_name


class K8sStorage(pulumi.ComponentResource):
    """MinIO standalone mode. Credentials in `skreg-minio` Secret."""

    BUCKET = "skreg"
    SECRET = "skreg-minio"  # noqa: S105
    ENDPOINT = "http://skreg-storage-minio.skreg-infra.svc:9000"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Storage", name, {}, opts)

        self._bucket_name = self.BUCKET

        Release(
            f"{name}-minio",
            ReleaseArgs(
                # Pin release name so the K8s service is always "skreg-storage-minio"
                # and matches the ENDPOINT constant above.
                name="skreg-storage-minio",
                chart="minio",
                repository_opts=RepositoryOptsArgs(repo="https://charts.min.io/"),
                version="5.4.0",
                namespace="skreg-infra",
                create_namespace=False,
                values={
                    "mode": "standalone",
                    "existingSecret": self.SECRET,
                    "persistence": {"size": "20Gi"},
                    "buckets": [{"name": self.BUCKET, "policy": "none", "purge": False}],
                    "resources": {
                        "requests": {"cpu": "100m", "memory": "256Mi"},
                    },
                    "service": {"type": "ClusterIP"},
                },
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs = K8sStorageOutputs(
            bucket_name=self.BUCKET,
            endpoint=self.ENDPOINT,
            secret_name=self.SECRET,
        )
        self.register_outputs(
            {
                "bucket_name": self.BUCKET,
                "endpoint": self.ENDPOINT,
                "secret_name": self.SECRET,
            }
        )

    @property
    def outputs(self) -> K8sStorageOutputs:
        return self._outputs
