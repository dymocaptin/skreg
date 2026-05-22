"""AWS Aurora Serverless v2 PostgreSQL implementation of SkillpkgDatabase."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws
import pulumi_random as random

from skreg_infra.components.database import DatabaseOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsDatabaseArgs:
    """Arguments for the AWS Aurora Serverless v2 database component.

    Args:
        vpc_id: ID of the VPC in which to place the cluster.
        subnet_ids: Private subnet IDs for the DB subnet group.
        max_capacity: Maximum Aurora Capacity Units (ACUs). Defaults to 1.
        instance_class: Unused — kept for interface compatibility.
        multi_az: Unused — Aurora Serverless v2 manages AZ placement automatically.
    """

    def __init__(
        self,
        vpc_id: pulumi.Input[str],
        subnet_ids: list[pulumi.Input[str]],
        instance_class: pulumi.Input[str] = "db.t3.micro",
        multi_az: pulumi.Input[bool] = False,
        max_capacity: float = 1.0,
    ) -> None:
        """Initialise Aurora Serverless v2 database arguments."""
        self.vpc_id: pulumi.Input[str] = vpc_id
        self.subnet_ids: list[pulumi.Input[str]] = subnet_ids
        self.max_capacity: float = max_capacity


class AwsDatabase(pulumi.ComponentResource):
    """AWS Aurora Serverless v2 PostgreSQL component satisfying ``SkillpkgDatabase``.

    Provisions an encrypted Aurora Serverless v2 cluster that auto-pauses after
    ~5 minutes of inactivity (min_capacity=0). Provisions a Secrets Manager secret
    and a security group. Cold-start after idle is ~15-30 s.
    """

    def __init__(
        self,
        name: str,
        args: AwsDatabaseArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the Aurora Serverless v2 database component."""
        super().__init__("skreg:aws:Database", name, {}, opts)

        logger.debug("provisioning_aws_database", extra={"name": name})

        security_group = aws.ec2.SecurityGroup(
            f"{name}-sg",
            aws.ec2.SecurityGroupArgs(vpc_id=args.vpc_id),
            opts=pulumi.ResourceOptions(parent=self),
        )

        subnet_group = aws.rds.SubnetGroup(
            f"{name}-subnets",
            aws.rds.SubnetGroupArgs(subnet_ids=args.subnet_ids),
            opts=pulumi.ResourceOptions(parent=self),
        )

        db_password = random.RandomPassword(
            f"{name}-db-password-gen",
            random.RandomPasswordArgs(length=32, special=False),
            opts=pulumi.ResourceOptions(parent=self),
        )

        credentials_secret = aws.secretsmanager.Secret(
            f"{name}-db-password",
            opts=pulumi.ResourceOptions(parent=self),
        )

        cluster = aws.rds.Cluster(
            f"{name}-cluster",
            aws.rds.ClusterArgs(
                engine="aurora-postgresql",
                engine_version="16.6",
                database_name="skreg",
                master_username="skreg",
                master_password=db_password.result,
                db_subnet_group_name=subnet_group.name,
                vpc_security_group_ids=[security_group.id],
                serverlessv2_scaling_configuration=aws.rds.ClusterServerlessv2ScalingConfigurationArgs(
                    min_capacity=0,
                    max_capacity=args.max_capacity,
                ),
                storage_encrypted=True,
                skip_final_snapshot=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.rds.ClusterInstance(
            f"{name}-instance",
            aws.rds.ClusterInstanceArgs(
                cluster_identifier=cluster.id,
                instance_class="db.serverless",
                engine="aurora-postgresql",
                engine_version="16.6",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.secretsmanager.SecretVersion(
            f"{name}-db-password-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=credentials_secret.id,
                secret_string=pulumi.Output.all(
                    db_password.result, cluster.endpoint
                ).apply(
                    lambda vals: (
                        f"postgresql://skreg:{vals[0]}@{vals[1]}:5432/skreg?sslmode=require"
                    )
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: DatabaseOutputs = DatabaseOutputs(
            connection_secret_name=credentials_secret.name,
            connection_secret_arn=credentials_secret.arn,
            host=cluster.endpoint,
            port=pulumi.Output.from_input(5432),
            database_name=pulumi.Output.from_input("skreg"),
            security_group_id=security_group.id,
        )

        self.register_outputs(
            {
                "connection_secret_name": self._outputs.connection_secret_name,
                "connection_secret_arn": self._outputs.connection_secret_arn,
                "host": self._outputs.host,
                "port": self._outputs.port,
                "security_group_id": self._outputs.security_group_id,
                "database_name": self._outputs.database_name,
            }
        )

    @property
    def outputs(self) -> DatabaseOutputs:
        """Return the resolved database connection outputs."""
        return self._outputs
