"""Bitnami PostgreSQL Helm release."""
from __future__ import annotations

import pulumi
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs, RepositoryOptsArgs


class K8sDatabaseOutputs:
    def __init__(self, secret_name: str) -> None:
        self.secret_name = secret_name


class K8sDatabase(pulumi.ComponentResource):
    """Single-node PostgreSQL 16 with a K8s Secret holding DATABASE_URL."""

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Database", name, {}, opts)

        self._secret_name = "skreg-db"  # noqa: S105

        self._release = Release(
            f"{name}-pg",
            ReleaseArgs(
                chart="postgresql",
                repository_opts=RepositoryOptsArgs(repo="https://charts.bitnami.com/bitnami"),
                version="16.4.5",
                namespace="skreg-infra",
                create_namespace=False,
                values={
                    "auth": {
                        "database": "skreg",
                        "username": "skreg",
                        "existingSecret": self._secret_name,
                        "secretKeys": {
                            "adminPasswordKey": "postgres-password",
                            "userPasswordKey": "password",
                        },
                    },
                    "primary": {
                        "persistence": {"size": "10Gi"},
                        "resources": {
                            "requests": {"cpu": "100m", "memory": "256Mi"},
                        },
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs = K8sDatabaseOutputs(secret_name=self._secret_name)
        self.register_outputs({"secret_name": self._secret_name})

    @property
    def outputs(self) -> K8sDatabaseOutputs:
        return self._outputs
