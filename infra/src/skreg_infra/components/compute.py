"""Provider-agnostic container compute component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class ComputeOutputs:
    """Resolved outputs from a provisioned container compute component."""

    def __init__(
        self,
        service_url: pulumi.Output[str],
        worker_service_name: pulumi.Output[str],
        alb_dns_name: pulumi.Output[str] | None = None,
        cert_validation_cname: pulumi.Output[dict[str, str] | None] | None = None,
    ) -> None:
        """Initialise compute outputs.

        Args:
            service_url: Public HTTPS URL of the registry API service.
            worker_service_name: Provider-specific name of the background worker service.
            alb_dns_name: DNS name of the Application Load Balancer, when applicable.
            cert_validation_cname: CNAME record used for ACM certificate DNS validation.
                The inner ``| None`` reflects the AWS ACM provider's behaviour of
                returning ``None`` for validation options while the certificate is
                being provisioned.
        """
        self.service_url: pulumi.Output[str] = service_url
        self.worker_service_name: pulumi.Output[str] = worker_service_name
        self.alb_dns_name: pulumi.Output[str] | None = alb_dns_name
        self.cert_validation_cname: pulumi.Output[dict[str, str] | None] | None = (
            cert_validation_cname
        )


class SkillpkgCompute(Protocol):
    """Provider-agnostic interface for the registry compute component."""

    @property
    def outputs(self) -> ComputeOutputs:
        """Return the resolved compute outputs."""
        ...
