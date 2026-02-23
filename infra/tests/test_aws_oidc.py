"""Unit tests for the AWS OIDC component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs = dict(args.inputs)
        outputs["arn"] = f"arn:aws:iam::123456789012:oidc-provider/{args.name}"
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.oidc import AwsOidc  # noqa: E402


@pulumi.runtime.test
def test_oidc_role_arn_is_set() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    oidc = AwsOidc("test-oidc", github_repo="dymocaptin/skreg")

    def check(arn: str) -> None:
        assert arn

    return oidc.outputs.role_arn.apply(check)


@pulumi.runtime.test
def test_oidc_role_arn_different_repo() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    oidc = AwsOidc("test-oidc2", github_repo="org/other-repo")

    def check(arn: str) -> None:
        assert arn

    return oidc.outputs.role_arn.apply(check)
