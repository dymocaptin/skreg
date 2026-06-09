"""skreg-api Deployment + Traefik IngressRoute."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s


class K8sComputeOutputs:
    def __init__(self, service_url: str) -> None:
        self.service_url = service_url


class K8sCompute(pulumi.ComponentResource):
    """Deploys skreg-api and wires Traefik IngressRoutes for api.skreg.ai and skreg.ai."""

    def __init__(
        self,
        name: str,
        api_image: str,
        worker_image: str,
        s3_bucket: str,
        from_email: str,
        domain_name: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:k8s:Compute", name, {}, opts)

        self._api_image = api_image
        labels = {"app": "skreg-api"}

        deploy = k8s.apps.v1.Deployment(
            f"{name}-api-deploy",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name="skreg-api",
                namespace="skreg",
                annotations={"pulumi.io/skipAwait": "true"},
            ),
            spec=k8s.apps.v1.DeploymentSpecArgs(
                replicas=1,
                selector=k8s.meta.v1.LabelSelectorArgs(match_labels=labels),
                template=k8s.core.v1.PodTemplateSpecArgs(
                    metadata=k8s.meta.v1.ObjectMetaArgs(labels=labels),
                    spec=k8s.core.v1.PodSpecArgs(
                        containers=[
                            k8s.core.v1.ContainerArgs(
                                name="skreg-api",
                                image=api_image,
                                ports=[k8s.core.v1.ContainerPortArgs(container_port=8080)],
                                env=[
                                    k8s.core.v1.EnvVarArgs(name="S3_BUCKET", value=s3_bucket),
                                    k8s.core.v1.EnvVarArgs(name="FROM_EMAIL", value=from_email),
                                    k8s.core.v1.EnvVarArgs(
                                        name="AWS_ENDPOINT_URL",
                                        value="http://skreg-storage-minio.skreg-infra.svc:9000",
                                    ),
                                    k8s.core.v1.EnvVarArgs(name="AWS_REGION", value="us-east-1"),
                                    k8s.core.v1.EnvVarArgs(
                                        name="AWS_EC2_METADATA_DISABLED", value="true"
                                    ),
                                ],
                                env_from=[
                                    k8s.core.v1.EnvFromSourceArgs(
                                        secret_ref=k8s.core.v1.SecretEnvSourceArgs(name="skreg-pki")
                                    ),
                                    k8s.core.v1.EnvFromSourceArgs(
                                        secret_ref=k8s.core.v1.SecretEnvSourceArgs(
                                            name="skreg-minio"
                                        )
                                    ),
                                    k8s.core.v1.EnvFromSourceArgs(
                                        secret_ref=k8s.core.v1.SecretEnvSourceArgs(name="skreg-db")
                                    ),
                                ],
                                liveness_probe=k8s.core.v1.ProbeArgs(
                                    http_get=k8s.core.v1.HTTPGetActionArgs(
                                        path="/healthz", port=8080
                                    ),
                                    initial_delay_seconds=5,
                                    period_seconds=15,
                                ),
                            )
                        ]
                    ),
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        svc = k8s.core.v1.Service(
            f"{name}-api-svc",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-api", namespace="skreg"),
            spec=k8s.core.v1.ServiceSpecArgs(
                selector=labels,
                ports=[k8s.core.v1.ServicePortArgs(port=8080, target_port=8080)],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[deploy]),
        )

        k8s.apiextensions.CustomResource(
            f"{name}-api-ingress",
            api_version="traefik.io/v1alpha1",
            kind="IngressRoute",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-api", namespace="skreg"),
            spec={
                "entryPoints": ["websecure"],
                "routes": [
                    {
                        "match": f"Host(`{domain_name}`) || Host(`api.{domain_name}`)",
                        "kind": "Rule",
                        "services": [{"name": "skreg-api", "port": 8080}],
                    }
                ],
                "tls": {"certResolver": "letsencrypt"},
            },
            opts=pulumi.ResourceOptions(parent=self, depends_on=[svc]),
        )

        k8s.apiextensions.CustomResource(
            f"{name}-api-ingress-http",
            api_version="traefik.io/v1alpha1",
            kind="IngressRoute",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="skreg-api-http", namespace="skreg"),
            spec={
                "entryPoints": ["web"],
                "routes": [
                    {
                        "match": f"Host(`{domain_name}`) || Host(`api.{domain_name}`)",
                        "kind": "Rule",
                        "middlewares": [{"name": "redirect-https", "namespace": "skreg-infra"}],
                        "services": [{"name": "skreg-api", "port": 8080}],
                    }
                ],
            },
            opts=pulumi.ResourceOptions(parent=self, depends_on=[svc]),
        )

        k8s.apiextensions.CustomResource(
            f"{name}-redirect-mw",
            api_version="traefik.io/v1alpha1",
            kind="Middleware",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="redirect-https", namespace="skreg-infra"),
            spec={"redirectScheme": {"scheme": "https", "permanent": True}},
            opts=pulumi.ResourceOptions(parent=self),
        )

        # suppress unused parameter — worker_image is passed for potential future use
        _ = worker_image

        self._outputs = K8sComputeOutputs(service_url=f"https://{domain_name}")
        self.register_outputs({"service_url": f"https://{domain_name}"})

    @property
    def outputs(self) -> K8sComputeOutputs:
        return self._outputs
