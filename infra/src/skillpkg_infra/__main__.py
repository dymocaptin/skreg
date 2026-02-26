"""Pulumi stack entry point for skreg infrastructure."""

from __future__ import annotations

import logging

import pulumi
import structlog

from skillpkg_infra.config import CloudProvider, StackConfig
from skillpkg_infra.providers.aws.compute import AwsCompute, AwsComputeArgs
from skillpkg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs
from skillpkg_infra.providers.aws.network import AwsNetwork
from skillpkg_infra.providers.aws.oidc import AwsOidc
from skillpkg_infra.providers.aws.pki import AwsPki, AwsPkiArgs
from skillpkg_infra.providers.aws.storage import AwsStorage

logger: logging.Logger = logging.getLogger(__name__)


class SkillpkgStack:
    """Orchestrates all provider-agnostic infrastructure components."""

    def __init__(self, config: StackConfig) -> None:
        """Initialise the stack with resolved configuration."""
        self._config: StackConfig = config

    def run(self) -> None:
        """Provision the full infrastructure stack."""
        logger.info(
            "stack_run_started",
            extra={"cloud_provider": self._config.cloud_provider.value},
        )
        if self._config.cloud_provider == CloudProvider.AWS:
            self._run_aws()
        else:
            raise NotImplementedError(
                f"Provider '{self._config.cloud_provider}' not yet implemented."
            )

    def _run_aws(self) -> None:
        config = self._config

        network = AwsNetwork("skreg-network")
        storage = AwsStorage("skreg-storage")
        pki = AwsPki("skreg-pki", AwsPkiArgs(bucket_name=storage.outputs.bucket_name))
        database = AwsDatabase(
            "skreg-db",
            AwsDatabaseArgs(
                vpc_id=network.outputs.vpc_id,
                subnet_ids=list(network.outputs.private_subnet_ids),
                multi_az=config.multi_az,
            ),
        )
        compute = AwsCompute(
            "skreg-compute",
            AwsComputeArgs(
                vpc_id=network.outputs.vpc_id,
                public_subnet_ids=list(network.outputs.public_subnet_ids),
                private_subnet_ids=list(network.outputs.private_subnet_ids),
                db_secret_arn=database.outputs.connection_secret_arn,
                api_image_uri=config.api_image_uri,
                worker_image_uri=config.worker_image_uri,
                domain_name=config.domain_name,
            ),
        )
        oidc = AwsOidc("skreg-oidc", github_repo="dymocaptin/skreg")

        pulumi.export(
            "api_url",
            (
                pulumi.Output.from_input(f"https://{config.domain_name}")
                if config.domain_name
                else compute.outputs.service_url
            ),
        )
        pulumi.export("alb_dns_name", compute.outputs.alb_dns_name)
        pulumi.export("cert_validation_cname", compute.outputs.cert_validation_cname)
        pulumi.export("cdn_base_url", storage.outputs.cdn_base_url)
        pulumi.export("root_ca_cert", pki.root_ca_cert_pem)
        pulumi.export("ecr_api_repo", compute.ecr_api_repo)
        pulumi.export("ecr_worker_repo", compute.ecr_worker_repo)
        pulumi.export("oidc_role_arn", oidc.outputs.role_arn)
        pulumi.export("deploy_role_arn", oidc.outputs.deploy_role_arn)


if __name__ == "__main__":
    structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
    SkillpkgStack(config=StackConfig.load()).run()
