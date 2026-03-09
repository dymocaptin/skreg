"""Unit tests for the AWS PKI component using Pulumi mocks."""
from __future__ import annotations

from unittest.mock import patch

import pulumi
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding as asym_padding
from cryptography.hazmat.primitives.serialization import load_pem_private_key
from cryptography.x509 import load_pem_x509_certificate
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
from skreg_infra.providers.aws.pki import _generate_publisher_ca  # noqa: E402


def test_generate_root_ca_returns_pem_strings() -> None:
    """_generate_root_ca returns two PEM-encoded strings."""
    key_pem, cert_pem = _generate_root_ca()
    assert "PRIVATE KEY" in key_pem
    assert "CERTIFICATE" in cert_pem


def test_generate_root_ca_key_is_4096_bits() -> None:
    """_generate_root_ca produces a 4096-bit RSA key."""
    key_pem, _ = _generate_root_ca()
    key = load_pem_private_key(key_pem.encode(), password=None)
    assert key.key_size == 4096  # type: ignore[attr-defined]


@pulumi.runtime.test
def test_pki_root_ca_cert_pem_is_exposed() -> None:
    """AwsPki must expose root_ca_cert_pem as a non-empty Output."""
    with patch(
        "skreg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ), patch(
        "skreg_infra.providers.aws.pki._generate_publisher_ca",
        return_value=("---PUB-KEY---", "---PUB-CERT---"),
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
    ), patch(
        "skreg_infra.providers.aws.pki._generate_publisher_ca",
        return_value=("---PUB-KEY---", "---PUB-CERT---"),
    ):
        pki = AwsPki("test-pki2", AwsPkiArgs(bucket_name="test-bucket"))

    def check(key_id: str) -> None:
        assert key_id

    return pki.outputs.hsm_key_id.apply(check)


def test_generate_publisher_ca_returns_pem_strings() -> None:
    """_generate_publisher_ca returns two PEM-encoded strings."""
    root_key_pem, root_cert_pem = _generate_root_ca()
    pub_key_pem, pub_cert_pem = _generate_publisher_ca(root_key_pem, root_cert_pem)
    assert "PRIVATE KEY" in pub_key_pem
    assert "CERTIFICATE" in pub_cert_pem


def test_generate_publisher_ca_cert_is_signed_by_root() -> None:
    """Publisher CA cert must be cryptographically signed by the root CA using PSS."""
    root_key_pem, root_cert_pem = _generate_root_ca()
    _, pub_cert_pem = _generate_publisher_ca(root_key_pem, root_cert_pem)

    root_cert = load_pem_x509_certificate(root_cert_pem.encode())
    pub_cert = load_pem_x509_certificate(pub_cert_pem.encode())

    root_pub = root_cert.public_key()
    root_pub.verify(
        pub_cert.signature,
        pub_cert.tbs_certificate_bytes,
        asym_padding.PSS(mgf=asym_padding.MGF1(hashes.SHA256()), salt_length=32),
        hashes.SHA256(),
    )


@pulumi.runtime.test
def test_pki_exposes_publisher_ca_outputs() -> None:
    """AwsPki must expose publisher_ca_cert_pem as a non-empty Output and publisher_ca_key_secret_name."""
    with patch(
        "skreg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ), patch(
        "skreg_infra.providers.aws.pki._generate_publisher_ca",
        return_value=("---PUB-KEY---", "---PUB-CERT---"),
    ):
        pki = AwsPki("test-pki3", AwsPkiArgs(bucket_name="test-bucket"))

    def check_cert(cert: str) -> None:
        assert cert

    def check_name(name: str) -> None:
        assert name

    pki.publisher_ca_cert_pem.apply(check_cert)
    return pki.outputs.publisher_ca_key_secret_name.apply(check_name)


def test_generate_root_ca_cert_uses_pss_signature() -> None:
    """Root CA cert must use RSA-PSS signature algorithm."""
    key_pem, cert_pem = _generate_root_ca()

    cert = load_pem_x509_certificate(cert_pem.encode())
    key = load_pem_private_key(key_pem.encode(), password=None)

    pub = key.public_key()
    pub.verify(
        cert.signature,
        cert.tbs_certificate_bytes,
        asym_padding.PSS(mgf=asym_padding.MGF1(hashes.SHA256()), salt_length=32),
        hashes.SHA256(),
    )
