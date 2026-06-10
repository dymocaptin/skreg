"""Cloud SQL PostgreSQL 16 database with Kubernetes secret injection.

Provisions a Cloud SQL instance, creates the `skreg` database and user, and
writes the composed DSN into a Kubernetes Secret so compute can connect without
code changes.
"""

from __future__ import annotations

import pulumi
import pulumi_gcp as gcp
import pulumi_kubernetes as k8s
import pulumi_random as random

from skreg_infra.contracts import DatabaseContract

_DB_NAME = "skreg"
_DB_USER = "skreg"
_SECRET_NAME = "skreg-db"  # noqa: S105
_SECRET_KEY = "DATABASE_URL"  # noqa: S105
_NAMESPACE = "skreg"
_TIER = "db-f1-micro"
_PG_VERSION = "POSTGRES_16"


class CloudSqlDatabase(pulumi.ComponentResource):
    """Cloud SQL PostgreSQL 16 instance with a k8s Secret for the DSN."""

    def __init__(
        self,
        name: str,
        project: str,
        region: str = "us-central1",
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:gcp:CloudSqlDatabase", name, {}, opts)

        self._instance = gcp.sql.DatabaseInstance(
            f"{name}-instance",
            project=project,
            region=region,
            database_version=_PG_VERSION,
            deletion_protection=False,
            settings=gcp.sql.DatabaseInstanceSettingsArgs(
                tier=_TIER,
                ip_configuration=gcp.sql.DatabaseInstanceSettingsIpConfigurationArgs(
                    ipv4_enabled=True,
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._database = gcp.sql.Database(
            f"{name}-db",
            project=project,
            instance=self._instance.name,
            name=_DB_NAME,
            opts=pulumi.ResourceOptions(parent=self, depends_on=[self._instance]),
        )

        self._password = random.RandomPassword(
            f"{name}-password",
            length=32,
            special=False,
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._user = gcp.sql.User(
            f"{name}-user",
            project=project,
            instance=self._instance.name,
            name=_DB_USER,
            password=self._password.result,
            opts=pulumi.ResourceOptions(parent=self, depends_on=[self._instance]),
        )

        # Compose the DSN from Pulumi Outputs
        dsn = pulumi.Output.all(
            self._instance.public_ip_address,
            self._password.result,
        ).apply(lambda args: (f"postgresql://{_DB_USER}:{args[1]}@{args[0]}:5432/{_DB_NAME}"))

        self._secret = k8s.core.v1.Secret(
            f"{name}-k8s-secret",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_SECRET_NAME,
                namespace=_NAMESPACE,
            ),
            string_data={_SECRET_KEY: dsn},
            opts=pulumi.ResourceOptions(
                parent=self,
                depends_on=[self._user, self._database],
            ),
        )

        self._contract = DatabaseContract(
            dsn_secret_name=_SECRET_NAME,
            dsn_secret_key=_SECRET_KEY,
        )

        self.register_outputs(
            {
                "secret_name": _SECRET_NAME,
                "instance_name": self._instance.name,
            }
        )

    @property
    def contract(self) -> DatabaseContract:
        return self._contract
