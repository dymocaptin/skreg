"""In-cluster PostgreSQL via the CloudNativePG operator.

Installs the cloudnative-pg operator (Helm) and a Postgres Cluster custom
resource. The existing `skreg-db` secret (key DATABASE_URL) remains the
connection point, so compute is unchanged. Continuous backup to the in-cluster
MinIO bucket is configured via barmanObjectStore.
"""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs, RepositoryOptsArgs

from skreg_infra.contracts import DatabaseContract

_NAMESPACE = "skreg-infra"
_SECRET_NAME = "skreg-db"  # noqa: S105
_CLUSTER_NAME = "skreg-db-pg"


class K8sCnpgDatabaseOutputs:
    def __init__(self, secret_name: str) -> None:
        self.secret_name = secret_name


class K8sCnpgDatabase(pulumi.ComponentResource):
    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:CnpgDatabase", name, {}, opts)

        self._secret_name = _SECRET_NAME

        operator = Release(
            f"{name}-operator",
            ReleaseArgs(
                name="cnpg",
                chart="cloudnative-pg",
                repository_opts=RepositoryOptsArgs(repo="https://cloudnative-pg.github.io/charts"),
                version="0.22.1",
                namespace="cnpg-system",
                create_namespace=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        k8s.apiextensions.CustomResource(
            f"{name}-cluster",
            api_version="postgresql.cnpg.io/v1",
            kind="Cluster",
            metadata=k8s.meta.v1.ObjectMetaArgs(name=_CLUSTER_NAME, namespace=_NAMESPACE),
            spec={
                "instances": 2,
                "storage": {"size": "10Gi"},
                "bootstrap": {
                    # No secret ref: CNPG generates the app credentials itself
                    # and publishes them (incl. connection URI) in the
                    # `skreg-db-pg-app` Secret. The skreg-db secret consumed by
                    # compute is cut over to that URI during migration.
                    "initdb": {
                        "database": "skreg",
                        "owner": "skreg",
                    }
                },
                "backup": {
                    "barmanObjectStore": {
                        "destinationPath": "s3://skreg/db-backups",
                        "endpointURL": "http://skreg-storage-minio.skreg-infra.svc:9000",
                        "s3Credentials": {
                            "accessKeyId": {
                                "name": "skreg-minio",
                                "key": "AWS_ACCESS_KEY_ID",
                            },
                            "secretAccessKey": {
                                "name": "skreg-minio",
                                "key": "AWS_SECRET_ACCESS_KEY",
                            },
                        },
                    },
                    "retentionPolicy": "30d",
                },
            },
            opts=pulumi.ResourceOptions(parent=self, depends_on=[operator]),
        )

        self._outputs = K8sCnpgDatabaseOutputs(secret_name=_SECRET_NAME)
        self._contract = DatabaseContract(
            dsn_secret_name=_SECRET_NAME, dsn_secret_key="DATABASE_URL"  # noqa: S106
        )
        self.register_outputs({"secret_name": _SECRET_NAME})

    @property
    def outputs(self) -> K8sCnpgDatabaseOutputs:
        return self._outputs

    @property
    def contract(self) -> DatabaseContract:
        return self._contract
