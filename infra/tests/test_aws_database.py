"""Unit tests for the AWS database component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks

from skreg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs


class SkregMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = dict(args.inputs)
        outputs["arn"] = f"arn:aws:secretsmanager:us-east-1:123456789012:secret:{args.name}-id"
        outputs["name"] = args.name
        if args.typ == "aws:rds/cluster:Cluster":
            outputs["endpoint"] = "test.cluster-abc.us-west-2.rds.amazonaws.com"
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkregMocks(), preview=False)


@pulumi.runtime.test
def test_database_port_is_5432() -> None:
    pulumi.runtime.set_mocks(SkregMocks(), preview=False)
    db = AwsDatabase("test-db", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_port(port: int) -> None:
        assert port == 5432

    return db.outputs.port.apply(assert_port)


@pulumi.runtime.test
def test_database_name_is_skreg() -> None:
    pulumi.runtime.set_mocks(SkregMocks(), preview=False)
    db = AwsDatabase("test-db2", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_name(name: str) -> None:
        assert name == "skreg"

    return db.outputs.database_name.apply(assert_name)


@pulumi.runtime.test
def test_database_connection_secret_arn_is_set() -> None:
    pulumi.runtime.set_mocks(SkregMocks(), preview=False)
    db = AwsDatabase("test-db3", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_arn(arn: str) -> None:
        assert arn, f"Expected non-empty connection_secret_arn, got {arn!r}"

    return db.outputs.connection_secret_arn.apply(assert_arn)


class _CapturingMocks(SkregMocks):
    def __init__(self) -> None:
        self.clusters: dict[str, dict[str, object]] = {}

    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        resource_id, outputs = super().new_resource(args)
        if args.typ == "aws:rds/cluster:Cluster":
            self.clusters[args.name] = dict(args.inputs)
        return resource_id, outputs


@pulumi.runtime.test
def test_database_aurora_auto_pause_configured() -> None:
    mocks = _CapturingMocks()
    pulumi.runtime.set_mocks(mocks, preview=False)
    db = AwsDatabase("test-db4", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def check(_: str) -> None:
        cluster = next(
            (inputs for name, inputs in mocks.clusters.items() if "test-db4" in name),
            None,
        )
        assert cluster is not None, "Expected an aws:rds/cluster:Cluster resource"
        scaling = cluster.get("serverlessv2ScalingConfiguration")
        assert scaling is not None, "Expected serverlessv2ScalingConfiguration to be set"
        assert scaling["minCapacity"] == 0, (
            f"Expected minCapacity=0 for auto-pause, got {scaling['minCapacity']}"
        )

    return db.outputs.connection_secret_arn.apply(check)
