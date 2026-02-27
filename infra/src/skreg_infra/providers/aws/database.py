"""AWS RDS PostgreSQL implementation of SkillpkgDatabase."""

from __future__ import annotations

import json
import logging

import pulumi
import pulumi_aws as aws
import pulumi_random as random

from skreg_infra.components.database import DatabaseOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsDatabaseArgs:
    """Arguments for the AWS RDS database component.

    Args:
        vpc_id: ID of the VPC in which to place the RDS instance.
        subnet_ids: Private subnet IDs for the DB subnet group.
        instance_class: RDS instance class.
        multi_az: Enable Multi-AZ deployment.
    """

    def __init__(
        self,
        vpc_id: pulumi.Input[str],
        subnet_ids: list[pulumi.Input[str]],
        instance_class: pulumi.Input[str] = "db.t3.micro",
        multi_az: pulumi.Input[bool] = False,
    ) -> None:
        """Initialise RDS database arguments."""
        self.vpc_id: pulumi.Input[str] = vpc_id
        self.subnet_ids: list[pulumi.Input[str]] = subnet_ids
        self.instance_class: pulumi.Input[str] = instance_class
        self.multi_az: pulumi.Input[bool] = multi_az


class AwsDatabase(pulumi.ComponentResource):
    """AWS RDS PostgreSQL component satisfying ``SkillpkgDatabase``.

    Provisions an encrypted RDS instance, a Secrets Manager secret,
    a security group, and a DB subnet group.
    """

    def __init__(
        self,
        name: str,
        args: AwsDatabaseArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """Initialise and provision the AWS database component.

        Args:
            name: Logical Pulumi resource name.
            args: Validated AWS-specific database arguments.
            opts: Optional Pulumi resource options.
        """
        super().__init__(
            "skreg:aws:Database",
            name,
            {},
            pulumi.ResourceOptions.merge(
                opts,
                pulumi.ResourceOptions(aliases=[pulumi.Alias(type_="skillpkg:aws:Database")]),
            ),
        )

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

        aws.secretsmanager.SecretVersion(
            f"{name}-db-password-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=credentials_secret.id,
                secret_string=db_password.result.apply(
                    lambda pw: json.dumps({"username": "skreg", "password": pw})
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        instance = aws.rds.Instance(
            f"{name}-rds",
            aws.rds.InstanceArgs(
                engine="postgres",
                engine_version="16",
                instance_class=args.instance_class,
                allocated_storage=20,
                storage_encrypted=True,
                db_name="skreg",
                username="skreg",
                password=db_password.result,
                multi_az=args.multi_az,
                db_subnet_group_name=subnet_group.name,
                vpc_security_group_ids=[security_group.id],
                skip_final_snapshot=False,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: DatabaseOutputs = DatabaseOutputs(
            connection_secret_name=credentials_secret.name,
            connection_secret_arn=credentials_secret.arn,
            host=instance.address,
            port=pulumi.Output.from_input(5432),
            database_name=pulumi.Output.from_input("skreg"),
        )

        self.register_outputs(
            {
                "connection_secret_name": self._outputs.connection_secret_name,
                "connection_secret_arn": self._outputs.connection_secret_arn,
                "host": self._outputs.host,
                "port": self._outputs.port,
                "database_name": self._outputs.database_name,
            }
        )

    @property
    def outputs(self) -> DatabaseOutputs:
        """Return the resolved database connection outputs."""
        return self._outputs
