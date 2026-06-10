"""GKE substrate: provisions a GKE cluster and exposes a SubstrateContract.

The kubeconfig is exported as a Pulumi output and resolved only after the
cluster is ready. The ingress_endpoint (LoadBalancer IP) is known only after
Traefik (or another ingress controller) deploys into the cluster, so the
contract returns an empty string as a placeholder.
"""

from __future__ import annotations

import pulumi
import pulumi_gcp as gcp

from skreg_infra.contracts import SubstrateContract

_DEFAULT_LOCATION = "us-central1"
_DEFAULT_NODE_COUNT = 1
_DEFAULT_MACHINE_TYPE = "e2-standard-2"


class GkeSubstrate(pulumi.ComponentResource):
    """GKE standard cluster with a single node pool."""

    def __init__(
        self,
        name: str,
        project: str,
        location: str = _DEFAULT_LOCATION,
        node_count: int = _DEFAULT_NODE_COUNT,
        machine_type: str = _DEFAULT_MACHINE_TYPE,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:gcp:GkeSubstrate", name, {}, opts)

        self._cluster = gcp.container.Cluster(
            f"{name}-cluster",
            project=project,
            location=location,
            # Remove the default node pool and manage it separately so we can
            # configure it explicitly.
            remove_default_node_pool=True,
            initial_node_count=1,
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._node_pool = gcp.container.NodePool(
            f"{name}-nodepool",
            project=project,
            location=location,
            cluster=self._cluster.name,
            node_count=node_count,
            node_config=gcp.container.NodePoolNodeConfigArgs(
                machine_type=machine_type,
                oauth_scopes=[
                    "https://www.googleapis.com/auth/cloud-platform",
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[self._cluster]),
        )

        # The kubeconfig is only resolvable as a Pulumi Output[str]; the
        # contract field holds an empty string as a static placeholder.
        # Callers that need the live kubeconfig should read the Pulumi stack
        # output directly.
        self._contract = SubstrateContract(
            kubeconfig="",
            ingress_endpoint="",
        )

        self.register_outputs(
            {
                "cluster_name": self._cluster.name,
                "cluster_endpoint": self._cluster.endpoint,
            }
        )

    @property
    def cluster(self) -> gcp.container.Cluster:
        return self._cluster

    @property
    def node_pool(self) -> gcp.container.NodePool:
        return self._node_pool

    @property
    def contract(self) -> SubstrateContract:
        return self._contract
