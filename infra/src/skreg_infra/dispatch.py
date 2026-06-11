"""Substrate and backend selection per cloud provider.

Dispatches on ``StackConfig.cloud_provider``:

- ``k8s``     — the existing in-cluster path (``K8sStack``), unchanged.
- ``aws``     — EKS substrate; RDS / S3 / Route53 when the matching backend is
  set to ``managed``.
- ``gcp``     — GKE substrate; Cloud SQL / GCS / Cloud DNS when ``managed``.
- ``azure``   — AKS substrate; Flexible Server / Blob / Azure DNS when
  ``managed``.

Managed-cloud paths provision the substrate and the selected backing services
and export their contracts. Deploying the application workloads onto a managed
substrate requires the cluster kubeconfig, which is only available after the
first ``pulumi up`` (see each substrate's docstring); the app layer therefore
still runs via the ``k8s`` provider pointed at the target cluster.

Required environment per provider (all prefixed ``SKREG_``):

- aws:   ``AWS_REGION`` (default us-west-2), ``HOSTED_ZONE_ID`` +
  ``INGRESS_ENDPOINT`` for managed DNS.
- gcp:   ``GCP_PROJECT`` (required), ``GCP_REGION`` (default us-central1),
  ``GCP_MANAGED_ZONE`` + ``INGRESS_ENDPOINT`` for managed DNS.
- azure: ``AZURE_LOCATION`` (default eastus), ``AZURE_RESOURCE_GROUP``
  (default skreg-data-rg), ``AZURE_DNS_ZONE`` + ``INGRESS_ENDPOINT`` for
  managed DNS.

Cloud credentials are sourced by the respective Pulumi providers from their
standard environment (``AWS_*``, ``GOOGLE_APPLICATION_CREDENTIALS``,
``ARM_*``).
"""

from __future__ import annotations

import logging

import pulumi

from skreg_infra.config import (
    CloudProvider,
    DatabaseBackend,
    DnsBackend,
    StackConfig,
    StorageBackend,
)

logger: logging.Logger = logging.getLogger(__name__)


class CloudStack:
    """Provisions the substrate and managed backends for one cloud provider."""

    def __init__(self, config: StackConfig) -> None:
        self._config: StackConfig = config

    def run(self) -> None:
        """Provision resources for the configured provider."""
        provider = self._config.cloud_provider
        logger.info("cloud_stack_dispatch", extra={"cloud_provider": provider.value})
        if provider is CloudProvider.K8S:
            from skreg_infra.providers.k8s.stack import K8sStack

            K8sStack(self._config).run()
        elif provider is CloudProvider.AWS:
            self._run_aws()
        elif provider is CloudProvider.GCP:
            self._run_gcp()
        else:
            self._run_azure()

    def _run_aws(self) -> None:
        from skreg_infra.providers.aws.database import RdsDatabase
        from skreg_infra.providers.aws.dns import Route53Dns
        from skreg_infra.providers.aws.storage import S3ObjectStore
        from skreg_infra.providers.aws.substrate import EksSubstrate

        config = self._config
        substrate = EksSubstrate("skreg-substrate", region=config.aws_region)
        pulumi.export("kubeconfig", substrate.kubeconfig)

        if config.database_backend is DatabaseBackend.MANAGED:
            database = RdsDatabase(
                "skreg-db",
                vpc_id=substrate.vpc_id,
                subnet_ids=substrate.subnet_ids,
                source_cidr=EksSubstrate.VPC_CIDR,
            )
            pulumi.export("db_secret_name", database.contract.dsn_secret_name)

        if config.storage_backend is StorageBackend.MANAGED:
            storage = S3ObjectStore("skreg-storage")
            pulumi.export("storage_bucket", storage.contract.bucket)

        if self._managed_dns_target() is not None:
            Route53Dns(
                "skreg-dns",
                domain_name=config.domain_name,
                hosted_zone_id=config.hosted_zone_id,
                target=config.ingress_endpoint,
            )

    def _run_gcp(self) -> None:
        from skreg_infra.providers.gcp.database import CloudSqlDatabase
        from skreg_infra.providers.gcp.dns import CloudDnsDns
        from skreg_infra.providers.gcp.storage import GcsObjectStore
        from skreg_infra.providers.gcp.substrate import GkeSubstrate

        config = self._config
        if not config.gcp_project:
            raise ValueError("SKREG_GCP_PROJECT is required for the gcp provider")
        substrate = GkeSubstrate(
            "skreg-substrate", project=config.gcp_project, location=config.gcp_region
        )
        pulumi.export("cluster_endpoint", substrate.cluster.endpoint)

        if config.database_backend is DatabaseBackend.MANAGED:
            database = CloudSqlDatabase(
                "skreg-db", project=config.gcp_project, region=config.gcp_region
            )
            pulumi.export("db_secret_name", database.contract.dsn_secret_name)

        if config.storage_backend is StorageBackend.MANAGED:
            storage = GcsObjectStore("skreg-storage", project=config.gcp_project)
            pulumi.export("storage_bucket", storage.contract.bucket)

        if self._managed_dns_target() is not None:
            if not config.gcp_managed_zone:
                raise ValueError("SKREG_GCP_MANAGED_ZONE is required for managed DNS on gcp")
            CloudDnsDns(
                "skreg-dns",
                project=config.gcp_project,
                domain_name=config.domain_name,
                managed_zone=config.gcp_managed_zone,
                target=config.ingress_endpoint,
            )

    def _run_azure(self) -> None:
        import pulumi_azure_native.resources as resources

        from skreg_infra.providers.azure.database import AzurePgDatabase
        from skreg_infra.providers.azure.dns import AzureDns
        from skreg_infra.providers.azure.storage import BlobObjectStore
        from skreg_infra.providers.azure.substrate import AksSubstrate

        config = self._config
        AksSubstrate("skreg-substrate", location=config.azure_location)

        needs_data_rg = (
            config.database_backend is DatabaseBackend.MANAGED
            or config.storage_backend is StorageBackend.MANAGED
        )
        data_rg: resources.ResourceGroup | None = None
        if needs_data_rg:
            data_rg = resources.ResourceGroup(
                "skreg-data-rg",
                resource_group_name=config.azure_resource_group,
                location=config.azure_location,
            )

        rg_opts = pulumi.ResourceOptions(depends_on=[data_rg] if data_rg else None)
        if config.database_backend is DatabaseBackend.MANAGED:
            database = AzurePgDatabase(
                "skreg-db",
                resource_group_name=config.azure_resource_group,
                location=config.azure_location,
                opts=rg_opts,
            )
            pulumi.export("db_secret_name", database.contract.dsn_secret_name)

        if config.storage_backend is StorageBackend.MANAGED:
            storage = BlobObjectStore(
                "skreg-storage",
                resource_group_name=config.azure_resource_group,
                location=config.azure_location,
                opts=rg_opts,
            )
            pulumi.export("storage_bucket", storage.contract.bucket)

        if self._managed_dns_target() is not None:
            if not config.azure_dns_zone:
                raise ValueError("SKREG_AZURE_DNS_ZONE is required for managed DNS on azure")
            AzureDns(
                "skreg-dns",
                domain_name=config.domain_name,
                resource_group=config.azure_resource_group,
                zone_name=config.azure_dns_zone,
                target=config.ingress_endpoint,
            )

    def _managed_dns_target(self) -> str | None:
        """The DNS target, or None when DNS is manual or no endpoint is known yet.

        The ingress endpoint (LoadBalancer hostname/IP) only exists after the
        ingress controller deploys into the new cluster, so a first deploy
        legitimately runs without DNS records.
        """
        config = self._config
        if config.dns_backend is not DnsBackend.MANAGED:
            return None
        if not config.ingress_endpoint:
            logger.info("managed_dns_skipped_no_ingress_endpoint")
            return None
        return config.ingress_endpoint
