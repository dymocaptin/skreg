"""Unit tests for the AWS compute component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs = dict(args.inputs)
        # Inject computed attributes that the real AWS provider would populate.
        outputs.setdefault("dns_name", f"{args.name}.example.com")
        outputs.setdefault("name", args.name)
        outputs.setdefault("repository_url", f"123456789.dkr.ecr.us-west-2.amazonaws.com/{args.name}")
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.compute import AwsCompute, AwsComputeArgs  # noqa: E402


def _args() -> AwsComputeArgs:
    return AwsComputeArgs(
        vpc_id="vpc-test",
        public_subnet_ids=["subnet-pub-1", "subnet-pub-2"],
        private_subnet_ids=["subnet-priv-1", "subnet-priv-2"],
        db_secret_arn="arn:aws:secretsmanager:us-west-2:123456789:secret:test-db",
    )


@pulumi.runtime.test
def test_compute_service_url_starts_with_http() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    compute = AwsCompute("test-cmp", _args())

    def check(url: str) -> None:
        assert url.startswith("http://"), url

    return compute.outputs.service_url.apply(check)


@pulumi.runtime.test
def test_compute_worker_service_name_is_set() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    compute = AwsCompute("test-cmp2", _args())

    def check(name: str) -> None:
        assert name

    return compute.outputs.worker_service_name.apply(check)


@pulumi.runtime.test
def test_compute_ecr_api_repo_is_set() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    compute = AwsCompute("test-cmp3", _args())

    def check(url: str) -> None:
        assert url

    return compute.ecr_api_repo.apply(check)


@pulumi.runtime.test
def test_compute_ecr_worker_repo_is_set() -> None:
    pulumi.runtime.set_mocks(SkillpkgMocks())
    compute = AwsCompute("test-cmp4", _args())

    def check(url: str) -> None:
        assert url

    return compute.ecr_worker_repo.apply(check)
