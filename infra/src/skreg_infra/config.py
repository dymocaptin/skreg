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
    K8S = "k8s"


class HsmBackend(StrEnum):
    HSM = "hsm"
    SOFTWARE = "software"


class DatabaseBackend(StrEnum):
    INCLUSTER = "incluster"
    MANAGED = "managed"


class StorageBackend(StrEnum):
    INCLUSTER = "incluster"
    MANAGED = "managed"


class DnsBackend(StrEnum):
    MANUAL = "manual"
    MANAGED = "managed"


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
    existing_cert_arn: str = ""
    web_domain_name: str = ""
    web_cert_arn: str = ""
    from_email: str = ""
    # AWS path: SES SMTP relay host (e.g. email-smtp.us-west-2.amazonaws.com)
    smtp_host: str = ""
    smtp_port: int = 587
    # ARN of a Secrets Manager secret with keys "username" and "password" for SMTP auth.
    # Required for the AWS path; absent on K8s (uses anonymous in-cluster Postfix).
    smtp_credentials_secret_arn: str = ""
    hsm_backend: HsmBackend = HsmBackend.HSM
    multi_az: bool = False
    environment: Literal["prod", "staging", "dev"] = "prod"
    github_repo: str = ""
    hosted_zone_id: str = ""
    database_backend: DatabaseBackend = DatabaseBackend.INCLUSTER
    storage_backend: StorageBackend = StorageBackend.INCLUSTER
    dns_backend: DnsBackend = DnsBackend.MANUAL
    ingress_endpoint: str = ""

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
