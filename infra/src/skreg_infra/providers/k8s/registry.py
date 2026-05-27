"""Docker registry v2 in-cluster, exposed via NodePort 30500."""
from __future__ import annotations

import pulumi
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs, RepositoryOptsArgs


class K8sRegistryOutputs:
    def __init__(self, registry_url: str) -> None:
        self.registry_url = registry_url


class K8sRegistry(pulumi.ComponentResource):
    """Docker registry v2, NodePort 30500 on the Kind host."""

    REGISTRY_URL = "localhost:30500"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Registry", name, {}, opts)

        self._registry_url = self.REGISTRY_URL

        Release(
            f"{name}-registry",
            ReleaseArgs(
                chart="docker-registry",
                repository_opts=RepositoryOptsArgs(repo="https://helm.twun.io"),
                version="2.2.3",
                namespace="skreg-infra",
                create_namespace=False,
                values={
                    "service": {"type": "NodePort", "nodePort": 30500},
                    "persistence": {"enabled": True, "size": "20Gi"},
                    "resources": {
                        "requests": {"cpu": "50m", "memory": "64Mi"},
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs = K8sRegistryOutputs(registry_url=self._registry_url)
        self.register_outputs({"registry_url": self._registry_url})

    @property
    def outputs(self) -> K8sRegistryOutputs:
        return self._outputs
