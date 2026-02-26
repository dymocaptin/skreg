"""Tests for the StackConfig environment-driven settings class."""
from __future__ import annotations

import pytest

from skillpkg_infra.config import CloudProvider, HsmBackend, StackConfig


def test_cloud_provider_values() -> None:
    assert CloudProvider.AWS == "aws"
    assert CloudProvider.GCP == "gcp"
    assert CloudProvider.AZURE == "azure"


def test_hsm_backend_values() -> None:
    assert HsmBackend.HSM == "hsm"
    assert HsmBackend.SOFTWARE == "software"


def test_stack_config_load(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.AWS
    assert config.hsm_backend == HsmBackend.HSM
    assert config.multi_az is False
    assert config.environment == "prod"
    assert config.api_image_uri == ""
    assert config.worker_image_uri == ""


def test_stack_config_defaults(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "gcp")
    monkeypatch.setenv("SKILLPKG_HSM_BACKEND", "software")
    monkeypatch.setenv("SKILLPKG_MULTI_AZ", "true")
    monkeypatch.setenv("SKILLPKG_ENVIRONMENT", "staging")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.GCP
    assert config.hsm_backend == HsmBackend.SOFTWARE
    assert config.multi_az is True
    assert config.environment == "staging"


def test_stack_config_image_uris(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    monkeypatch.setenv("SKILLPKG_API_IMAGE_URI", "123.dkr.ecr.us-west-2.amazonaws.com/skreg-api:latest")
    monkeypatch.setenv("SKILLPKG_WORKER_IMAGE_URI", "123.dkr.ecr.us-west-2.amazonaws.com/skreg-worker:latest")
    config = StackConfig.load()
    assert config.api_image_uri == "123.dkr.ecr.us-west-2.amazonaws.com/skreg-api:latest"
    assert config.worker_image_uri == "123.dkr.ecr.us-west-2.amazonaws.com/skreg-worker:latest"


def test_stack_config_domain_name_defaults_to_empty(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    config = StackConfig.load()
    assert config.domain_name == ""


def test_stack_config_domain_name_read_from_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    monkeypatch.setenv("SKILLPKG_DOMAIN_NAME", "api.skreg.ai")
    config = StackConfig.load()
    assert config.domain_name == "api.skreg.ai"
