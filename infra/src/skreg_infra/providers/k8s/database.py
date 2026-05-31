"""PostgreSQL 16 StatefulSet — plain K8s resources, no Helm dependency."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sDatabaseOutputs:
    def __init__(self, secret_name: str) -> None:
        self.secret_name = secret_name


class K8sDatabase(pulumi.ComponentResource):
    """Single-node PostgreSQL 16 using the official postgres:16 image.

    Service name is skreg-db-postgresql to match the DATABASE_URL stored in the
    skreg-db secret. Passwords are sourced from that pre-existing secret.
    """

    SERVICE_NAME = "skreg-db-postgresql"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Database", name, {}, opts)

        self._secret_name = "skreg-db"  # noqa: S105
        labels = {"app": "postgresql"}

        pvc = k8s.core.v1.PersistentVolumeClaim(
            f"{name}-pvc",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="postgresql-data", namespace="skreg-infra"),
            spec=k8s.core.v1.PersistentVolumeClaimSpecArgs(
                access_modes=["ReadWriteOnce"],
                resources=k8s.core.v1.VolumeResourceRequirementsArgs(
                    requests={"storage": "10Gi"},
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        sts = k8s.apps.v1.StatefulSet(
            f"{name}-sts",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="postgresql", namespace="skreg-infra"),
            spec=k8s.apps.v1.StatefulSetSpecArgs(
                replicas=1,
                service_name=self.SERVICE_NAME,
                selector=k8s.meta.v1.LabelSelectorArgs(match_labels=labels),
                template=k8s.core.v1.PodTemplateSpecArgs(
                    metadata=k8s.meta.v1.ObjectMetaArgs(labels=labels),
                    spec=k8s.core.v1.PodSpecArgs(
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="postgresql",
                                image="postgres:16",
                                ports=[k8s.core.v1.ContainerPortArgs(container_port=5432)],
                                env=[
                                    k8s.core.v1.EnvVarArgs(
                                        name="POSTGRES_DB",
                                        value="skreg",
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="POSTGRES_USER",
                                        value="skreg",
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="POSTGRES_PASSWORD",
                                        value_from=k8s.core.v1.EnvVarSourceArgs(
                                            secret_key_ref=k8s.core.v1.SecretKeySelectorArgs(
                                                name="skreg-db",
                                                key="password",
                                            )
                                        ),
                                    ),
                                ],
                                resources=k8s.core.v1.ResourceRequirementsArgs(
                                    requests={"cpu": "100m", "memory": "256Mi"},
                                ),
                                volume_mounts=[
                                    k8s.core.v1.VolumeMountArgs(
                                        name="data", mount_path="/var/lib/postgresql/data"
                                    )
                                ],
                                readiness_probe=k8s.core.v1.ProbeArgs(
                                    exec_=k8s.core.v1.ExecActionArgs(
                                        command=["pg_isready", "-U", "skreg"]
                                    ),
                                    initial_delay_seconds=5,
                                    period_seconds=10,
                                ),
                            )
                        ],
                        volumes=[
                            k8s.core.v1.VolumeArgs(
                                name="data",
                                persistent_volume_claim=k8s.core.v1.PersistentVolumeClaimVolumeSourceArgs(
                                    claim_name="postgresql-data"
                                ),
                            )
                        ],
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[pvc]),
        )

        k8s.core.v1.Service(
            f"{name}-svc",
            metadata=k8s.meta.v1.ObjectMetaArgs(name=self.SERVICE_NAME, namespace="skreg-infra"),
            spec=k8s.core.v1.ServiceSpecArgs(
                selector=labels,
                ports=[k8s.core.v1.ServicePortArgs(port=5432, target_port=5432)],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[sts]),
        )

        self._outputs = K8sDatabaseOutputs(secret_name=self._secret_name)
        self.register_outputs({"secret_name": self._secret_name})

    @property
    def outputs(self) -> K8sDatabaseOutputs:
        return self._outputs
