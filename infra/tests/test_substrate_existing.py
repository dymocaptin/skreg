from skreg_infra.contracts import SubstrateContract
from skreg_infra.providers.substrate.existing import ExistingSubstrate


def test_existing_substrate_yields_empty_kubeconfig() -> None:
    sub = ExistingSubstrate(ingress_endpoint="50.53.217.195")
    assert isinstance(sub.contract, SubstrateContract)
    assert sub.contract.kubeconfig == ""
    assert sub.contract.ingress_endpoint == "50.53.217.195"
