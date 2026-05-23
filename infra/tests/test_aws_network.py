"""Unit tests for the AWS network component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skreg_infra.providers.aws.network import AwsNetwork  # noqa: E402


def test_network_public_subnet_count() -> None:
    net = AwsNetwork("test-net")
    assert len(net.outputs.public_subnet_ids) == 2


def test_network_private_subnet_count() -> None:
    net = AwsNetwork("test-net2")
    assert len(net.outputs.private_subnet_ids) == 2


@pulumi.runtime.test
def test_network_vpc_id_is_set() -> None:
    net = AwsNetwork("test-net3")

    def check(vpc_id: str) -> None:
        assert vpc_id

    return net.outputs.vpc_id.apply(check)


@pulumi.runtime.test
def test_network_first_public_subnet_is_set() -> None:
    net = AwsNetwork("test-net4")

    def check(subnet_id: str) -> None:
        assert subnet_id

    return net.outputs.public_subnet_ids[0].apply(check)


@pulumi.runtime.test
def test_network_first_private_subnet_is_set() -> None:
    net = AwsNetwork("test-net5")

    def check(subnet_id: str) -> None:
        assert subnet_id

    return net.outputs.private_subnet_ids[0].apply(check)


class _CapturingNetworkMocks(SkillpkgMocks):
    """Records resource types created during network provisioning."""

    def __init__(self) -> None:
        super().__init__()
        self.resource_types: list[str] = []

    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        self.resource_types.append(args.typ)
        return super().new_resource(args)


def test_network_has_no_nat_gateway() -> None:
    """Verify NAT Gateway is not created (ECS tasks use public subnets)."""
    mocks = _CapturingNetworkMocks()
    pulumi.runtime.set_mocks(mocks)
    AwsNetwork("test-no-nat")
    assert "aws:ec2/natGateway:NatGateway" not in mocks.resource_types, (
        "Network must not create a NAT Gateway — ECS tasks run in public subnets"
    )
