"""Pulumi stack entry point for skreg infrastructure."""

from __future__ import annotations

import logging

import structlog

from skreg_infra.config import StackConfig
from skreg_infra.dispatch import CloudStack

logger: logging.Logger = logging.getLogger(__name__)


class SkregStack:
    """Resolves the substrate and pluggable components, then runs the app stack."""

    def __init__(self, config: StackConfig) -> None:
        self._config: StackConfig = config

    def run(self) -> None:
        logger.info(
            "stack_run_started",
            extra={
                "cloud_provider": self._config.cloud_provider.value,
                "database_backend": self._config.database_backend.value,
                "storage_backend": self._config.storage_backend.value,
                "dns_backend": self._config.dns_backend.value,
            },
        )
        CloudStack(self._config).run()


if __name__ == "__main__":
    structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
    SkregStack(config=StackConfig.load()).run()
