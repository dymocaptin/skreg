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
    ) -> None:
        """Initialise PKI outputs.

        Args:
            hsm_key_id: Provider-specific HSM key identifier.
            intermediate_ca_cert_secret_name: Secret name for the intermediate CA cert.
            crl_bucket_path: Object storage path for the CRL file.
            hsm_backend: Either ``"hsm"`` or ``"software"``.
        """
        self.hsm_key_id: pulumi.Output[str] = hsm_key_id
        self.intermediate_ca_cert_secret_name: pulumi.Output[str] = (
            intermediate_ca_cert_secret_name
        )
        self.crl_bucket_path: pulumi.Output[str] = crl_bucket_path
        self.hsm_backend: str = hsm_backend


class SkillpkgPki(Protocol):
    """Provider-agnostic interface for the registry PKI + HSM component."""

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        ...
