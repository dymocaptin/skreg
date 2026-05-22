"""Unit tests for the AWS storage component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self,
        args: pulumi.runtime.MockResourceArgs,
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = dict(args.inputs)
        outputs["arn"] = f"arn:aws:s3:::{args.name}"
        return (f"{args.name}-id", outputs)

    def call(
        self,
        args: pulumi.runtime.MockCallArgs,
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skreg_infra.providers.aws.storage import AwsStorage  # noqa: E402


@pulumi.runtime.test
def test_storage_cdn_url_starts_with_https() -> None:
    """AwsStorage CDN URL must begin with https://."""
    storage = AwsStorage("test-storage")

    def assert_https(url: str) -> None:
        assert url.startswith("https://"), f"Expected https://, got {url!r}"

    return storage.outputs.cdn_base_url.apply(assert_https)


@pulumi.runtime.test
def test_storage_bucket_arn_is_set() -> None:
    """AwsStorage must expose bucket_arn as a non-empty Output."""
    storage = AwsStorage("test-storage-arn")

    def assert_arn(arn: str) -> None:
        assert arn, f"Expected non-empty bucket_arn, got {arn!r}"

    return storage.bucket_arn.apply(assert_arn)
