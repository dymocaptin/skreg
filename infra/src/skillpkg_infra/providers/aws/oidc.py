"""AWS IAM OIDC provider + ECR push role for GitHub Actions."""

from __future__ import annotations

import json
import logging

import pulumi
import pulumi_aws as aws

logger: logging.Logger = logging.getLogger(__name__)

_GITHUB_THUMBPRINTS: list[str] = [
    "6938fd4d98bab03faadb97b34396831e3780aea1",
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd",
]


class AwsOidcOutputs:
    """Resolved outputs from the OIDC component."""

    def __init__(
        self,
        role_arn: pulumi.Output[str],
        deploy_role_arn: pulumi.Output[str],
    ) -> None:
        self.role_arn: pulumi.Output[str] = role_arn
        self.deploy_role_arn: pulumi.Output[str] = deploy_role_arn


class AwsOidc(pulumi.ComponentResource):
    """GitHub Actions OIDC identity provider + least-privilege ECR push role.

    Trust policy constrains assumption to pushes from ``github_repo`` on ``main`` only.
    """

    def __init__(
        self,
        name: str,
        github_repo: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skillpkg:aws:Oidc", name, {}, opts)

        logger.debug("provisioning_aws_oidc", extra={"name": name, "repo": github_repo})

        provider = aws.iam.OpenIdConnectProvider(
            f"{name}-gh-oidc",
            aws.iam.OpenIdConnectProviderArgs(
                url="https://token.actions.githubusercontent.com",
                client_id_lists=["sts.amazonaws.com"],
                thumbprint_lists=_GITHUB_THUMBPRINTS,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        trust_policy = provider.arn.apply(
            lambda arn: json.dumps(
                {
                    "Version": "2012-10-17",
                    "Statement": [
                        {
                            "Effect": "Allow",
                            "Principal": {"Federated": arn},
                            "Action": "sts:AssumeRoleWithWebIdentity",
                            "Condition": {
                                "StringEquals": {
                                    "token.actions.githubusercontent.com:aud": "sts.amazonaws.com",
                                    "token.actions.githubusercontent.com:sub": (
                                        f"repo:{github_repo}:ref:refs/heads/main"
                                    ),
                                }
                            },
                        }
                    ],
                }
            )
        )

        role = aws.iam.Role(
            f"{name}-gh-role",
            aws.iam.RoleArgs(assume_role_policy=trust_policy),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicy(
            f"{name}-gh-policy",
            aws.iam.RolePolicyArgs(
                role=role.name,
                policy=json.dumps(
                    {
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Action": ["ecr:GetAuthorizationToken"],
                                "Resource": "*",
                            },
                            {
                                "Effect": "Allow",
                                "Action": [
                                    "ecr:BatchCheckLayerAvailability",
                                    "ecr:PutImage",
                                    "ecr:InitiateLayerUpload",
                                    "ecr:UploadLayerPart",
                                    "ecr:CompleteLayerUpload",
                                ],
                                "Resource": "arn:aws:ecr:us-west-2:*:repository/skreg-*",
                            },
                        ],
                    }
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        deploy_role = aws.iam.Role(
            f"{name}-deploy-role",
            aws.iam.RoleArgs(assume_role_policy=trust_policy),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicyAttachment(
            f"{name}-deploy-policy",
            aws.iam.RolePolicyAttachmentArgs(
                role=deploy_role.name,
                policy_arn="arn:aws:iam::aws:policy/AdministratorAccess",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: AwsOidcOutputs = AwsOidcOutputs(
            role_arn=role.arn,
            deploy_role_arn=deploy_role.arn,
        )
        self.register_outputs({
            "role_arn": self._outputs.role_arn,
            "deploy_role_arn": self._outputs.deploy_role_arn,
        })

    @property
    def outputs(self) -> AwsOidcOutputs:
        """Return the resolved OIDC outputs."""
        return self._outputs
