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

from skreg_infra.providers.k8s.dispatcher import K8sDispatcher  # noqa: E402


def test_dispatcher_stores_image() -> None:
    obj = K8sDispatcher.__new__(K8sDispatcher)
    obj._worker_image = "localhost:30500/skreg-worker:latest"
    assert "skreg-worker" in obj._worker_image


def test_dispatcher_instantiates() -> None:
    disp = K8sDispatcher(
        "test-dispatcher",
        worker_image="localhost:30500/skreg-worker:latest",
        s3_bucket="skreg",
        from_email="noreply@skreg.ai",
    )
    assert disp._worker_image == "localhost:30500/skreg-worker:latest"
