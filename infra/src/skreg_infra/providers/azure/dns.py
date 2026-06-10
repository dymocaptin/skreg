"""Azure DNS zone management for skreg.

Creates an apex A record and an ``api.`` A or CNAME record in an existing
Azure DNS zone. If ``target`` looks like an IPv4 address, both records are
created as A records. Otherwise the apex gets an A record with the resolved
address and ``api.`` gets a CNAME pointing at ``target``.

In practice the target is the Traefik LoadBalancer IP, which is only known
after the AKS cluster and ingress controller are deployed.
"""

from __future__ import annotations

import re

import pulumi
import pulumi_azure_native.network as network

from skreg_infra.contracts import DnsContract

_IPV4_RE = re.compile(r"^\d{1,3}(?:\.\d{1,3}){3}$")


def _is_ip(value: str) -> bool:
    """Return True if *value* looks like a dotted-decimal IPv4 address."""
    return bool(_IPV4_RE.match(value))


class AzureDns(pulumi.ComponentResource):
    """Azure DNS RecordSets for apex and ``api.`` sub-domain.

    Both ``@`` (apex) and ``api.`` are pointed at *target*. When *target* is
    an IP address, both are A records. When *target* is a hostname, the apex
    is an A record (requires resolution outside Pulumi) and ``api.`` is a CNAME.
    """

    def __init__(
        self,
        name: str,
        domain_name: str,
        resource_group: str,
        zone_name: str,
        target: str,
        ttl: int = 300,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:azure:Dns", name, {}, opts)

        if _is_ip(target):
            network.RecordSet(
                f"{name}-apex",
                resource_group_name=resource_group,
                zone_name=zone_name,
                relative_record_set_name="@",
                record_type="A",
                ttl=ttl,
                a_records=[network.ARecordArgs(ipv4_address=target)],
                opts=pulumi.ResourceOptions(parent=self),
            )

            network.RecordSet(
                f"{name}-api",
                resource_group_name=resource_group,
                zone_name=zone_name,
                relative_record_set_name="api",
                record_type="A",
                ttl=ttl,
                a_records=[network.ARecordArgs(ipv4_address=target)],
                opts=pulumi.ResourceOptions(parent=self),
            )
        else:
            # target is a hostname — apex A record cannot be a CNAME per DNS spec.
            # The operator must resolve the IP manually for the apex record.
            network.RecordSet(
                f"{name}-apex",
                resource_group_name=resource_group,
                zone_name=zone_name,
                relative_record_set_name="@",
                record_type="A",
                ttl=ttl,
                a_records=[network.ARecordArgs(ipv4_address="0.0.0.0")],  # noqa: S104
                opts=pulumi.ResourceOptions(parent=self),
            )

            network.RecordSet(
                f"{name}-api",
                resource_group_name=resource_group,
                zone_name=zone_name,
                relative_record_set_name="api",
                record_type="CNAME",
                ttl=ttl,
                cname_record=network.CnameRecordArgs(cname=target),
                opts=pulumi.ResourceOptions(parent=self),
            )

        self._contract = DnsContract(ingress_endpoint=target)
        self.register_outputs({"ingress_endpoint": target, "domain_name": domain_name})

    @property
    def contract(self) -> DnsContract:
        return self._contract
