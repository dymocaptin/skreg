"""Tests for the Azure provider components.

Each component is exercised in isolation using Pulumi's mock runtime. The
set_mocks() call MUST precede any import of the modules under test.
"""

import pulumi
from pulumi.runtime import Mocks


class AzureMocks(Mocks):
    def new_resource(self, args: pulumi.runtime.MockResourceArgs) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = {**args.inputs, "name": args.name}
        # Provide realistic-looking outputs for fields the modules use.
        if args.typ == "azure-native:resources:ResourceGroup":
            outputs["location"] = args.inputs.get("location", "eastus")
            outputs["name"] = args.inputs.get("resourceGroupName", args.name)
        if args.typ == "azure-native:containerservice:ManagedCluster":
            outputs["name"] = args.inputs.get("dnsPrefix", args.name)
        if args.typ == "azure-native:dbforpostgresql:Server":
            outputs["fullyQualifiedDomainName"] = "skreg-server.postgres.database.azure.com"
            outputs["name"] = args.name
        if args.typ == "azure-native:storage:StorageAccount":
            outputs["name"] = args.inputs.get("accountName", "skregpackages")
            outputs["primaryEndpoints"] = {
                "blob": "https://skregpackages.blob.core.windows.net/"
            }
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        if args.token == "azure-native:storage:listStorageAccountKeys":
            return (
                {
                    "keys": [
                        {"keyName": "key1", "value": "fake-account-key", "permissions": "Full"}
                    ]
                },
                [],
            )
        return ({}, [])


pulumi.runtime.set_mocks(AzureMocks())

# Imports must come AFTER set_mocks.
from skreg_infra.contracts import (  # noqa: E402
    DatabaseContract,
    DnsContract,
    ObjectStoreContract,
    SubstrateContract,
)
from skreg_infra.providers.azure.database import AzurePgDatabase  # noqa: E402
from skreg_infra.providers.azure.dns import AzureDns, _is_ip  # noqa: E402
from skreg_infra.providers.azure.storage import BlobObjectStore  # noqa: E402
from skreg_infra.providers.azure.substrate import AksSubstrate  # noqa: E402


# ---------------------------------------------------------------------------
# AksSubstrate
# ---------------------------------------------------------------------------


def test_aks_substrate_contract_type() -> None:
    sub = AksSubstrate("test-aks", location="eastus")
    assert isinstance(sub.contract, SubstrateContract)


def test_aks_substrate_contract_fields() -> None:
    sub = AksSubstrate("test-aks-2", location="eastus")
    # kubeconfig and ingress_endpoint are empty strings at plan time.
    assert sub.contract.kubeconfig == ""
    assert sub.contract.ingress_endpoint == ""


# ---------------------------------------------------------------------------
# AzurePgDatabase
# ---------------------------------------------------------------------------


def test_pg_database_contract_type() -> None:
    db = AzurePgDatabase("test-db", resource_group_name="test-rg")
    assert isinstance(db.contract, DatabaseContract)


def test_pg_database_contract_fields() -> None:
    db = AzurePgDatabase("test-db-2", resource_group_name="test-rg")
    assert db.contract.dsn_secret_name == "skreg-db"
    assert db.contract.dsn_secret_key == "DATABASE_URL"


# ---------------------------------------------------------------------------
# BlobObjectStore
# ---------------------------------------------------------------------------


def test_blob_store_contract_type() -> None:
    store = BlobObjectStore("test-store", resource_group_name="test-rg")
    assert isinstance(store.contract, ObjectStoreContract)


def test_blob_store_contract_fields() -> None:
    store = BlobObjectStore("test-store-2", resource_group_name="test-rg")
    assert store.contract.bucket == "skreg-packages"
    assert store.contract.credentials_secret_name == "skreg-minio"
    assert store.contract.endpoint == "https://skregpackages.blob.core.windows.net"


def test_blob_store_custom_account_name() -> None:
    store = BlobObjectStore(
        "test-store-3",
        resource_group_name="test-rg",
        account_name="myaccount",
    )
    assert store.contract.endpoint == "https://myaccount.blob.core.windows.net"


# ---------------------------------------------------------------------------
# AzureDns
# ---------------------------------------------------------------------------


def test_is_ip_with_valid_ip() -> None:
    assert _is_ip("1.2.3.4") is True
    assert _is_ip("203.0.113.10") is True


def test_is_ip_with_hostname() -> None:
    assert _is_ip("example.com") is False
    assert _is_ip("traefik.example.com") is False


def test_dns_contract_type_with_ip() -> None:
    dns = AzureDns(
        "test-dns",
        domain_name="example.com",
        resource_group="test-rg",
        zone_name="example.com",
        target="1.2.3.4",
    )
    assert isinstance(dns.contract, DnsContract)


def test_dns_contract_ingress_endpoint_ip() -> None:
    dns = AzureDns(
        "test-dns-2",
        domain_name="example.com",
        resource_group="test-rg",
        zone_name="example.com",
        target="203.0.113.5",
    )
    assert dns.contract.ingress_endpoint == "203.0.113.5"


def test_dns_contract_ingress_endpoint_hostname() -> None:
    dns = AzureDns(
        "test-dns-3",
        domain_name="example.com",
        resource_group="test-rg",
        zone_name="example.com",
        target="lb.example.net",
    )
    assert dns.contract.ingress_endpoint == "lb.example.net"
