"""Unit tests for the AWS web hosting component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class WebHostingMocks(Mocks):
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
        """Return a minimal policy document for IAM data source calls."""
        if args.token == "aws:iam/getPolicyDocument:getPolicyDocument":
            return ({"json": "{}"}, [])
        return ({}, [])


pulumi.runtime.set_mocks(WebHostingMocks())

from skreg_infra.providers.aws.web_hosting import AwsWebHosting  # noqa: E402


@pulumi.runtime.test
def test_web_hosting_cdn_url_starts_with_https() -> None:
    """AwsWebHosting CDN URL must begin with https://."""
    hosting = AwsWebHosting("test-web", dist_dir="/nonexistent/dist")

    def assert_https(url: str) -> None:
        assert url.startswith("https://"), f"Expected https://, got {url!r}"

    return hosting.outputs.cdn_url.apply(assert_https)


@pulumi.runtime.test
def test_web_hosting_outputs_are_pulumi_outputs() -> None:
    """AwsWebHosting exposes both bucket_name and cdn_url as Pulumi Outputs."""
    import pulumi as _pulumi

    hosting = AwsWebHosting("test-web-outputs", dist_dir="/nonexistent/dist")
    assert isinstance(hosting.outputs.bucket_name, _pulumi.Output)
    assert isinstance(hosting.outputs.cdn_url, _pulumi.Output)
