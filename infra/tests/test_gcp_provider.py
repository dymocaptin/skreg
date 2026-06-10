"""Pulumi-mock tests for the GCP provider components.

set_mocks must be called before any module under test is imported so that
Pulumi's resource registration is intercepted.
"""

import pulumi
from pulumi.runtime import Mocks


class GcpMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = {
            **args.inputs,
            "name": args.name,
            # Stub common output fields
            "endpoint": "10.0.0.1",
            "public_ip_address": "1.2.3.4",
            "access_id": "GOOGACCESSID",
            "secret": "mock-secret-value",
            "result": "mock-password",
            "email": "sa@project.iam.gserviceaccount.com",
        }
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(GcpMocks())

# Imports must come AFTER set_mocks
from skreg_infra.contracts import (  # noqa: E402
    DatabaseContract,
    DnsContract,
    ObjectStoreContract,
    SubstrateContract,
)
from skreg_infra.providers.gcp.database import CloudSqlDatabase  # noqa: E402
from skreg_infra.providers.gcp.dns import CloudDnsDns, _is_ip  # noqa: E402
from skreg_infra.providers.gcp.storage import GcsObjectStore  # noqa: E402
from skreg_infra.providers.gcp.substrate import GkeSubstrate  # noqa: E402


# ---------------------------------------------------------------------------
# GkeSubstrate
# ---------------------------------------------------------------------------


def test_gke_substrate_contract_type() -> None:
    substrate = GkeSubstrate("test-substrate", project="my-project")
    assert isinstance(substrate.contract, SubstrateContract)


def test_gke_substrate_contract_fields() -> None:
    substrate = GkeSubstrate("test-substrate-2", project="my-project")
    # kubeconfig is a static placeholder; LB IP only known after Traefik deploys
    assert substrate.contract.kubeconfig == ""
    assert substrate.contract.ingress_endpoint == ""


# ---------------------------------------------------------------------------
# CloudSqlDatabase
# ---------------------------------------------------------------------------


def test_cloud_sql_database_contract_type() -> None:
    db = CloudSqlDatabase("test-db", project="my-project")
    assert isinstance(db.contract, DatabaseContract)


def test_cloud_sql_database_contract_fields() -> None:
    db = CloudSqlDatabase("test-db-2", project="my-project")
    assert db.contract.dsn_secret_name == "skreg-db"
    assert db.contract.dsn_secret_key == "DATABASE_URL"


# ---------------------------------------------------------------------------
# GcsObjectStore
# ---------------------------------------------------------------------------


def test_gcs_object_store_contract_type() -> None:
    store = GcsObjectStore("test-store", project="my-project")
    assert isinstance(store.contract, ObjectStoreContract)


def test_gcs_object_store_contract_fields_defaults() -> None:
    store = GcsObjectStore("test-store-2", project="my-project")
    assert store.contract.endpoint == "https://storage.googleapis.com"
    assert store.contract.bucket == "skreg-packages"
    assert store.contract.credentials_secret_name == "skreg-minio"


def test_gcs_object_store_custom_bucket() -> None:
    store = GcsObjectStore("test-store-3", project="my-project", bucket_name="my-bucket")
    assert store.contract.bucket == "my-bucket"


# ---------------------------------------------------------------------------
# CloudDnsDns
# ---------------------------------------------------------------------------


def test_cloud_dns_contract_type() -> None:
    dns = CloudDnsDns(
        "test-dns",
        project="my-project",
        domain_name="example.com",
        managed_zone="example-zone",
        target="1.2.3.4",
    )
    assert isinstance(dns.contract, DnsContract)


def test_cloud_dns_contract_ip_target() -> None:
    dns = CloudDnsDns(
        "test-dns-ip",
        project="my-project",
        domain_name="example.com",
        managed_zone="example-zone",
        target="1.2.3.4",
    )
    assert dns.contract.ingress_endpoint == "1.2.3.4"


def test_cloud_dns_contract_hostname_target() -> None:
    dns = CloudDnsDns(
        "test-dns-cname",
        project="my-project",
        domain_name="example.com",
        managed_zone="example-zone",
        target="lb.example.net",
    )
    assert dns.contract.ingress_endpoint == "lb.example.net"


def test_is_ip_true() -> None:
    assert _is_ip("192.168.1.1") is True
    assert _is_ip("1.2.3.4") is True


def test_is_ip_false() -> None:
    assert _is_ip("lb.example.com") is False
    assert _is_ip("not-an-ip") is False
