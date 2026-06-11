"""skreg-dispatcher Deployment + RBAC for K8s Job creation."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sDispatcher(pulumi.ComponentResource):
    """Runs skreg-dispatcher; receives MinIO webhooks and spawns worker Jobs."""

    def __init__(
        self,
        name: str,
        worker_image: str,
        s3_bucket: str,
        from_email: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:k8s:Dispatcher", name, {}, opts)

        self._worker_image = worker_image
        labels = {"app": "skreg-dispatcher"}

        sa = k8s.core.v1.ServiceAccount(
            f"{name}-sa",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-dispatcher", namespace="skreg"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        role = k8s.rbac.v1.Role(
            f"{name}-role",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-dispatcher", namespace="skreg"),
            rules=[
                k8s.rbac.v1.PolicyRuleArgs(
                    api_groups=["batch"],
                    resources=["jobs"],
                    verbs=["create", "list", "get"],
                ),
            ],
            opts=pulumi.ResourceOptions(parent=self),
        )

        k8s.rbac.v1.RoleBinding(
            f"{name}-rb",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-dispatcher", namespace="skreg"),
            role_ref=k8s.rbac.v1.RoleRefArgs(
                api_group="rbac.authorization.k8s.io",
                kind="Role",
                name="skreg-dispatcher",
            ),
            subjects=[
                k8s.rbac.v1.SubjectArgs(
                    kind="ServiceAccount",
                    name="skreg-dispatcher",
                    namespace="skreg",
                )
            ],
            opts=pulumi.ResourceOptions(parent=self, depends_on=[sa, role]),
        )

        k8s.apps.v1.Deployment(
            f"{name}-deploy",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name="skreg-dispatcher",
                namespace="skreg",
                annotations={"pulumi.io/skipAwait": "true"},
            ),
            spec=k8s.apps.v1.DeploymentSpecArgs(
                replicas=1,
                selector=k8s.meta.v1.LabelSelectorArgs(match_labels=labels),
                template=k8s.core.v1.PodTemplateSpecArgs(
                    metadata=k8s.meta.v1.ObjectMetaArgs(labels=labels),
                    spec=k8s.core.v1.PodSpecArgs(
                        service_account_name="skreg-dispatcher",
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="dispatcher",
                                image=worker_image.replace("skreg-worker", "skreg-dispatcher"),
                                ports=[k8s.core.v1.ContainerPortArgs(container_port=9090)],
                                env=[
                                    k8s.core.v1.EnvVarArgs(name="KUBE_NAMESPACE", value="skreg"),
                                    k8s.core.v1.EnvVarArgs(name="WORKER_IMAGE", value=worker_image),
                                    k8s.core.v1.EnvVarArgs(name="S3_BUCKET", value=s3_bucket),
                                    k8s.core.v1.EnvVarArgs(name="FROM_EMAIL", value=from_email),
                                    k8s.core.v1.EnvVarArgs(
                                        name="PKI_SECRET_NAME", value="skreg-pki"
                                    ),
                                    k8s.core.v1.EnvVarArgs(name="DB_SECRET_NAME", value="skreg-db"),
                                    k8s.core.v1.EnvVarArgs(
                                        name="MINIO_SECRET_NAME", value="skreg-minio"
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="AWS_ENDPOINT_URL",
                                        value="http://skreg-storage-minio.skreg-infra.svc:9000",
                                    ),
                                    k8s.core.v1.EnvVarArgs(
                                        name="AWS_EC2_METADATA_DISABLED", value="true"
                                    ),
                                ],
                                liveness_probe=k8s.core.v1.ProbeArgs(
                                    http_get=k8s.core.v1.HTTPGetActionArgs(
                                        path="/healthz", port=9090
                                    ),
                                    period_seconds=30,
                                ),
                            )
                        ],
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self.register_outputs({})
