"""Verify that component Protocol interfaces are importable and well-typed."""
from __future__ import annotations

import pulumi

from skillpkg_infra.components.compute import ComputeOutputs, SkillpkgCompute
from skillpkg_infra.components.database import DatabaseOutputs, SkillpkgDatabase
from skillpkg_infra.components.network import NetworkOutputs, SkillpkgNetwork
from skillpkg_infra.components.pki import PkiOutputs, SkillpkgPki
from skillpkg_infra.components.storage import SkillpkgStorage, StorageOutputs


def test_database_protocol_is_importable() -> None:
    assert SkillpkgDatabase is not None
    assert DatabaseOutputs is not None


def test_storage_protocol_is_importable() -> None:
    assert SkillpkgStorage is not None
    assert StorageOutputs is not None


def test_pki_protocol_is_importable() -> None:
    assert SkillpkgPki is not None
    assert PkiOutputs is not None


def test_database_outputs_constructible() -> None:
    outputs = DatabaseOutputs(
        connection_secret_name=pulumi.Output.from_input("secret"),
        connection_secret_arn=pulumi.Output.from_input("arn:aws:secretsmanager:us-west-2:123456789:secret:skreg"),
        host=pulumi.Output.from_input("localhost"),
        port=pulumi.Output.from_input(5432),
        database_name=pulumi.Output.from_input("skreg"),
    )
    assert outputs is not None


def test_storage_outputs_constructible() -> None:
    outputs = StorageOutputs(
        bucket_name=pulumi.Output.from_input("skreg-packages"),
        cdn_base_url=pulumi.Output.from_input("https://cdn.example.com"),
        service_account_secret_name=pulumi.Output.from_input("sa-secret"),
    )
    assert outputs is not None


def test_pki_outputs_constructible() -> None:
    outputs = PkiOutputs(
        hsm_key_id=pulumi.Output.from_input("key-id"),
        intermediate_ca_cert_secret_name=pulumi.Output.from_input("ca-secret"),
        crl_bucket_path=pulumi.Output.from_input("s3://bucket/crl.pem"),
        hsm_backend="software",
    )
    assert outputs is not None


def test_compute_outputs_constructible() -> None:
    outputs = ComputeOutputs(
        service_url=pulumi.Output.from_input("https://api.example.com"),
        worker_service_name=pulumi.Output.from_input("skreg-worker"),
    )
    assert outputs is not None


def test_compute_outputs_includes_alb_dns_name() -> None:
    outputs = ComputeOutputs(
        service_url=pulumi.Output.from_input("https://api.example.com"),
        worker_service_name=pulumi.Output.from_input("skreg-worker"),
        alb_dns_name=pulumi.Output.from_input("skreg-alb-123.us-west-2.elb.amazonaws.com"),
    )
    assert outputs.alb_dns_name is not None


def test_compute_outputs_cert_validation_cname_defaults_to_none() -> None:
    outputs = ComputeOutputs(
        service_url=pulumi.Output.from_input("https://api.example.com"),
        worker_service_name=pulumi.Output.from_input("skreg-worker"),
    )
    assert outputs.cert_validation_cname is None


def test_network_outputs_constructible() -> None:
    outputs = NetworkOutputs(
        vpc_id=pulumi.Output.from_input("vpc-123"),
        public_subnet_ids=[
            pulumi.Output.from_input("subnet-pub-1"),
            pulumi.Output.from_input("subnet-pub-2"),
        ],
        private_subnet_ids=[
            pulumi.Output.from_input("subnet-priv-1"),
            pulumi.Output.from_input("subnet-priv-2"),
        ],
    )
    assert outputs is not None
    assert len(outputs.public_subnet_ids) == 2
    assert len(outputs.private_subnet_ids) == 2
