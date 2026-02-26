"""Unit tests for the AWS PKI component using Pulumi mocks."""
from __future__ import annotations

from unittest.mock import patch

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skreg_infra.providers.aws.pki import AwsPki, AwsPkiArgs, _generate_root_ca  # noqa: E402


def test_generate_root_ca_returns_pem_strings() -> None:
    """_generate_root_ca returns two PEM-encoded strings."""
    key_pem, cert_pem = _generate_root_ca()
    assert "PRIVATE KEY" in key_pem
    assert "CERTIFICATE" in cert_pem


def test_generate_root_ca_key_is_4096_bits() -> None:
    """_generate_root_ca produces a 4096-bit RSA key."""
    from cryptography.hazmat.primitives.serialization import load_pem_private_key

    key_pem, _ = _generate_root_ca()
    key = load_pem_private_key(key_pem.encode(), password=None)
    assert key.key_size == 4096  # type: ignore[attr-defined]


@pulumi.runtime.test
def test_pki_root_ca_cert_pem_is_exposed() -> None:
    """AwsPki must expose root_ca_cert_pem as a non-empty Output."""
    with patch(
        "skreg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ):
        pki = AwsPki("test-pki", AwsPkiArgs(bucket_name="test-bucket"))

    def check(cert: str) -> None:
        assert cert

    return pki.root_ca_cert_pem.apply(check)


@pulumi.runtime.test
def test_pki_hsm_key_id_is_set() -> None:
    """AwsPki outputs.hsm_key_id must be non-empty."""
    with patch(
        "skreg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ):
        pki = AwsPki("test-pki2", AwsPkiArgs(bucket_name="test-bucket"))

    def check(key_id: str) -> None:
        assert key_id

    return pki.outputs.hsm_key_id.apply(check)
