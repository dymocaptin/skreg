"""Unit tests for the AWS storage component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    """Deterministic mock returning stable IDs for all resources."""

    def new_resource(
        self,
        args: pulumi.runtime.MockResourceArgs,
    ) -> tuple[str, dict[str, object]]:
        """Return a stable mock ID and echo inputs as outputs."""
        return (f"{args.name}-id", args.inputs)

    def call(
        self,
        args: pulumi.runtime.MockCallArgs,
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        """Return empty outputs for all provider function calls."""
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.storage import AwsStorage  # noqa: E402


@pulumi.runtime.test
def test_storage_cdn_url_starts_with_https() -> None:
    """AwsStorage CDN URL must begin with https://."""
    storage = AwsStorage("test-storage")

    def assert_https(url: str) -> None:
        assert url.startswith("https://"), f"Expected https://, got {url!r}"

    return storage.outputs.cdn_base_url.apply(assert_https)
