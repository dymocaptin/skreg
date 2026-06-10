import pulumi
from pulumi.runtime import Mocks


class K8sMocks(Mocks):
    def new_resource(self, args: pulumi.runtime.MockResourceArgs) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", {**args.inputs, "name": args.name})

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(K8sMocks())

from skreg_infra.providers.k8s.database_cnpg import K8sCnpgDatabase  # noqa: E402


def test_cnpg_database_instantiates() -> None:
    db = K8sCnpgDatabase("test-db")
    assert db.outputs.secret_name == "skreg-db"


def test_cnpg_database_exposes_contract() -> None:
    from skreg_infra.contracts import DatabaseContract

    db = K8sCnpgDatabase("test-db-2")
    assert isinstance(db.contract, DatabaseContract)
    assert db.contract.dsn_secret_name == "skreg-db"
    assert db.contract.dsn_secret_key == "DATABASE_URL"
