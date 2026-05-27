"""Postfix SMTP relay Deployment in the skreg namespace."""
from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sEmailOutputs:
    def __init__(self, smtp_host: str, smtp_port: int) -> None:
        self.smtp_host = smtp_host
        self.smtp_port = smtp_port


class K8sEmail(pulumi.ComponentResource):
    """Runs `boky/postfix` as a Deployment, providing SMTP relay on port 25."""

    def __init__(self, name: str, opts: pulumi.ResourceOptions | None = None) -> None:
        super().__init__("skreg:k8s:Email", name, {}, opts)

        self._smtp_host = "postfix.skreg.svc.cluster.local"
        self._smtp_port = 25

        labels = {"app": "postfix"}

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
