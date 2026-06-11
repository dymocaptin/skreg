#!/usr/bin/env python3
"""Dynamic DNS updater: syncs the server's public IP to Route53 A records."""

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


def get_current_record(client: "Route53Client", zone_id: str, name: str) -> str | None:
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


def upsert_record(client: "Route53Client", zone_id: str, name: str, ip: str, ttl: int = 60) -> None:
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
