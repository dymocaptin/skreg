"""Tests for CloudStack provider dispatch."""

from __future__ import annotations

import pulumi
import pytest
from pulumi.runtime import Mocks


class DispatchMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = {**args.inputs, "name": args.name}
        if args.typ == "aws:eks/cluster:Cluster":
            outputs.setdefault("endpoint", "https://eks.example.com")
            outputs.setdefault(
                "certificate_authorities", [{"data": "ZHVtbXk="}]
            )
            outputs.setdefault("identities", [{"oidcs": [{"issuer": "https://oidc"}]}])
        if args.typ == "aws:rds/instance:Instance":
            outputs.setdefault("endpoint", "db.example.com:5432")
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(DispatchMocks())

from skreg_infra.config import (  # noqa: E402
    CloudProvider,
    DatabaseBackend,
    DnsBackend,
    StackConfig,
    StorageBackend,
)
from skreg_infra.dispatch import CloudStack  # noqa: E402


def _make_config(provider: CloudProvider, **kwargs: object) -> StackConfig:
    return StackConfig(
        cloud_provider=provider,
        api_image_uri="localhost:30500/skreg-api:latest",
        worker_image_uri="localhost:30500/skreg-worker:latest",
        domain_name="skreg.ai",
        from_email="noreply@skreg.ai",
        **kwargs,  # type: ignore[arg-type]
    )


def test_k8s_dispatch_runs_k8s_stack() -> None:
    """The k8s provider routes to the existing in-cluster stack."""
    CloudStack(_make_config(CloudProvider.K8S)).run()  # should not raise


def test_aws_dispatch_with_managed_backends() -> None:
    CloudStack(
        _make_config(
            CloudProvider.AWS,
            database_backend=DatabaseBackend.MANAGED,
            storage_backend=StorageBackend.MANAGED,
            dns_backend=DnsBackend.MANAGED,
            hosted_zone_id="Z123",
            ingress_endpoint="nlb.example.com",
        )
    ).run()


def test_aws_dispatch_substrate_only() -> None:
    """In-cluster backends mean only the substrate is provisioned."""
    CloudStack(_make_config(CloudProvider.AWS)).run()


def test_gcp_dispatch_with_managed_backends() -> None:
    CloudStack(
        _make_config(
            CloudProvider.GCP,
            gcp_project="skreg-test",
            database_backend=DatabaseBackend.MANAGED,
            storage_backend=StorageBackend.MANAGED,
            dns_backend=DnsBackend.MANAGED,
            gcp_managed_zone="skreg-zone",
            ingress_endpoint="203.0.113.10",
        )
    ).run()


def test_gcp_dispatch_requires_project() -> None:
    with pytest.raises(ValueError, match="SKREG_GCP_PROJECT"):
        CloudStack(_make_config(CloudProvider.GCP)).run()


def test_gcp_dispatch_managed_dns_requires_zone() -> None:
    with pytest.raises(ValueError, match="SKREG_GCP_MANAGED_ZONE"):
        CloudStack(
            _make_config(
                CloudProvider.GCP,
                gcp_project="skreg-test",
                dns_backend=DnsBackend.MANAGED,
                ingress_endpoint="203.0.113.10",
            )
        ).run()


def test_azure_dispatch_with_managed_backends() -> None:
    CloudStack(
        _make_config(
            CloudProvider.AZURE,
            database_backend=DatabaseBackend.MANAGED,
            storage_backend=StorageBackend.MANAGED,
            dns_backend=DnsBackend.MANAGED,
            azure_dns_zone="skreg.ai",
            ingress_endpoint="203.0.113.10",
        )
    ).run()


def test_azure_dispatch_managed_dns_requires_zone() -> None:
    with pytest.raises(ValueError, match="SKREG_AZURE_DNS_ZONE"):
        CloudStack(
            _make_config(
                CloudProvider.AZURE,
                dns_backend=DnsBackend.MANAGED,
                ingress_endpoint="203.0.113.10",
            )
        ).run()


def test_managed_dns_skipped_without_ingress_endpoint() -> None:
    """A first deploy has no LoadBalancer endpoint yet; DNS must be skipped."""
    CloudStack(
        _make_config(
            CloudProvider.AWS,
            dns_backend=DnsBackend.MANAGED,
            hosted_zone_id="Z123",
        )
    ).run()
