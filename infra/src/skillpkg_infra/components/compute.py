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
    ) -> None:
        """Initialise compute outputs.

        Args:
            service_url: Public HTTPS URL of the registry API service.
            worker_service_name: Internal name of the vetting worker service.
        """
        self.service_url: pulumi.Output[str] = service_url
        self.worker_service_name: pulumi.Output[str] = worker_service_name


class SkillpkgCompute(Protocol):
    """Provider-agnostic interface for the registry compute component."""

    @property
    def outputs(self) -> ComputeOutputs:
        """Return the resolved compute outputs."""
        ...
