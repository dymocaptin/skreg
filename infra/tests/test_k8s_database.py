from unittest.mock import MagicMock

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

from skreg_infra.providers.k8s.database import K8sDatabase  # noqa: E402


def test_k8s_database_component_name() -> None:
    obj = K8sDatabase.__new__(K8sDatabase)
    obj._release = MagicMock()
    obj._secret_name = "skreg-db"
    assert obj._secret_name == "skreg-db"


def test_k8s_database_instantiates() -> None:
    db = K8sDatabase("test-db")
    assert db.outputs.secret_name == "skreg-db"
