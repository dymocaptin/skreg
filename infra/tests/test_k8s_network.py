from unittest.mock import MagicMock, patch

import pulumi
from pulumi.runtime import Mocks


class K8sMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", {**args.inputs, "name": args.name})

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(K8sMocks())

from skreg_infra.providers.k8s.network import K8sNetwork  # noqa: E402


def test_k8s_network_creates_traefik_release() -> None:
    with patch("pulumi_kubernetes.helm.v3.Release") as mock_release:
        mock_release.return_value = MagicMock()
        net = K8sNetwork.__new__(K8sNetwork)
        net._traefik_release = mock_release.return_value
        assert net._traefik_release is not None


def test_k8s_network_instantiates() -> None:
    net = K8sNetwork("test-net")
    assert net.outputs is not None
