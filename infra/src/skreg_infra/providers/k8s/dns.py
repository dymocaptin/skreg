"""Route53 dynamic DNS CronJob + IAM credentials Secret reference."""
from __future__ import annotations

import pathlib

import pulumi
import pulumi_kubernetes as k8s

_SCRIPT_PATH = pathlib.Path(__file__).parents[5] / "scripts" / "dns-updater.py"


class K8sDns(pulumi.ComponentResource):
    """CronJob that checks public IP every 5 min and updates Route53 A records.

    Pre-create the IAM credentials secret:
        kubectl create secret generic route53-creds --namespace skreg-infra \\
          --from-literal=AWS_ACCESS_KEY_ID=<key> \\
          --from-literal=AWS_SECRET_ACCESS_KEY=<secret>
    """

    def __init__(
        self,
        name: str,
        domain_name: str,
        hosted_zone_id: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:k8s:Dns", name, {}, opts)

        self._hosted_zone_id = hosted_zone_id

        script_cm = k8s.core.v1.ConfigMap(
            f"{name}-script",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name="dns-updater-script", namespace="skreg-infra"
            ),
            data={"dns_updater.py": _SCRIPT_PATH.read_text()},
            opts=pulumi.ResourceOptions(parent=self),
        )

        k8s.batch.v1.CronJob(
            f"{name}-cronjob",
            metadata=k8s.meta.v1.ObjectMetaArgs(name="dns-updater", namespace="skreg-infra"),
            spec=k8s.batch.v1.CronJobSpecArgs(
                schedule="*/5 * * * *",
                concurrency_policy="Forbid",
                job_template=k8s.batch.v1.JobTemplateSpecArgs(
                    spec=k8s.batch.v1.JobSpecArgs(
                        backoff_limit=1,
                        ttl_seconds_after_finished=300,
                        template=k8s.core.v1.PodTemplateSpecArgs(
                            spec=k8s.core.v1.PodSpecArgs(
                                restart_policy="Never",
                                containers=[
                                    k8s.core.v1.ContainerArgs(
                                        name="dns-updater",
                                        image="python:3.12-slim",
                                        command=[
                                            "sh",
                                            "-c",
                                            (
                                                "pip install boto3 -q"
                                                " && python /scripts/dns_updater.py"
                                            ),
                                        ],
                                        env=[
                                            k8s.core.v1.EnvVarArgs(
                                                name="HOSTED_ZONE_ID", value=hosted_zone_id
                                            ),
                                            k8s.core.v1.EnvVarArgs(
                                                name="DOMAIN_NAME", value=domain_name
                                            ),
                                            k8s.core.v1.EnvVarArgs(
                                                name="AWS_DEFAULT_REGION", value="us-east-1"
                                            ),
                                        ],
                                        env_from=[
                                            k8s.core.v1.EnvFromSourceArgs(
                                                secret_ref=k8s.core.v1.SecretEnvSourceArgs(
                                                    name="route53-creds"
                                                )
                                            )
                                        ],
                                        volume_mounts=[
                                            k8s.core.v1.VolumeMountArgs(
                                                name="script", mount_path="/scripts"
                                            )
                                        ],
                                    )
                                ],
                                volumes=[
                                    k8s.core.v1.VolumeArgs(
                                        name="script",
                                        config_map=k8s.core.v1.ConfigMapVolumeSourceArgs(
                                            name="dns-updater-script"
                                        ),
                                    )
                                ],
                            )
                        ),
                    )
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[script_cm]),
        )

        self.register_outputs({"hosted_zone_id": hosted_zone_id})
