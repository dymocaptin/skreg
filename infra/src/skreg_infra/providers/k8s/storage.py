"""MinIO Helm release — S3-compatible package storage."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s
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
    NAMESPACE = "skreg-infra"
    ENDPOINT = "http://skreg-storage-minio.skreg-infra.svc:9000"
    MC_IMAGE = "quay.io/minio/mc:RELEASE.2024-11-21T17-21-54Z"

    # Bucket-notification wiring: MinIO posts `.skill` ObjectCreated events to
    # the dispatcher, which spawns vetting worker Jobs. The webhook target is
    # declared via server env (the ARN below); the bucket→ARN binding is applied
    # by an idempotent Job because `mc event add` is not repeatable under the
    # chart post-job's `set -e`.
    WEBHOOK_ENDPOINT = "http://skreg-dispatcher.skreg.svc.cluster.local:9090/notify"
    WEBHOOK_ARN = "arn:minio:sqs::DISPATCHER:webhook"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Storage", name, {}, opts)

        self._bucket_name = self.BUCKET

        release = Release(
            f"{name}-minio",
            ReleaseArgs(
                # Pin release name so the K8s service is always "skreg-storage-minio"
                # and matches the ENDPOINT constant above.
                name="skreg-storage-minio",
                chart="minio",
                repository_opts=RepositoryOptsArgs(repo="https://charts.min.io/"),
                version="5.4.0",
                namespace=self.NAMESPACE,
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
                    # Pod runs under this ServiceAccount; create it with the release.
                    "serviceAccount": {"create": True, "name": "minio-sa"},
                    # Register the dispatcher webhook as a notification target.
                    # Stripping these would silently break the vetting pipeline.
                    "environment": {
                        "MINIO_NOTIFY_WEBHOOK_ENABLE_DISPATCHER": "on",
                        "MINIO_NOTIFY_WEBHOOK_ENDPOINT_DISPATCHER": self.WEBHOOK_ENDPOINT,
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        # Bind the bucket to the webhook ARN for `.skill` uploads. Idempotent:
        # tolerates an existing binding, retries until MinIO (with the webhook
        # target env above) is reachable.
        event_script = (
            "set -e; "
            "for i in $(seq 1 60); do "
            f'mc alias set m {self.ENDPOINT} "$MC_USER" "$MC_PASS" >/dev/null 2>&1 '
            "&& break || sleep 4; done; "
            f"mc event add m/{self.BUCKET} {self.WEBHOOK_ARN} "
            "--event put --suffix .skill >/tmp/o 2>&1 "
            '|| grep -qi "already exists" /tmp/o; cat /tmp/o'
        )
        k8s.batch.v1.Job(
            f"{name}-event",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name="skreg-storage-minio-event-setup",
                namespace=self.NAMESPACE,
            ),
            spec=k8s.batch.v1.JobSpecArgs(
                backoff_limit=20,
                template=k8s.core.v1.PodTemplateSpecArgs(
                    spec=k8s.core.v1.PodSpecArgs(
                        restart_policy="OnFailure",
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="mc",
                                image=self.MC_IMAGE,
                                command=["/bin/sh", "-c"],
                                args=[event_script],
                                env=[
                                    k8s.core.v1.EnvVarArgs(
                                        name="MC_USER",
                                        value_from=k8s.core.v1.EnvVarSourceArgs(
                                            secret_key_ref=k8s.core.v1.SecretKeySelectorArgs(
                                                name=self.SECRET, key="rootUser"
                                            )
                                        ),
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="MC_PASS",
                                        value_from=k8s.core.v1.EnvVarSourceArgs(
                                            secret_key_ref=k8s.core.v1.SecretKeySelectorArgs(
                                                name=self.SECRET, key="rootPassword"
                                            )
                                        ),
                                    ),
                                ],
                            )
                        ],
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[release]),
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
