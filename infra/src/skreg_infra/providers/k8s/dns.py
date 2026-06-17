"""Route53 dynamic DNS CronJob + IAM credentials Secret reference."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s

# The dynamic-DNS updater previously lived at scripts/dns-updater.py. It is
# inlined here so the provider carries no external file dependency; the script
# is delivered to the CronJob via the ConfigMap below and run with
# `python /scripts/dns_updater.py`.
_DNS_UPDATER_SCRIPT = """#!/usr/bin/env python3
# Dynamic DNS updater: syncs the server's public IP to Route53 A records.
import json
import os
import sys
import urllib.request
from typing import TYPE_CHECKING

import boto3

if TYPE_CHECKING:
    from mypy_boto3_route53 import Route53Client


def get_public_ip() -> str:
    with urllib.request.urlopen("https://api4.ipify.org?format=json", timeout=10) as r:
        return str(json.loads(r.read())["ip"])


def get_current_record(
    client: "Route53Client", zone_id: str, name: str
) -> str | None:
    resp = client.list_resource_record_sets(
        HostedZoneId=zone_id,
        StartRecordName=name,
        StartRecordType="A",
        MaxItems="1",
    )
    for rrs in resp.get("ResourceRecordSets", []):
        if rrs["Name"].rstrip(".") == name.rstrip(".") and rrs["Type"] == "A":
            return str(rrs["ResourceRecords"][0]["Value"])
    return None


def upsert_record(
    client: "Route53Client", zone_id: str, name: str, ip: str, ttl: int = 60
) -> None:
    client.change_resource_record_sets(
        HostedZoneId=zone_id,
        ChangeBatch={
            "Changes": [
                {
                    "Action": "UPSERT",
                    "ResourceRecordSet": {
                        "Name": name,
                        "Type": "A",
                        "TTL": ttl,
                        "ResourceRecords": [{"Value": ip}],
                    },
                }
            ]
        },
    )


def main() -> None:
    zone_id = os.environ["HOSTED_ZONE_ID"]
    domain = os.environ["DOMAIN_NAME"]
    names = [domain, f"api.{domain}"]

    ip = get_public_ip()
    client: Route53Client = boto3.client("route53")

    for name in names:
        current = get_current_record(client, zone_id, name)
        if current == ip:
            print(f"no_change name={name} ip={ip}")
        else:
            upsert_record(client, zone_id, name, ip)
            print(f"updated name={name} old={current} new={ip}")

    sys.exit(0)


if __name__ == "__main__":
    main()
"""


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
            metadata=k8s.meta.v1.ObjectMetaArgs(name="dns-updater-script", namespace="skreg-infra"),
            data={"dns_updater.py": _DNS_UPDATER_SCRIPT},
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
