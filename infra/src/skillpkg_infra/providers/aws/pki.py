"""AWS Secrets Manager + S3 software PKI implementation of SkillpkgPki."""

from __future__ import annotations

import datetime
import logging

import pulumi
import pulumi_aws as aws
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

from skillpkg_infra.components.pki import PkiOutputs

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
        .sign(private_key, hashes.SHA256())
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
        super().__init__("skillpkg:aws:Pki", name, {}, opts)

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
            opts=pulumi.ResourceOptions(parent=self),
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
            }
        )

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        return self._outputs
