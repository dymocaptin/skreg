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

from skreg_infra.providers.k8s.storage import K8sStorage  # noqa: E402


def test_k8s_storage_bucket_name() -> None:
    obj = K8sStorage.__new__(K8sStorage)
    obj._bucket_name = "skreg"
    assert obj._bucket_name == "skreg"


def test_k8s_storage_instantiates() -> None:
    storage = K8sStorage("test-storage")
    assert storage.outputs.bucket_name == "skreg"
    assert storage.outputs.endpoint == K8sStorage.ENDPOINT
