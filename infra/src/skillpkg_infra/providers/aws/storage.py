"""AWS S3 + CloudFront object storage implementation of SkillpkgStorage."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws

from skillpkg_infra.components.storage import StorageOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsStorage(pulumi.ComponentResource):
    """AWS S3 + CloudFront implementation satisfying ``SkillpkgStorage``.

    Provisions a private S3 bucket for content-addressed package storage
    and a CloudFront distribution for immutable download serving.
    """

    def __init__(
        self,
        name: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the AWS storage component.

        Args:
            name: Logical Pulumi resource name.
            opts: Optional Pulumi resource options.
        """
        super().__init__("skillpkg:aws:Storage", name, {}, opts)

        logger.debug("provisioning_aws_storage", extra={"name": name})

        bucket = aws.s3.Bucket(
            f"{name}-bucket",
            aws.s3.BucketArgs(
                force_destroy=False,
                server_side_encryption_configuration=aws.s3.BucketServerSideEncryptionConfigurationArgs(
                    rule=aws.s3.BucketServerSideEncryptionConfigurationRuleArgs(
                        apply_server_side_encryption_by_default=aws.s3.BucketServerSideEncryptionConfigurationRuleApplyServerSideEncryptionByDefaultArgs(
                            sse_algorithm="AES256",
                        ),
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        service_secret = aws.secretsmanager.Secret(
            f"{name}-storage-credentials",
            opts=pulumi.ResourceOptions(parent=self),
        )

        distribution = aws.cloudfront.Distribution(
            f"{name}-cdn",
            aws.cloudfront.DistributionArgs(
                enabled=True,
                origins=[
                    aws.cloudfront.DistributionOriginArgs(
                        origin_id="s3-origin",
                        domain_name=bucket.bucket_regional_domain_name,
                    )
                ],
                default_cache_behavior=aws.cloudfront.DistributionDefaultCacheBehaviorArgs(
                    target_origin_id="s3-origin",
                    viewer_protocol_policy="redirect-to-https",
                    allowed_methods=["GET", "HEAD"],
                    cached_methods=["GET", "HEAD"],
                    forwarded_values=aws.cloudfront.DistributionDefaultCacheBehaviorForwardedValuesArgs(
                        query_string=False,
                        cookies=aws.cloudfront.DistributionDefaultCacheBehaviorForwardedValuesCookiesArgs(
                            forward="none",
                        ),
                    ),
                ),
                restrictions=aws.cloudfront.DistributionRestrictionsArgs(
                    geo_restriction=aws.cloudfront.DistributionRestrictionsGeoRestrictionArgs(
                        restriction_type="none",
                    ),
                ),
                viewer_certificate=aws.cloudfront.DistributionViewerCertificateArgs(
                    cloudfront_default_certificate=True,
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: StorageOutputs = StorageOutputs(
            bucket_name=bucket.bucket,
            cdn_base_url=distribution.domain_name.apply(lambda d: f"https://{d}"),
            service_account_secret_name=service_secret.name,
        )

        self.register_outputs(
            {
                "bucket_name": self._outputs.bucket_name,
                "cdn_base_url": self._outputs.cdn_base_url,
                "service_account_secret_name": self._outputs.service_account_secret_name,
            }
        )

    @property
    def outputs(self) -> StorageOutputs:
        """Return the resolved storage outputs."""
        return self._outputs
