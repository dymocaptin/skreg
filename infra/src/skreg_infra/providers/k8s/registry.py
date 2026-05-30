"""Docker registry v2 in-cluster, exposed via NodePort 30500."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sRegistryOutputs:
    def __init__(self, registry_url: str) -> None:
        self.registry_url = registry_url


class K8sRegistry(pulumi.ComponentResource):
    """Docker registry v2, NodePort 30500 on the Kind host.

    Deployed as a plain Deployment + PVC + NodePort Service rather than a Helm
    chart to avoid the unreliable helm.twun.io repository.
    """

    REGISTRY_URL = "localhost:30500"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Registry", name, {}, opts)

        self._registry_url = self.REGISTRY_URL
        labels = {"app": "docker-registry"}

        pvc = k8s.core.v1.PersistentVolumeClaim(
            f"{name}-pvc",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="docker-registry", namespace="skreg-infra"),
            spec=k8s.core.v1.PersistentVolumeClaimSpecArgs(
                access_modes=["ReadWriteOnce"],
                resources=k8s.core.v1.VolumeResourceRequirementsArgs(
                    requests={"storage": "20Gi"},
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        deploy = k8s.apps.v1.Deployment(
            f"{name}-deploy",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="docker-registry", namespace="skreg-infra"),
            spec=k8s.apps.v1.DeploymentSpecArgs(
                replicas=1,
                selector=k8s.meta.v1.LabelSelectorArgs(match_labels=labels),
                template=k8s.core.v1.PodTemplateSpecArgs(
                    metadata=k8s.meta.v1.ObjectMetaArgs(labels=labels),
                    spec=k8s.core.v1.PodSpecArgs(
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="registry",
                                image="registry:2",
                                ports=[k8s.core.v1.ContainerPortArgs(container_port=5000)],
                                env=[
                                    k8s.core.v1.EnvVarArgs(
                                        name="REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY",
                                        value="/var/lib/registry",
                                    ),
                                ],
                                resources=k8s.core.v1.ResourceRequirementsArgs(
                                    requests={"cpu": "50m", "memory": "64Mi"},
                                ),
                                volume_mounts=[
                                    k8s.core.v1.VolumeMountArgs(
                                        name="data", mount_path="/var/lib/registry"
                                    )
                                ],
                            )
                        ],
                        volumes=[
                            k8s.core.v1.VolumeArgs(
                                name="data",
                                persistent_volume_claim=k8s.core.v1.PersistentVolumeClaimVolumeSourceArgs(
                                    claim_name="docker-registry"
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
            metadata=k8s.meta.v1.ObjectMetaArgs(name="docker-registry", namespace="skreg-infra"),
            spec=k8s.core.v1.ServiceSpecArgs(
                selector=labels,
                type="NodePort",
                ports=[
                    k8s.core.v1.ServicePortArgs(port=5000, target_port=5000, node_port=30500)
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[deploy]),
        )

        self._outputs = K8sRegistryOutputs(registry_url=self._registry_url)
        self.register_outputs({"registry_url": self._registry_url})

    @property
    def outputs(self) -> K8sRegistryOutputs:
        return self._outputs
