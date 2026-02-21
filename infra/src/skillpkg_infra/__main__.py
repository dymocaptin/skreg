"""Pulumi stack entry point for skreg infrastructure."""

from __future__ import annotations

import logging

import structlog

from skillpkg_infra.config import StackConfig

logger: logging.Logger = logging.getLogger(__name__)


class SkillpkgStack:
    """Orchestrates all provider-agnostic infrastructure components."""

    def __init__(self, config: StackConfig) -> None:
        """Initialise the stack with resolved configuration."""
        self._config: StackConfig = config

    def run(self) -> None:
        """Provision the full infrastructure stack."""
        logger.info(
            "stack_run_started",
            extra={"cloud_provider": self._config.cloud_provider.value},
        )
        raise NotImplementedError("Provider implementations not yet built.")


if __name__ == "__main__":
    structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
    SkillpkgStack(config=StackConfig.load()).run()
