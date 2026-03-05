"""AWS S3 + CloudFront static web hosting for the skreg.ai landing page."""

from __future__ import annotations

import logging
import mimetypes
import os

import pulumi
import pulumi_aws as aws

logger: logging.Logger = logging.getLogger(__name__)

_DIST_DIR = os.path.join(
    os.path.dirname(__file__),
    "..",
    "..",
    "..",
    "..",
    "..",
    "..",
    "web",
    "dist",
)


class WebHostingOutputs:
    """Resolved outputs from the provisioned web hosting component."""

    def __init__(
        self,
        bucket_name: pulumi.Output[str],
        cdn_url: pulumi.Output[str],
    ) -> None:
        """Initialise web hosting outputs.

        Args:
            bucket_name: Name of the S3 bucket holding the built assets.
            cdn_url: HTTPS URL of the CloudFront distribution serving the site.
        """
        self.bucket_name: pulumi.Output[str] = bucket_name
        self.cdn_url: pulumi.Output[str] = cdn_url


class AwsWebHosting(pulumi.ComponentResource):
    """AWS S3 + CloudFront static site hosting for skreg.ai.

    Provisions a private S3 bucket (no website hosting), a CloudFront
    distribution using an Origin Access Control policy to serve assets over
    HTTPS, and uploads the contents of ``web/dist/`` to the bucket.
    """

    def __init__(
        self,
        name: str,
        dist_dir: str = _DIST_DIR,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the static web hosting component.

        Args:
            name: Logical Pulumi resource name.
            dist_dir: Filesystem path to the built ``web/dist/`` directory.
            opts: Optional Pulumi resource options.
        """
        super().__init__("skreg:aws:WebHosting", name, {}, opts)

        logger.debug("provisioning_aws_web_hosting", extra={"name": name})

        bucket = aws.s3.Bucket(
            f"{name}-bucket",
            aws.s3.BucketArgs(
                force_destroy=True,
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

        aws.s3.BucketPublicAccessBlock(
            f"{name}-public-access-block",
            aws.s3.BucketPublicAccessBlockArgs(
                bucket=bucket.id,
                block_public_acls=True,
                block_public_policy=True,
                ignore_public_acls=True,
                restrict_public_buckets=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        oac = aws.cloudfront.OriginAccessControl(
            f"{name}-oac",
            aws.cloudfront.OriginAccessControlArgs(
                origin_access_control_origin_type="s3",
                signing_behavior="always",
                signing_protocol="sigv4",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        distribution = aws.cloudfront.Distribution(
            f"{name}-cdn",
            aws.cloudfront.DistributionArgs(
                enabled=True,
                default_root_object="index.html",
                origins=[
                    aws.cloudfront.DistributionOriginArgs(
                        origin_id="s3-web-origin",
                        domain_name=bucket.bucket_regional_domain_name,
                        origin_access_control_id=oac.id,
                    )
                ],
                default_cache_behavior=aws.cloudfront.DistributionDefaultCacheBehaviorArgs(
                    target_origin_id="s3-web-origin",
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
                custom_error_responses=[
                    aws.cloudfront.DistributionCustomErrorResponseArgs(
                        error_code=403,
                        response_code=200,
                        response_page_path="/index.html",
                    ),
                    aws.cloudfront.DistributionCustomErrorResponseArgs(
                        error_code=404,
                        response_code=200,
                        response_page_path="/index.html",
                    ),
                ],
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

        bucket_policy: pulumi.Output[str] = pulumi.Output.all(
            dist_arn=distribution.arn, bucket_arn=bucket.arn
        ).apply(
            lambda args: aws.iam.get_policy_document(
                statements=[
                    aws.iam.GetPolicyDocumentStatementArgs(
                        principals=[
                            aws.iam.GetPolicyDocumentStatementPrincipalArgs(
                                type="Service",
                                identifiers=["cloudfront.amazonaws.com"],
                            )
                        ],
                        actions=["s3:GetObject"],
                        resources=[f"{args['bucket_arn']}/*"],
                        conditions=[
                            aws.iam.GetPolicyDocumentStatementConditionArgs(
                                test="StringEquals",
                                variable="AWS:SourceArn",
                                values=[args["dist_arn"]],
                            )
                        ],
                    )
                ]
            ).json
        )

        aws.s3.BucketPolicy(
            f"{name}-bucket-policy",
            aws.s3.BucketPolicyArgs(
                bucket=bucket.id,
                policy=bucket_policy,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._upload_dist(name, bucket, dist_dir)

        self._outputs = WebHostingOutputs(
            bucket_name=bucket.bucket,
            cdn_url=distribution.domain_name.apply(lambda d: f"https://{d}"),
        )

        self.register_outputs(
            {
                "bucket_name": self._outputs.bucket_name,
                "cdn_url": self._outputs.cdn_url,
            }
        )

    def _upload_dist(
        self,
        name: str,
        bucket: aws.s3.Bucket,
        dist_dir: str,
    ) -> None:
        """Upload all files from ``dist_dir`` to the S3 bucket.

        Args:
            name: Component resource name prefix.
            bucket: Target S3 bucket resource.
            dist_dir: Local path to the built ``web/dist/`` directory.
        """
        dist_dir = os.path.normpath(dist_dir)
        if not os.path.isdir(dist_dir):
            logger.warning("web_dist_dir_missing", extra={"dist_dir": dist_dir})
            return

        for root, _dirs, files in os.walk(dist_dir):
            for filename in files:
                abs_path = os.path.join(root, filename)
                rel_path = os.path.relpath(abs_path, dist_dir)
                # Normalise to forward slashes for S3 keys on all platforms.
                s3_key = rel_path.replace(os.sep, "/")
                content_type, _ = mimetypes.guess_type(abs_path)
                content_type = content_type or "application/octet-stream"

                safe_resource_name = f"{name}-asset-{s3_key.replace('/', '-').replace('.', '-')}"

                aws.s3.BucketObject(
                    safe_resource_name,
                    aws.s3.BucketObjectArgs(
                        bucket=bucket.id,
                        key=s3_key,
                        source=pulumi.FileAsset(abs_path),
                        content_type=content_type,
                    ),
                    opts=pulumi.ResourceOptions(parent=self),
                )

    @property
    def outputs(self) -> WebHostingOutputs:
        """Return the resolved web hosting outputs."""
        return self._outputs
