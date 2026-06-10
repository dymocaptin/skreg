"""Unit tests for the AWS provider components using Pulumi mocks.

IMPORTANT: ``pulumi.runtime.set_mocks`` MUST be called before importing any
module under test.  All component imports are therefore deferred to after the
mock installation at module level.
"""

import pulumi
from pulumi.runtime import Mocks


class AwsMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = {
            **args.inputs,
            "name": args.name,
            # RDS
            "endpoint": "db.example.rds.amazonaws.com:5432",
            # IAM access key
            "id": "AKIAIOSFODNN7EXAMPLE",
            "secret": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            # EKS
            "identities": [{"oidcs": [{"issuer": "https://oidc.example.com"}]}],
            "certificate_authorities": [{"data": "dGVzdA=="}],
            "endpoint": "https://eks.example.com",
            # random password
            "result": "supersecretpassword",
            # OIDC provider
            "arn": "arn:aws:iam::123456789012:oidc-provider/oidc.example.com",
            "url": "https://oidc.example.com",
        }
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(AwsMocks())

# Deferred imports — after set_mocks
from skreg_infra.contracts import (  # noqa: E402
    DatabaseContract,
    DnsContract,
    ObjectStoreContract,
    SubstrateContract,
)
from skreg_infra.providers.aws.database import RdsDatabase  # noqa: E402
from skreg_infra.providers.aws.dns import Route53Dns, _is_ip  # noqa: E402
from skreg_infra.providers.aws.storage import S3ObjectStore  # noqa: E402
from skreg_infra.providers.aws.substrate import EksSubstrate  # noqa: E402


# ── _is_ip helper ─────────────────────────────────────────────────────────────


def test_is_ip_returns_true_for_ipv4() -> None:
    assert _is_ip("1.2.3.4") is True


def test_is_ip_returns_true_for_ipv6() -> None:
    assert _is_ip("::1") is True


def test_is_ip_returns_false_for_hostname() -> None:
    assert _is_ip("abc-123.elb.us-west-2.amazonaws.com") is False


def test_is_ip_returns_false_for_domain() -> None:
    assert _is_ip("skreg.ai") is False


# ── EksSubstrate ──────────────────────────────────────────────────────────────


def test_eks_substrate_instantiates() -> None:
    sub = EksSubstrate("test-substrate")
    assert sub is not None


def test_eks_substrate_exposes_contract() -> None:
    sub = EksSubstrate("test-substrate-2")
    assert isinstance(sub.contract, SubstrateContract)


def test_eks_substrate_contract_kubeconfig_sentinel() -> None:
    sub = EksSubstrate("test-substrate-3")
    # kubeconfig is "" in the contract; actual value is in sub.kubeconfig Output
    assert sub.contract.kubeconfig == ""


def test_eks_substrate_contract_ingress_sentinel() -> None:
    sub = EksSubstrate("test-substrate-4")
    assert sub.contract.ingress_endpoint == ""


def test_eks_substrate_exposes_kubeconfig_output() -> None:
    sub = EksSubstrate("test-substrate-5")
    # Must expose the kubeconfig as a Pulumi Output (not None)
    assert sub.kubeconfig is not None


# ── RdsDatabase ───────────────────────────────────────────────────────────────


def test_rds_database_instantiates_with_sg() -> None:
    db = RdsDatabase(
        "test-db",
        vpc_id="vpc-12345",
        subnet_ids=["subnet-aaa", "subnet-bbb"],
        source_sg_id="sg-nodes",
    )
    assert db is not None


def test_rds_database_instantiates_with_cidr() -> None:
    db = RdsDatabase(
        "test-db-cidr",
        vpc_id="vpc-12345",
        subnet_ids=["subnet-aaa", "subnet-bbb"],
        source_cidr="10.0.0.0/8",
    )
    assert db is not None


def test_rds_database_exposes_contract() -> None:
    db = RdsDatabase(
        "test-db-contract",
        vpc_id="vpc-12345",
        subnet_ids=["subnet-aaa", "subnet-bbb"],
        source_sg_id="sg-nodes",
    )
    assert isinstance(db.contract, DatabaseContract)


def test_rds_database_contract_secret_name() -> None:
    db = RdsDatabase(
        "test-db-secret",
        vpc_id="vpc-12345",
        subnet_ids=["subnet-aaa", "subnet-bbb"],
        source_sg_id="sg-nodes",
    )
    assert db.contract.dsn_secret_name == "skreg-db"


def test_rds_database_contract_secret_key() -> None:
    db = RdsDatabase(
        "test-db-key",
        vpc_id="vpc-12345",
        subnet_ids=["subnet-aaa", "subnet-bbb"],
        source_sg_id="sg-nodes",
    )
    assert db.contract.dsn_secret_key == "DATABASE_URL"


def test_rds_database_raises_without_source() -> None:
    import pytest

    with pytest.raises(ValueError):
        RdsDatabase(
            "test-db-nosg",
            vpc_id="vpc-12345",
            subnet_ids=["subnet-aaa", "subnet-bbb"],
        )


# ── S3ObjectStore ─────────────────────────────────────────────────────────────


def test_s3_object_store_instantiates() -> None:
    store = S3ObjectStore("test-store")
    assert store is not None


def test_s3_object_store_exposes_contract() -> None:
    store = S3ObjectStore("test-store-2")
    assert isinstance(store.contract, ObjectStoreContract)


def test_s3_object_store_contract_bucket() -> None:
    store = S3ObjectStore("test-store-3")
    assert store.contract.bucket == "skreg-packages"


def test_s3_object_store_contract_endpoint() -> None:
    store = S3ObjectStore("test-store-4")
    assert store.contract.endpoint == "https://s3.us-west-2.amazonaws.com"


def test_s3_object_store_contract_credentials_secret() -> None:
    store = S3ObjectStore("test-store-5")
    assert store.contract.credentials_secret_name == "skreg-minio"


# ── Route53Dns ────────────────────────────────────────────────────────────────


def test_route53_dns_instantiates_with_hostname() -> None:
    dns = Route53Dns(
        "test-dns",
        domain_name="skreg.ai",
        hosted_zone_id="Z1234567890",
        target="abc.elb.amazonaws.com",
    )
    assert dns is not None


def test_route53_dns_instantiates_with_ip() -> None:
    dns = Route53Dns(
        "test-dns-ip",
        domain_name="skreg.ai",
        hosted_zone_id="Z1234567890",
        target="1.2.3.4",
    )
    assert dns is not None


def test_route53_dns_exposes_contract() -> None:
    dns = Route53Dns(
        "test-dns-contract",
        domain_name="skreg.ai",
        hosted_zone_id="Z1234567890",
        target="abc.elb.amazonaws.com",
    )
    assert isinstance(dns.contract, DnsContract)


def test_route53_dns_contract_ingress_endpoint() -> None:
    target = "abc.elb.amazonaws.com"
    dns = Route53Dns(
        "test-dns-endpoint",
        domain_name="skreg.ai",
        hosted_zone_id="Z1234567890",
        target=target,
    )
    assert dns.contract.ingress_endpoint == target


def test_route53_dns_contract_ingress_endpoint_ip() -> None:
    target = "1.2.3.4"
    dns = Route53Dns(
        "test-dns-endpoint-ip",
        domain_name="skreg.ai",
        hosted_zone_id="Z1234567890",
        target=target,
    )
    assert dns.contract.ingress_endpoint == target
