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
        public_subnet_ids: list[pulumi.Output[str]],
        private_subnet_ids: list[pulumi.Output[str]],
    ) -> None:
        self.vpc_id: pulumi.Output[str] = vpc_id
        self.public_subnet_ids: list[pulumi.Output[str]] = public_subnet_ids
        self.private_subnet_ids: list[pulumi.Output[str]] = private_subnet_ids


class SkillpkgNetwork(Protocol):
    @property
    def outputs(self) -> NetworkOutputs: ...
