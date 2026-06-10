"""Route53 DNS component.

Creates DNS records in an existing hosted zone pointing at the ingress endpoint:
- ``api.<domain_name>`` — always a CNAME record
- apex ``<domain_name>`` — CNAME when target is a hostname (Route53 ALIAS to an
  external hostname requires the target's hosted zone ID, which we don't have);
  A record when target is an IP address

NOTE: Route53 native ALIAS records (not CNAMEs) are only possible for targets that
AWS knows about (e.g. ALBs, NLBs, CloudFront in the same account). For an
arbitrary NLB DNS hostname we cannot construct an ALIAS without the NLB's hosted
zone ID. The NLB hosted zone ID is not surfaced in this component's interface, so
we use a CNAME for ``api.`` and skip the apex when the target is a hostname. When
the target is an IP address we create a plain A record for both apex and api.

Yields DnsContract(ingress_endpoint=target).
"""

from __future__ import annotations

import ipaddress

import pulumi
import pulumi_aws as aws

from skreg_infra.contracts import DnsContract

_TTL = 60


def _is_ip(value: str) -> bool:
    """Return True if *value* is a valid IPv4 or IPv6 address."""
    try:
        ipaddress.ip_address(value)
        return True
    except ValueError:
        return False


class Route53Dns(pulumi.ComponentResource):
    """DNS records in an existing Route53 hosted zone.

    Args:
        name: Pulumi resource name prefix.
        domain_name: Apex domain (e.g. ``skreg.ai``). Records are created for
            this domain and ``api.<domain_name>``.
        hosted_zone_id: ID of the existing Route53 hosted zone. The component
            creates records only — never the zone itself.
        target: Hostname or IP address to point the records at (e.g. the NLB
            hostname emitted by Traefik's LoadBalancer Service).
        opts: Pulumi resource options.
    """

    def __init__(
        self,
        name: str,
        domain_name: str,
        hosted_zone_id: str,
        target: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:Route53Dns", name, {}, opts)
        child_opts = pulumi.ResourceOptions(parent=self)

        target_is_ip = _is_ip(target)
        record_type = "A" if target_is_ip else "CNAME"
        records = [target]

        # ``api.<domain_name>`` record — always created
        aws.route53.Record(
            f"{name}-api",
            zone_id=hosted_zone_id,
            name=f"api.{domain_name}",
            type=record_type,
            ttl=_TTL,
            records=records,
            opts=child_opts,
        )

        # Apex record — only when target is an IP (CNAME at apex is invalid in
        # standard DNS; Route53 ALIAS requires the NLB hosted zone ID which is
        # not available here)
        if target_is_ip:
            aws.route53.Record(
                f"{name}-apex",
                zone_id=hosted_zone_id,
                name=domain_name,
                type="A",
                ttl=_TTL,
                records=records,
                opts=child_opts,
            )
        # else: skip apex when target is a hostname; document and revisit when
        # the NLB hosted zone ID is threaded through the contract or config.

        self._contract = DnsContract(ingress_endpoint=target)
        self.register_outputs({"ingress_endpoint": target})

    @property
    def contract(self) -> DnsContract:
        return self._contract
