"""Typed configuration loaded from environment variables at startup."""

from __future__ import annotations

import logging
from enum import StrEnum
from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict

logger: logging.Logger = logging.getLogger(__name__)


class CloudProvider(StrEnum):
    AWS = "aws"
    GCP = "gcp"
    AZURE = "azure"


class HsmBackend(StrEnum):
    HSM = "hsm"
    SOFTWARE = "software"


class StackConfig(BaseSettings):
    """Fully validated infrastructure stack configuration."""

    model_config = SettingsConfigDict(
        env_prefix="SKREG_",
        env_file=".env",
        env_file_encoding="utf-8",
    )

    cloud_provider: CloudProvider
    api_image_uri: str = ""
    worker_image_uri: str = ""
    domain_name: str = ""
    hsm_backend: HsmBackend = HsmBackend.HSM
    multi_az: bool = False
    environment: Literal["prod", "staging", "dev"] = "prod"

    @classmethod
    def load(cls) -> StackConfig:
        """Load and validate configuration from the environment."""
        config = cls()  # type: ignore[call-arg]
        logger.debug(
            "stack_config_loaded",
            extra={
                "cloud_provider": config.cloud_provider.value,
                "hsm_backend": config.hsm_backend.value,
                "multi_az": config.multi_az,
                "environment": config.environment,
            },
        )
        return config
