"""AWS Secrets Manager + S3 software PKI implementation of SkillpkgPki."""

from __future__ import annotations

import datetime
import logging

import pulumi
import pulumi_aws as aws
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding as asym_padding
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

from skreg_infra.components.pki import PkiOutputs

logger: logging.Logger = logging.getLogger(__name__)


def _generate_root_ca() -> tuple[str, str]:
    """Generate a self-signed RSA-4096 root CA key and certificate.

    Returns:
        ``(pem_key, pem_cert)`` — both as ASCII strings. Validity: 10 years.
    """
    private_key = rsa.generate_private_key(public_exponent=65537, key_size=4096)

    subject = x509.Name(
        [
            x509.NameAttribute(NameOID.COMMON_NAME, "skreg Root CA"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "skreg"),
        ]
    )
    now = datetime.datetime.now(tz=datetime.UTC)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(subject)
        .public_key(private_key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now)
        .not_valid_after(now + datetime.timedelta(days=3650))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .sign(
            private_key,
            hashes.SHA256(),
            rsa_padding=asym_padding.PSS(
                mgf=asym_padding.MGF1(hashes.SHA256()),
                salt_length=32,
            ),
        )
    )
    pem_key = private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.TraditionalOpenSSL,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode("ascii")
    pem_cert = cert.public_bytes(serialization.Encoding.PEM).decode("ascii")
    return pem_key, pem_cert


def _generate_publisher_ca(root_key_pem: str, root_cert_pem: str) -> tuple[str, str]:
    """Generate an RSA-2048 Publisher CA key and certificate signed by the root CA.

    Returns:
        ``(pem_key, pem_cert)`` — both as ASCII strings. Validity: 5 years.
    """
    _root_key_raw = serialization.load_pem_private_key(root_key_pem.encode(), password=None)
    if not isinstance(_root_key_raw, rsa.RSAPrivateKey):
        raise TypeError("root CA key must be an RSA private key")
    root_key: rsa.RSAPrivateKey = _root_key_raw
    root_cert = x509.load_pem_x509_certificate(root_cert_pem.encode())

    private_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)

    subject = x509.Name(
        [
            x509.NameAttribute(NameOID.COMMON_NAME, "skreg Publisher CA"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "skreg"),
        ]
    )
    now = datetime.datetime.now(tz=datetime.UTC)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(root_cert.subject)
        .public_key(private_key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now)
        .not_valid_after(now + datetime.timedelta(days=1825))  # 5 years
        .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
        .sign(
            root_key,
            hashes.SHA256(),
            rsa_padding=asym_padding.PSS(
                mgf=asym_padding.MGF1(hashes.SHA256()),
                salt_length=32,
            ),
        )
    )
    pem_key = private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.TraditionalOpenSSL,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode("ascii")
    pem_cert = cert.public_bytes(serialization.Encoding.PEM).decode("ascii")
    return pem_key, pem_cert


class AwsPkiArgs:
    """Arguments for the software PKI component."""

    def __init__(self, bucket_name: pulumi.Input[str]) -> None:
        self.bucket_name: pulumi.Input[str] = bucket_name


class AwsPki(pulumi.ComponentResource):
    """AWS Secrets Manager + S3 software PKI satisfying ``SkillpkgPki``.

    Generates an RSA-4096 root CA on first deploy; ``ignore_changes`` on the
    key secret prevents unintentional rotation on subsequent ``pulumi up`` runs.
    """

    def __init__(
        self,
        name: str,
        args: AwsPkiArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:Pki", name, {}, opts)

        logger.debug("provisioning_aws_pki", extra={"name": name})

        pem_key, pem_cert = _generate_root_ca()

        ca_key_secret = aws.secretsmanager.Secret(
            f"{name}-ca-key",
            aws.secretsmanager.SecretArgs(name="skreg/pki/root-ca-key"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.secretsmanager.SecretVersion(
            f"{name}-ca-key-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=ca_key_secret.id,
                secret_string=pem_key,
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["secret_string"]),
        )

        ca_cert_secret = aws.secretsmanager.Secret(
            f"{name}-ca-cert",
            aws.secretsmanager.SecretArgs(name="skreg/pki/root-ca-cert"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        ca_cert_version = aws.secretsmanager.SecretVersion(
            f"{name}-ca-cert-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=ca_cert_secret.id,
                secret_string=pem_cert,
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["secret_string"]),
        )

        pem_pub_key, pem_pub_cert = _generate_publisher_ca(pem_key, pem_cert)

        publisher_ca_key_secret = aws.secretsmanager.Secret(
            f"{name}-publisher-ca-key",
            aws.secretsmanager.SecretArgs(name="skreg/pki/publisher-ca-key"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.secretsmanager.SecretVersion(
            f"{name}-publisher-ca-key-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=publisher_ca_key_secret.id,
                secret_string=pem_pub_key,
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["secret_string"]),
        )

        publisher_ca_cert_secret = aws.secretsmanager.Secret(
            f"{name}-publisher-ca-cert",
            aws.secretsmanager.SecretArgs(name="skreg/pki/publisher-ca-cert"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        publisher_ca_cert_version = aws.secretsmanager.SecretVersion(
            f"{name}-publisher-ca-cert-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=publisher_ca_cert_secret.id,
                secret_string=pem_pub_cert,
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["secret_string"]),
        )

        self.publisher_ca_cert_pem: pulumi.Output[str] = (
            publisher_ca_cert_version.secret_string.apply(lambda s: s or "")
        )

        aws.s3.BucketObject(
            f"{name}-crl",
            aws.s3.BucketObjectArgs(
                bucket=args.bucket_name,
                key=".well-known/crl.pem",
                content="",
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["content"]),
        )

        self._outputs: PkiOutputs = PkiOutputs(
            hsm_key_id=ca_key_secret.id,
            intermediate_ca_cert_secret_name=ca_cert_secret.name,
            crl_bucket_path=pulumi.Output.from_input(args.bucket_name).apply(
                lambda b: f"s3://{b}/.well-known/crl.pem"
            ),
            hsm_backend="software",
            publisher_ca_key_secret_name=publisher_ca_key_secret.name,
        )
        self.root_ca_cert_pem: pulumi.Output[str] = ca_cert_version.secret_string.apply(
            lambda s: s or ""
        )

        self.register_outputs(
            {
                "hsm_key_id": self._outputs.hsm_key_id,
                "intermediate_ca_cert_secret_name": self._outputs.intermediate_ca_cert_secret_name,
                "crl_bucket_path": self._outputs.crl_bucket_path,
                "root_ca_cert_pem": self.root_ca_cert_pem,
                "publisher_ca_cert_pem": self.publisher_ca_cert_pem,
                "publisher_ca_key_secret_name": self._outputs.publisher_ca_key_secret_name,
            }
        )

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        return self._outputs
