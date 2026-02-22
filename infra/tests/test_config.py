"""Tests for the StackConfig environment-driven settings class."""
from __future__ import annotations

import pytest

from skillpkg_infra.config import CloudProvider, HsmBackend, StackConfig


def test_cloud_provider_values() -> None:
    """CloudProvider enum must expose the three supported providers."""
    assert CloudProvider.AWS == "aws"
    assert CloudProvider.GCP == "gcp"
    assert CloudProvider.AZURE == "azure"


def test_hsm_backend_values() -> None:
    """HsmBackend enum must expose hsm and software backends."""
    assert HsmBackend.HSM == "hsm"
    assert HsmBackend.SOFTWARE == "software"


def test_stack_config_load(monkeypatch: pytest.MonkeyPatch) -> None:
    """StackConfig.load() must resolve required fields from the environment."""
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    monkeypatch.setenv("SKILLPKG_IMAGE_URI", "123456789.dkr.ecr.us-east-1.amazonaws.com/skreg:latest")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.AWS
    assert config.hsm_backend == HsmBackend.HSM
    assert config.multi_az is False
    assert config.environment == "prod"


def test_stack_config_defaults(monkeypatch: pytest.MonkeyPatch) -> None:
    """StackConfig must accept optional overrides."""
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "gcp")
    monkeypatch.setenv("SKILLPKG_IMAGE_URI", "gcr.io/project/skreg:latest")
    monkeypatch.setenv("SKILLPKG_HSM_BACKEND", "software")
    monkeypatch.setenv("SKILLPKG_MULTI_AZ", "true")
    monkeypatch.setenv("SKILLPKG_ENVIRONMENT", "staging")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.GCP
    assert config.hsm_backend == HsmBackend.SOFTWARE
    assert config.multi_az is True
    assert config.environment == "staging"
