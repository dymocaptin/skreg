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
