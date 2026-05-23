"""AWS Lambda + S3 notification trigger for the skreg worker Fargate service."""

from __future__ import annotations

import json
import logging
from dataclasses import dataclass

import pulumi
import pulumi_aws as aws

logger: logging.Logger = logging.getLogger(__name__)

_HANDLER = """\
import logging
import os

import boto3

logger = logging.getLogger()
logger.setLevel(logging.INFO)


def handler(event, context):
    cluster_arn = os.environ["CLUSTER_ARN"]
    task_def_arn = os.environ["TASK_DEF_ARN"]
    subnet_ids = os.environ["SUBNET_IDS"].split(",")
    sg_id = os.environ["WORKER_SG_ID"]

    ecs = boto3.client("ecs")
    running = ecs.list_tasks(
        cluster=cluster_arn,
        family="skreg-worker",
        desiredStatus="RUNNING",
    )
    if running["taskArns"]:
        logger.info("worker_already_running")
        return

    ecs.run_task(
        cluster=cluster_arn,
        taskDefinition=task_def_arn,
        launchType="FARGATE",
        networkConfiguration={
            "awsvpcConfiguration": {
                "subnets": subnet_ids,
                "securityGroups": [sg_id],
                "assignPublicIp": "ENABLED",
            },
        },
        count=1,
    )
    logger.info("worker_task_started")
"""


@dataclass
class WorkerTriggerOutputs:
    """Resolved outputs from the provisioned worker trigger component."""

    lambda_arn: pulumi.Output[str]


class AwsWorkerTriggerArgs:
    """Arguments for the worker trigger component."""

    def __init__(
        self,
        cluster_arn: pulumi.Input[str],
        worker_task_def_arn: pulumi.Input[str],
        worker_task_role_arn: pulumi.Input[str],
        exec_role_arn: pulumi.Input[str],
        public_subnet_ids: list[pulumi.Input[str]],
        worker_sg_id: pulumi.Input[str],
        bucket_name: pulumi.Input[str],
        bucket_arn: pulumi.Input[str],
    ) -> None:
        self.cluster_arn = cluster_arn
        self.worker_task_def_arn = worker_task_def_arn
        self.worker_task_role_arn = worker_task_role_arn
        self.exec_role_arn = exec_role_arn
        self.public_subnet_ids = public_subnet_ids
        self.worker_sg_id = worker_sg_id
        self.bucket_name = bucket_name
        self.bucket_arn = bucket_arn


class AwsWorkerTrigger(pulumi.ComponentResource):
    """Triggers the skreg worker Fargate task when a .skill file lands in S3.

    Chain: S3 ObjectCreated (.skill suffix) → Lambda → ecs:RunTask.
    The Lambda checks for a running worker first to avoid duplicates.
    """

    def __init__(
        self,
        name: str,
        args: AwsWorkerTriggerArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:WorkerTrigger", name, {}, opts)

        logger.debug("provisioning_aws_worker_trigger", extra={"name": name})

        lambda_role = aws.iam.Role(
            f"{name}-lambda-role",
            aws.iam.RoleArgs(
                assume_role_policy=json.dumps(
                    {
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Principal": {"Service": "lambda.amazonaws.com"},
                                "Action": "sts:AssumeRole",
                            }
                        ],
                    }
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicyAttachment(
            f"{name}-lambda-basic",
            aws.iam.RolePolicyAttachmentArgs(
                role=lambda_role.name,
                policy_arn="arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicy(
            f"{name}-ecs-policy",
            aws.iam.RolePolicyArgs(
                role=lambda_role.name,
                policy=pulumi.Output.all(
                    pulumi.Output.from_input(args.worker_task_role_arn),
                    pulumi.Output.from_input(args.exec_role_arn),
                ).apply(
                    lambda vals: json.dumps(
                        {
                            "Version": "2012-10-17",
                            "Statement": [
                                {
                                    "Effect": "Allow",
                                    "Action": ["ecs:RunTask", "ecs:ListTasks"],
                                    "Resource": "*",
                                },
                                {
                                    "Effect": "Allow",
                                    "Action": "iam:PassRole",
                                    "Resource": [vals[0], vals[1]],
                                },
                            ],
                        }
                    )
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        subnet_ids_csv = pulumi.Output.all(
            *[pulumi.Output.from_input(s) for s in args.public_subnet_ids]
        ).apply(lambda ids: ",".join(ids))

        env_vars = pulumi.Output.all(
            pulumi.Output.from_input(args.cluster_arn),
            pulumi.Output.from_input(args.worker_task_def_arn),
            subnet_ids_csv,
            pulumi.Output.from_input(args.worker_sg_id),
        ).apply(
            lambda vals: {
                "CLUSTER_ARN": vals[0],
                "TASK_DEF_ARN": vals[1],
                "SUBNET_IDS": vals[2],
                "WORKER_SG_ID": vals[3],
            }
        )

        trigger_fn = aws.lambda_.Function(
            f"{name}-fn",
            aws.lambda_.FunctionArgs(
                runtime="python3.12",
                handler="index.handler",
                role=lambda_role.arn,
                code=pulumi.AssetArchive({"index.py": pulumi.StringAsset(_HANDLER)}),
                environment=aws.lambda_.FunctionEnvironmentArgs(variables=env_vars),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        s3_permission = aws.lambda_.Permission(
            f"{name}-s3-permission",
            aws.lambda_.PermissionArgs(
                action="lambda:InvokeFunction",
                function=trigger_fn.name,
                principal="s3.amazonaws.com",
                source_arn=args.bucket_arn,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        bucket_notification = aws.s3.BucketNotification(
            f"{name}-notification",
            aws.s3.BucketNotificationArgs(
                bucket=args.bucket_name,
                lambda_functions=[
                    aws.s3.BucketNotificationLambdaFunctionArgs(
                        lambda_function_arn=trigger_fn.arn,
                        events=["s3:ObjectCreated:*"],
                        filter_suffix=".skill",
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[s3_permission]),
        )

        # ensures BucketNotification is created before the component output resolves
        lambda_arn_output = pulumi.Output.all(trigger_fn.arn, bucket_notification.id).apply(
            lambda vals: vals[0]
        )

        self._outputs = WorkerTriggerOutputs(lambda_arn=lambda_arn_output)

        self.register_outputs({"lambda_arn": self._outputs.lambda_arn})

    @property
    def outputs(self) -> WorkerTriggerOutputs:
        """Return the resolved trigger outputs."""
        return self._outputs
