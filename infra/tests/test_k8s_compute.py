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

from skreg_infra.providers.k8s.compute import K8sCompute  # noqa: E402


def test_k8s_compute_stores_image() -> None:
    obj = K8sCompute.__new__(K8sCompute)
    obj._api_image = "localhost:30500/skreg-api:latest"
    assert "skreg-api" in obj._api_image


def test_k8s_compute_instantiates() -> None:
    compute = K8sCompute(
        "test-compute",
        api_image="localhost:30500/skreg-api:latest",
        worker_image="localhost:30500/skreg-worker:latest",
        s3_bucket="skreg",
        from_email="noreply@skreg.ai",
        domain_name="skreg.ai",
    )
    assert "skreg.ai" in compute.outputs.service_url
