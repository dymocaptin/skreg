"""Postfix SMTP relay Deployment in the skreg namespace."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sEmailOutputs:
    def __init__(self, smtp_host: str, smtp_port: int) -> None:
        self.smtp_host = smtp_host
        self.smtp_port = smtp_port


class K8sEmail(pulumi.ComponentResource):
    """Runs `boky/postfix` as a Deployment, providing SMTP relay on port 25.

    The cluster's network blocks outbound port 25, so postfix cannot deliver
    directly to recipient MX servers. It must forward all mail through an
    upstream smarthost on a submission port (587). The smarthost address and
    SASL credentials are supplied via the ``skreg-smtp-relay`` Secret
    (keys ``relayhost``, ``username``, ``password``); see
    ``scripts/create-smtp-relay-secret.sh``.
    """

    RELAY_SECRET_NAME = "skreg-smtp-relay"  # noqa: S105

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Email", name, {}, opts)

        self._smtp_host = "postfix.skreg.svc.cluster.local"
        self._smtp_port = 25

        labels = {"app": "postfix"}

        def _relay_secret(key: str) -> k8s.core.v1.EnvVarSourceArgs:
            return k8s.core.v1.EnvVarSourceArgs(
                secret_key_ref=k8s.core.v1.SecretKeySelectorArgs(
                    name=self.RELAY_SECRET_NAME, key=key
                )
            )

        deploy = k8s.apps.v1.Deployment(
            f"{name}-deploy",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="postfix", namespace="skreg"),
            spec=k8s.apps.v1.DeploymentSpecArgs(
                replicas=1,
                selector=k8s.meta.v1.LabelSelectorArgs(match_labels=labels),
                template=k8s.core.v1.PodTemplateSpecArgs(
                    metadata=k8s.meta.v1.ObjectMetaArgs(labels=labels),
                    spec=k8s.core.v1.PodSpecArgs(
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="postfix",
                                image="boky/postfix:latest",
                                ports=[k8s.core.v1.ContainerPortArgs(container_port=25)],
                                env=[
                                    k8s.core.v1.EnvVarArgs(
                                        name="ALLOWED_SENDER_DOMAINS", value="skreg.ai"
                                    ),
                                    k8s.core.v1.EnvVarArgs(name="HOSTNAME", value="skreg.ai"),
                                    # Forward all outbound mail through an upstream
                                    # smarthost on the submission port (587) — the
                                    # cluster cannot reach recipient MX servers on 25.
                                    k8s.core.v1.EnvVarArgs(
                                        name="RELAYHOST",
                                        value_from=_relay_secret("relayhost"),
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="RELAYHOST_USERNAME",
                                        value_from=_relay_secret("username"),
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="RELAYHOST_PASSWORD",
                                        value_from=_relay_secret("password"),
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="RELAYHOST_TLS_LEVEL", value="encrypt"
                                    ),
                                ],
                            )
                        ]
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        k8s.core.v1.Service(
            f"{name}-svc",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="postfix", namespace="skreg"),
            spec=k8s.core.v1.ServiceSpecArgs(
                selector=labels,
                ports=[k8s.core.v1.ServicePortArgs(port=25, target_port=25)],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[deploy]),
        )

        self._outputs = K8sEmailOutputs(smtp_host=self._smtp_host, smtp_port=self._smtp_port)
        self.register_outputs({"smtp_host": self._smtp_host, "smtp_port": self._smtp_port})

    @property
    def outputs(self) -> K8sEmailOutputs:
        return self._outputs
