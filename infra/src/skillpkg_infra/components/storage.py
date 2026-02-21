"""Provider-agnostic object storage + CDN component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class StorageOutputs:
    """Resolved outputs from a provisioned object storage + CDN component."""

    def __init__(
        self,
        bucket_name: pulumi.Output[str],
        cdn_base_url: pulumi.Output[str],
        service_account_secret_name: pulumi.Output[str],
    ) -> None:
        """Initialise storage outputs.

        Args:
            bucket_name: Name of the object storage bucket.
            cdn_base_url: Base URL for CDN-distributed package downloads.
            service_account_secret_name: Secret name for storage service credentials.
        """
        self.bucket_name: pulumi.Output[str] = bucket_name
        self.cdn_base_url: pulumi.Output[str] = cdn_base_url
        self.service_account_secret_name: pulumi.Output[str] = service_account_secret_name


class SkillpkgStorage(Protocol):
    """Provider-agnostic interface for the registry object storage component."""

    @property
    def outputs(self) -> StorageOutputs:
        """Return the resolved storage outputs."""
        ...
