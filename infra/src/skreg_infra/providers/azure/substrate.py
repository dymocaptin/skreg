"""AKS-based substrate: managed Kubernetes cluster on Azure.

Provisions a resource group and an AKS managed cluster with a single system
node pool (Standard_B2s VMs). The kubeconfig is exported as a Pulumi stack
output and must be retrieved via ``pulumi stack output kubeconfig --show-secrets``
after the first deploy. The ingress endpoint (LoadBalancer IP assigned to
Traefik) is not known until Traefik has been deployed into the cluster; the
contract therefore carries an empty string for ``ingress_endpoint`` at provision
time and must be updated manually once the IP is available.
"""

from __future__ import annotations

import pulumi
import pulumi_azure_native.containerservice as containerservice
import pulumi_azure_native.resources as resources

from skreg_infra.contracts import SubstrateContract

_NODE_VM_SIZE = "Standard_B2s"
_NODE_COUNT = 1


class AksSubstrate(pulumi.ComponentResource):
    """AKS managed cluster + resource group.

    The ``contract.kubeconfig`` field is always an empty string at plan time.
    The real kubeconfig is accessible via the ``kubeconfig`` Pulumi stack output
    after ``pulumi up``.

    The ``contract.ingress_endpoint`` field is an empty string until Traefik
    (or another ingress controller) has been deployed and its LoadBalancer IP
    has been assigned by Azure.
    """

    def __init__(
        self,
        name: str,
        location: str = "eastus",
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:azure:AksSubstrate", name, {}, opts)

        rg = resources.ResourceGroup(
            f"{name}-rg",
            resource_group_name=f"{name}-rg",
            location=location,
            opts=pulumi.ResourceOptions(parent=self),
        )

        cluster = containerservice.ManagedCluster(
            f"{name}-aks",
            resource_group_name=rg.name,
            location=rg.location,
            dns_prefix=name,
            agent_pool_profiles=[
                containerservice.ManagedClusterAgentPoolProfileArgs(
                    name="system",
                    count=_NODE_COUNT,
                    vm_size=_NODE_VM_SIZE,
                    mode=containerservice.AgentPoolMode.SYSTEM,
                    os_disk_size_gb=30,
                )
            ],
            identity=containerservice.ManagedClusterIdentityArgs(
                type=containerservice.ResourceIdentityType.SYSTEM_ASSIGNED,
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[rg]),
        )

        # Export kubeconfig as a stack output so operators can retrieve it.
        pulumi.export(
            "kubeconfig",
            pulumi.Output.all(rg.name, cluster.name).apply(
                lambda args: (
                    "<retrieve via: az aks get-credentials"
                    f" --resource-group {args[0]} --name {args[1]}>"
                )
            ),
        )

        self._contract = SubstrateContract(
            kubeconfig="",
            ingress_endpoint="",
        )
        self.register_outputs({})

    @property
    def contract(self) -> SubstrateContract:
        return self._contract
