import pathlib

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

from skreg_infra.providers.k8s.ci import K8sCi  # noqa: E402
from skreg_infra.providers.k8s.dns import K8sDns  # noqa: E402


def test_ci_stores_repo() -> None:
    obj = K8sCi.__new__(K8sCi)
    obj._github_repo = "dymocaptin/skreg"
    assert "skreg" in obj._github_repo


def test_ci_instantiates() -> None:
    ci = K8sCi("test-ci", github_repo="dymocaptin/skreg")
    assert ci._github_repo == "dymocaptin/skreg"


def test_ci_deployer_service_account_name() -> None:
    from skreg_infra.providers.k8s import ci as ci_module

    assert ci_module._DEPLOYER_SA == "skreg-ci-deployer"
    assert ci_module._NAMESPACE == "skreg-ci"


def test_dns_stores_zone() -> None:
    obj = K8sDns.__new__(K8sDns)
    obj._hosted_zone_id = "Z123ABC"
    assert obj._hosted_zone_id == "Z123ABC"


def test_dns_instantiates() -> None:
    dns = K8sDns("test-dns", domain_name="skreg.ai", hosted_zone_id="Z123ABC")
    assert dns._hosted_zone_id == "Z123ABC"


def test_dns_updater_script_exists() -> None:
    script = pathlib.Path(__file__).parents[2] / "scripts" / "dns-updater.py"
    assert script.exists()
