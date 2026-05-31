"""Provider-agnostic PKI + HSM component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class PkiOutputs:
    """Resolved outputs from a provisioned PKI component."""

    def __init__(
        self,
        hsm_key_id: pulumi.Output[str],
        intermediate_ca_cert_secret_name: pulumi.Output[str],
        crl_bucket_path: pulumi.Output[str],
        hsm_backend: str,
        publisher_ca_key_secret_name: pulumi.Output[str],
        publisher_ca_key_secret_arn: pulumi.Output[str] | None = None,
        registry_ca_key_secret_arn: pulumi.Output[str] | None = None,
    ) -> None:
        self.hsm_key_id: pulumi.Output[str] = hsm_key_id
        self.intermediate_ca_cert_secret_name: pulumi.Output[str] = intermediate_ca_cert_secret_name
        self.crl_bucket_path: pulumi.Output[str] = crl_bucket_path
        self.hsm_backend: str = hsm_backend
        self.publisher_ca_key_secret_name: pulumi.Output[str] = publisher_ca_key_secret_name
        # ARNs are AWS-specific; None on providers that use K8s Secrets instead.
        self.publisher_ca_key_secret_arn: pulumi.Output[str] | None = publisher_ca_key_secret_arn
        self.registry_ca_key_secret_arn: pulumi.Output[str] | None = registry_ca_key_secret_arn


class SkillpkgPki(Protocol):
    """Provider-agnostic interface for the registry PKI + HSM component."""

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        ...
