"""Provider-agnostic network component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class NetworkOutputs:
    """Resolved outputs from a provisioned network component."""

    def __init__(
        self,
        vpc_id: pulumi.Output[str],
        private_subnet_ids: list[pulumi.Output[str]],
    ) -> None:
        """Initialise network outputs.

        Args:
            vpc_id: ID of the provisioned VPC or equivalent.
            private_subnet_ids: IDs of private subnets for backend services.
        """
        self.vpc_id: pulumi.Output[str] = vpc_id
        self.private_subnet_ids: list[pulumi.Output[str]] = private_subnet_ids


class SkillpkgNetwork(Protocol):
    """Provider-agnostic interface for the network component."""

    @property
    def outputs(self) -> NetworkOutputs:
        """Return the resolved network outputs."""
        ...
