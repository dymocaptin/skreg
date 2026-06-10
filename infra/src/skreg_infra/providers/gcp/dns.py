"""Cloud DNS record set management.

Creates apex A / CNAME and api. subdomain records in an existing Cloud DNS
managed zone. If the target looks like an IP address an A record is created;
otherwise a CNAME record is used.
"""

from __future__ import annotations

import re

import pulumi
import pulumi_gcp as gcp

from skreg_infra.contracts import DnsContract

_IP_RE = re.compile(r"^\d{1,3}(\.\d{1,3}){3}$")


def _is_ip(value: str) -> bool:
    return bool(_IP_RE.match(value))


class CloudDnsDns(pulumi.ComponentResource):
    """Cloud DNS A/CNAME records for apex and api. subdomains."""

    def __init__(
        self,
        name: str,
        project: str,
        domain_name: str,
        managed_zone: str,
        target: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:gcp:CloudDnsDns", name, {}, opts)

        # Ensure domain_name ends with a trailing dot (DNS absolute form)
        apex = domain_name if domain_name.endswith(".") else f"{domain_name}."
        api_fqdn = f"api.{apex}"

        if _is_ip(target):
            record_type = "A"
            rrdatas = [target]
        else:
            record_type = "CNAME"
            cname_target = target if target.endswith(".") else f"{target}."
            rrdatas = [cname_target]

        gcp.dns.RecordSet(
            f"{name}-apex",
            project=project,
            managed_zone=managed_zone,
            name=apex,
            type=record_type,
            ttl=300,
            rrdatas=rrdatas,
            opts=pulumi.ResourceOptions(parent=self),
        )

        gcp.dns.RecordSet(
            f"{name}-api",
            project=project,
            managed_zone=managed_zone,
            name=api_fqdn,
            type=record_type,
            ttl=300,
            rrdatas=rrdatas,
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._contract = DnsContract(ingress_endpoint=target)

        self.register_outputs({"ingress_endpoint": target})

    @property
    def contract(self) -> DnsContract:
        return self._contract
