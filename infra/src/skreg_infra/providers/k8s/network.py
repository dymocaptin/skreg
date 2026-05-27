"""Traefik v3 ingress + ACME TLS for Kind."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs, RepositoryOptsArgs


class K8sNetworkOutputs:
    def __init__(self, traefik_release_name: pulumi.Output[str | None]) -> None:
        self.traefik_release_name = traefik_release_name


class K8sNetwork(pulumi.ComponentResource):
    """Installs Traefik v3 via Helm as a DaemonSet with hostNetwork for Kind."""

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Network", name, {}, opts)

        ns = k8s.core.v1.Namespace(
            f"{name}-ns",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-infra"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        skreg_ns = k8s.core.v1.Namespace(
            f"{name}-skreg-ns",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._traefik_release = Release(
            f"{name}-traefik",
            ReleaseArgs(
                chart="traefik",
                repository_opts=RepositoryOptsArgs(repo="https://helm.traefik.io/traefik"),
                version="32.1.1",
                namespace="skreg-infra",
                create_namespace=False,
                values={
                    "deployment": {"kind": "DaemonSet"},
                    "hostNetwork": True,
                    "dnsPolicy": "ClusterFirstWithHostNet",
                    "service": {"type": "ClusterIP"},
                    "ports": {
                        "web": {"hostPort": 80, "port": 80},
                        "websecure": {"hostPort": 443, "port": 443},
                    },
                    "tolerations": [
                        {
                            "key": "node-role.kubernetes.io/control-plane",
                            "operator": "Exists",
                            "effect": "NoSchedule",
                        }
                    ],
                    "nodeSelector": {"ingress-ready": "true"},
                    "additionalArguments": [
                        "--certificatesresolvers.letsencrypt.acme.email=peknudsen@gmail.com",
                        "--certificatesresolvers.letsencrypt.acme.storage=/data/acme.json",
                        "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web",
                    ],
                    "persistence": {"enabled": True, "size": "128Mi"},
                    "ingressClass": {"enabled": True, "isDefaultClass": True},
                },
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[ns, skreg_ns]),
        )

        self._outputs = K8sNetworkOutputs(
            traefik_release_name=self._traefik_release.name,
        )
        self.register_outputs({"traefik_release_name": self._outputs.traefik_release_name})

    @property
    def outputs(self) -> K8sNetworkOutputs:
        return self._outputs
