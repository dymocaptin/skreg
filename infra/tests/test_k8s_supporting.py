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

from skreg_infra.providers.k8s.email import K8sEmail  # noqa: E402
from skreg_infra.providers.k8s.pki import K8sPki  # noqa: E402
from skreg_infra.providers.k8s.registry import K8sRegistry  # noqa: E402


def test_pki_secret_name() -> None:
    obj = K8sPki.__new__(K8sPki)
    obj._secret_name = "skreg-pki"
    assert obj._secret_name == "skreg-pki"


def test_pki_instantiates() -> None:
    pki = K8sPki("test-pki")
    assert pki.outputs.secret_name == "skreg-pki"


def test_email_service_name() -> None:
    obj = K8sEmail.__new__(K8sEmail)
    obj._smtp_host = "postfix.skreg.svc.cluster.local"
    assert "postfix" in obj._smtp_host


def test_email_instantiates() -> None:
    email = K8sEmail("test-email")
    assert email.outputs.smtp_port == 25
    assert "postfix" in email.outputs.smtp_host


def test_registry_url() -> None:
    obj = K8sRegistry.__new__(K8sRegistry)
    obj._registry_url = "localhost:30500"
    assert "30500" in obj._registry_url


def test_registry_instantiates() -> None:
    reg = K8sRegistry("test-registry")
    assert reg.outputs.registry_url == "localhost:30500"
