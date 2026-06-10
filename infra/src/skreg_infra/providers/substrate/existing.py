"""Bring-your-own-cluster substrate: use the ambient kubeconfig.

The Pulumi Kubernetes provider reads the ambient kubeconfig, so this substrate
provisions nothing. It exists so dispatch is uniform across self-hosted and
managed clusters.
"""

from __future__ import annotations

from skreg_infra.contracts import SubstrateContract


class ExistingSubstrate:
    def __init__(self, ingress_endpoint: str = "") -> None:
        self._contract = SubstrateContract(kubeconfig="", ingress_endpoint=ingress_endpoint)

    @property
    def contract(self) -> SubstrateContract:
        return self._contract
