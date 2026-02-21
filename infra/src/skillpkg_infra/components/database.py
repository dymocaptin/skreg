"""Provider-agnostic database component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class DatabaseOutputs:
    """Resolved connection outputs from a provisioned database component."""

    def __init__(
        self,
        connection_secret_name: pulumi.Output[str],
        host: pulumi.Output[str],
        port: pulumi.Output[int],
        database_name: pulumi.Output[str],
    ) -> None:
        """Initialise database outputs.

        Args:
            connection_secret_name: Provider secret store key for DB credentials.
            host: Database hostname or IP address.
            port: Database TCP port.
            database_name: Name of the application database.
        """
        self.connection_secret_name: pulumi.Output[str] = connection_secret_name
        self.host: pulumi.Output[str] = host
        self.port: pulumi.Output[int] = port
        self.database_name: pulumi.Output[str] = database_name


class SkillpkgDatabase(Protocol):
    """Provider-agnostic interface for the registry PostgreSQL component."""

    @property
    def outputs(self) -> DatabaseOutputs:
        """Return the resolved database connection outputs."""
        ...
