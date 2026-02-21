"""Typed configuration loaded from environment variables at startup."""

from __future__ import annotations

import logging
from enum import StrEnum
from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict

logger: logging.Logger = logging.getLogger(__name__)


class CloudProvider(StrEnum):
    """Supported cloud provider deployment targets."""

    AWS = "aws"
    GCP = "gcp"
    AZURE = "azure"


class HsmBackend(StrEnum):
    """PKI signing key storage backend."""

    HSM = "hsm"
    SOFTWARE = "software"


class StackConfig(BaseSettings):
    """Fully validated infrastructure stack configuration.

    All values are sourced from environment variables at startup.
    Raises ``ValidationError`` on missing or invalid values.
    """

    model_config = SettingsConfigDict(
        env_prefix="SKILLPKG_",
        env_file=".env",
        env_file_encoding="utf-8",
    )

    cloud_provider: CloudProvider
    image_uri: str
    hsm_backend: HsmBackend = HsmBackend.HSM
    multi_az: bool = False
    environment: Literal["prod", "staging", "dev"] = "prod"

    @classmethod
    def load(cls) -> StackConfig:
        """Load and validate configuration from the environment.

        Logs each resolved setting at DEBUG level.
        Raises ``pydantic.ValidationError`` on missing or invalid values.
        """
        config = cls()  # type: ignore[call-arg]  # env vars supply required fields
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
