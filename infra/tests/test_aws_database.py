"""Unit tests for the AWS database component using Pulumi mocks."""
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

from skillpkg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs  # noqa: E402


@pulumi.runtime.test
def test_database_port_is_5432() -> None:
    """AwsDatabase outputs must expose PostgreSQL port 5432."""
    db = AwsDatabase(
        "test-db",
        AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]),
    )

    def assert_port(port: int) -> None:
        assert port == 5432, f"Expected 5432, got {port}"

    return db.outputs.port.apply(assert_port)


@pulumi.runtime.test
def test_database_name_is_skillpkg() -> None:
    """AwsDatabase outputs must use 'skillpkg' as the database name."""
    db = AwsDatabase(
        "test-db2",
        AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]),
    )

    def assert_name(name: str) -> None:
        assert name == "skillpkg", f"Expected 'skillpkg', got {name}"

    return db.outputs.database_name.apply(assert_name)
