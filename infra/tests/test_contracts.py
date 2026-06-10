from skreg_infra.contracts import (
    DatabaseContract,
    DnsContract,
    ObjectStoreContract,
    SubstrateContract,
)


def test_database_contract_holds_dsn_and_secret() -> None:
    c = DatabaseContract(dsn_secret_name="skreg-db", dsn_secret_key="DATABASE_URL")
    assert c.dsn_secret_name == "skreg-db"
    assert c.dsn_secret_key == "DATABASE_URL"


def test_object_store_contract_fields() -> None:
    c = ObjectStoreContract(
        endpoint="http://minio:9000", bucket="skreg", credentials_secret_name="skreg-minio"
    )
    assert c.endpoint == "http://minio:9000"
    assert c.bucket == "skreg"
    assert c.credentials_secret_name == "skreg-minio"


def test_dns_contract_target() -> None:
    assert DnsContract(ingress_endpoint="50.53.217.195").ingress_endpoint == "50.53.217.195"


def test_substrate_contract_fields() -> None:
    c = SubstrateContract(kubeconfig="", ingress_endpoint="")
    assert c.kubeconfig == ""
    assert c.ingress_endpoint == ""
