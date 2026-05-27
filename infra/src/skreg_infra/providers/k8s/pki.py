"""PKI: reads the manually-created skreg-pki Secret (does not provision it)."""
from __future__ import annotations

import pulumi


class K8sPkiOutputs:
    def __init__(self, secret_name: str) -> None:
        self.secret_name = secret_name


class K8sPki(pulumi.ComponentResource):
    """References the pre-existing `skreg-pki` Secret in the `skreg` namespace.

    Create it manually once:
        kubectl create secret generic skreg-pki --namespace skreg \\
          --from-literal=PUBLISHER_CA_KEY_PEM="$(cat publisher-ca.key)" \\
          --from-literal=PUBLISHER_CA_CERT_PEM="$(cat publisher-ca.pem)" \\
          --from-literal=REGISTRY_CA_KEY_PEM="$(cat registry-ca.key)" \\
          --from-literal=ROOT_CA_CERT_PEM="$(cat root-ca.pem)"
    """

    SECRET_NAME = "skreg-pki"  # noqa: S105
    NAMESPACE = "skreg"

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Pki", name, {}, opts)
        self._secret_name = self.SECRET_NAME
        self._outputs = K8sPkiOutputs(secret_name=self._secret_name)
        self.register_outputs({"secret_name": self._secret_name})

    @property
    def outputs(self) -> K8sPkiOutputs:
        return self._outputs
