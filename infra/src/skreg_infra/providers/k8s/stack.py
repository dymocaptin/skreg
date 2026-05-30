"""Orchestrates all K8s provider components."""

from __future__ import annotations

import pulumi

from skreg_infra.config import StackConfig


class K8sStack:
    def __init__(self, config: StackConfig) -> None:
        self._config = config

    def run(self) -> None:
        from skreg_infra.providers.k8s.ci import K8sCi
        from skreg_infra.providers.k8s.compute import K8sCompute
        from skreg_infra.providers.k8s.database import K8sDatabase
        from skreg_infra.providers.k8s.dispatcher import K8sDispatcher
        from skreg_infra.providers.k8s.dns import K8sDns
        from skreg_infra.providers.k8s.email import K8sEmail
        from skreg_infra.providers.k8s.network import K8sNetwork
        from skreg_infra.providers.k8s.pki import K8sPki
        from skreg_infra.providers.k8s.registry import K8sRegistry
        from skreg_infra.providers.k8s.storage import K8sStorage

        config = self._config

        K8sNetwork("skreg-network")
        database = K8sDatabase("skreg-db")
        storage = K8sStorage("skreg-storage")
        pki = K8sPki("skreg-pki")
        email = K8sEmail("skreg-email")
        registry = K8sRegistry("skreg-registry")

        compute = K8sCompute(
            "skreg-compute",
            api_image=config.api_image_uri,
            worker_image=config.worker_image_uri,
            s3_bucket=storage.outputs.bucket_name,
            from_email=config.from_email,
            domain_name=config.domain_name,
        )

        K8sDispatcher(
            "skreg-dispatcher",
            worker_image=config.worker_image_uri,
            s3_bucket=storage.outputs.bucket_name,
            from_email=config.from_email,
        )

        K8sCi("skreg-ci", github_repo=config.github_repo)
        K8sDns(
            "skreg-dns",
            domain_name=config.domain_name,
            hosted_zone_id=config.hosted_zone_id,
        )

        # suppress unused-variable warnings — these resources register themselves
        _ = database, pki, email

        pulumi.export("api_url", f"https://api.{config.domain_name}")
        pulumi.export("registry_url", registry.outputs.registry_url)
        pulumi.export("api_service_url", compute.outputs.service_url)
