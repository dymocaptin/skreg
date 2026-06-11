"""Managed RDS PostgreSQL 16 database.

Provisions:
- DB subnet group (requires caller to supply subnet IDs from EksSubstrate VPC)
- Security group allowing TCP 5432 from a source security group or CIDR
- RDS PostgreSQL 16 instance (db.t4g.micro, 20 GiB, not publicly accessible)
- Random master password via pulumi_random
- Kubernetes Secret ``skreg-db`` (namespace ``skreg``) with key ``DATABASE_URL``

Yields DatabaseContract(dsn_secret_name="skreg-db", dsn_secret_key="DATABASE_URL").
"""

from __future__ import annotations

from collections.abc import Sequence

import pulumi
import pulumi_aws as aws
import pulumi_kubernetes as k8s
import pulumi_random as random

from skreg_infra.contracts import DatabaseContract

_SECRET_NAME = "skreg-db"  # noqa: S105
_DB_KEY = "DATABASE_URL"  # noqa: S105
_NAMESPACE = "skreg"
_DB_NAME = "skreg"
_DB_USER = "skreg"
_ENGINE_VERSION = "16"
_INSTANCE_CLASS = "db.t4g.micro"
_ALLOCATED_STORAGE = 20


class RdsDatabase(pulumi.ComponentResource):
    """RDS PostgreSQL 16 + Kubernetes secret for the DSN.

    Args:
        name: Pulumi resource name prefix.
        vpc_id: VPC in which to create the DB subnet group and security group.
        subnet_ids: At least two subnet IDs (different AZs) for the DB subnet group.
        source_sg_id: Security group ID from which to allow TCP 5432. Pass the
            EKS node group SG.  Mutually exclusive with ``source_cidr``.
        source_cidr: CIDR block from which to allow TCP 5432 (fallback when no
            source SG is available, e.g. in unit tests).
        opts: Pulumi resource options.
    """

    def __init__(
        self,
        name: str,
        vpc_id: pulumi.Input[str],
        subnet_ids: Sequence[pulumi.Input[str]],
        source_sg_id: str | None = None,
        source_cidr: str | None = None,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:RdsDatabase", name, {}, opts)
        child_opts = pulumi.ResourceOptions(parent=self)

        if source_sg_id is None and source_cidr is None:
            raise ValueError("Provide source_sg_id or source_cidr")

        # ── Random master password ────────────────────────────────────────────
        password = random.RandomPassword(
            f"{name}-password",
            length=32,
            special=False,
            opts=child_opts,
        )

        # ── Security group ────────────────────────────────────────────────────
        sg = aws.ec2.SecurityGroup(
            f"{name}-sg",
            vpc_id=vpc_id,
            description=f"RDS PostgreSQL access for {name}",
            opts=child_opts,
        )

        if source_sg_id is not None:
            aws.ec2.SecurityGroupRule(
                f"{name}-sg-ingress",
                type="ingress",
                from_port=5432,
                to_port=5432,
                protocol="tcp",
                security_group_id=sg.id,
                source_security_group_id=source_sg_id,
                opts=child_opts,
            )
        else:
            aws.ec2.SecurityGroupRule(
                f"{name}-sg-ingress-cidr",
                type="ingress",
                from_port=5432,
                to_port=5432,
                protocol="tcp",
                security_group_id=sg.id,
                cidr_blocks=[source_cidr or "0.0.0.0/0"],
                opts=child_opts,
            )

        # Allow all egress
        aws.ec2.SecurityGroupRule(
            f"{name}-sg-egress",
            type="egress",
            from_port=0,
            to_port=0,
            protocol="-1",
            security_group_id=sg.id,
            cidr_blocks=["0.0.0.0/0"],
            opts=child_opts,
        )

        # ── DB subnet group ───────────────────────────────────────────────────
        subnet_group = aws.rds.SubnetGroup(
            f"{name}-subnet-group",
            subnet_ids=subnet_ids,
            tags={"Name": f"{name}-subnet-group"},
            opts=child_opts,
        )

        # ── RDS instance ──────────────────────────────────────────────────────
        db_instance = aws.rds.Instance(
            f"{name}-instance",
            engine="postgres",
            engine_version=_ENGINE_VERSION,
            instance_class=_INSTANCE_CLASS,
            allocated_storage=_ALLOCATED_STORAGE,
            db_name=_DB_NAME,
            username=_DB_USER,
            password=password.result,
            db_subnet_group_name=subnet_group.name,
            vpc_security_group_ids=[sg.id],
            publicly_accessible=False,
            skip_final_snapshot=True,
            tags={"Name": f"{name}-instance"},
            opts=child_opts,
        )

        # ── Kubernetes Secret ─────────────────────────────────────────────────
        dsn = pulumi.Output.all(
            db_instance.endpoint,
            password.result,
        ).apply(lambda args: f"postgresql://{_DB_USER}:{args[1]}@{args[0]}/{_DB_NAME}")

        k8s.core.v1.Secret(
            f"{name}-k8s-secret",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_SECRET_NAME,
                namespace=_NAMESPACE,
            ),
            string_data={_DB_KEY: dsn},
            opts=child_opts,
        )

        self._contract = DatabaseContract(
            dsn_secret_name=_SECRET_NAME,
            dsn_secret_key=_DB_KEY,
        )
        self.register_outputs({"dsn_secret_name": _SECRET_NAME, "dsn_secret_key": _DB_KEY})

    @property
    def contract(self) -> DatabaseContract:
        return self._contract
