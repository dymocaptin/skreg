"""Tests for K8sStack orchestration."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class K8sMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", {**args.inputs, "name": args.name})

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(K8sMocks())

from skreg_infra.config import (  # noqa: E402
    CloudProvider,
    DatabaseBackend,
    DnsBackend,
    StorageBackend,
    StackConfig,
)
from skreg_infra.providers.k8s.stack import K8sStack  # noqa: E402


def _make_config(**kwargs: object) -> StackConfig:
    return StackConfig(
        cloud_provider=CloudProvider.K8S,
        api_image_uri="localhost:30500/skreg-api:latest",
        worker_image_uri="localhost:30500/skreg-worker:latest",
        domain_name="skreg.ai",
        from_email="noreply@skreg.ai",
        **kwargs,  # type: ignore[arg-type]
    )


def test_k8s_stack_run_completes() -> None:
    """K8sStack.run() should complete without raising."""
    stack = K8sStack(_make_config())
    stack.run()  # should not raise


def test_k8s_stack_stores_config() -> None:
    config = _make_config()
    stack = K8sStack(config)
    assert stack._config is config


def test_k8s_stack_run_with_managed_backends() -> None:
    """K8sStack.run() works with managed database/storage/dns backends."""
    stack = K8sStack(
        _make_config(
            database_backend=DatabaseBackend.MANAGED,
            storage_backend=StorageBackend.MANAGED,
            dns_backend=DnsBackend.MANAGED,
            hosted_zone_id="Z123",
            github_repo="owner/repo",
        )
    )
    stack.run()
