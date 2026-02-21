"""Verify that component Protocol interfaces are importable and well-typed."""
from __future__ import annotations

import pulumi

from skillpkg_infra.components.compute import ComputeOutputs, SkillpkgCompute
from skillpkg_infra.components.database import DatabaseOutputs, SkillpkgDatabase
from skillpkg_infra.components.network import NetworkOutputs, SkillpkgNetwork
from skillpkg_infra.components.pki import PkiOutputs, SkillpkgPki
from skillpkg_infra.components.storage import SkillpkgStorage, StorageOutputs


def test_database_protocol_is_importable() -> None:
    """DatabaseOutputs and SkillpkgDatabase must be importable."""
    assert SkillpkgDatabase is not None
    assert DatabaseOutputs is not None


def test_storage_protocol_is_importable() -> None:
    """StorageOutputs and SkillpkgStorage must be importable."""
    assert SkillpkgStorage is not None
    assert StorageOutputs is not None


def test_pki_protocol_is_importable() -> None:
    """PkiOutputs and SkillpkgPki must be importable."""
    assert SkillpkgPki is not None
    assert PkiOutputs is not None


def test_database_outputs_constructible() -> None:
    """DatabaseOutputs must accept pulumi.Output arguments."""
    outputs = DatabaseOutputs(
        connection_secret_name=pulumi.Output.from_input("secret"),
        host=pulumi.Output.from_input("localhost"),
        port=pulumi.Output.from_input(5432),
        database_name=pulumi.Output.from_input("skreg"),
    )
    assert outputs is not None


def test_storage_outputs_constructible() -> None:
    """StorageOutputs must accept pulumi.Output arguments."""
    outputs = StorageOutputs(
        bucket_name=pulumi.Output.from_input("skreg-packages"),
        cdn_base_url=pulumi.Output.from_input("https://cdn.example.com"),
        service_account_secret_name=pulumi.Output.from_input("sa-secret"),
    )
    assert outputs is not None


def test_pki_outputs_constructible() -> None:
    """PkiOutputs must accept pulumi.Output arguments."""
    outputs = PkiOutputs(
        hsm_key_id=pulumi.Output.from_input("key-id"),
        intermediate_ca_cert_secret_name=pulumi.Output.from_input("ca-secret"),
        crl_bucket_path=pulumi.Output.from_input("s3://bucket/crl.pem"),
        hsm_backend="software",
    )
    assert outputs is not None


def test_compute_outputs_constructible() -> None:
    """ComputeOutputs must accept pulumi.Output arguments."""
    outputs = ComputeOutputs(
        service_url=pulumi.Output.from_input("https://api.example.com"),
        worker_service_name=pulumi.Output.from_input("skreg-worker"),
    )
    assert outputs is not None


def test_network_outputs_constructible() -> None:
    """NetworkOutputs must accept pulumi.Output arguments."""
    outputs = NetworkOutputs(
        vpc_id=pulumi.Output.from_input("vpc-123"),
        private_subnet_ids=[
            pulumi.Output.from_input("subnet-1"),
            pulumi.Output.from_input("subnet-2"),
        ],
    )
    assert outputs is not None
