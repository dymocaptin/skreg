"""AWS ECS Fargate + ALB implementation of SkillpkgCompute."""

from __future__ import annotations

import json
import logging

import pulumi
import pulumi_aws as aws

from skillpkg_infra.components.compute import ComputeOutputs

logger: logging.Logger = logging.getLogger(__name__)

_FALLBACK_IMAGE = "public.ecr.aws/amazonlinux/amazonlinux:2023"


class AwsComputeArgs:
    """Arguments for the AWS ECS Fargate compute component."""

    def __init__(
        self,
        vpc_id: pulumi.Input[str],
        public_subnet_ids: list[pulumi.Input[str]],
        private_subnet_ids: list[pulumi.Input[str]],
        db_secret_arn: pulumi.Input[str],
        api_image_uri: str = "",
        worker_image_uri: str = "",
    ) -> None:
        self.vpc_id: pulumi.Input[str] = vpc_id
        self.public_subnet_ids: list[pulumi.Input[str]] = public_subnet_ids
        self.private_subnet_ids: list[pulumi.Input[str]] = private_subnet_ids
        self.db_secret_arn: pulumi.Input[str] = db_secret_arn
        self.api_image_uri: str = api_image_uri or _FALLBACK_IMAGE
        self.worker_image_uri: str = worker_image_uri or _FALLBACK_IMAGE


class AwsCompute(pulumi.ComponentResource):
    """AWS ECS Fargate + ALB satisfying ``SkillpkgCompute``.

    Provisions ECR repositories, ECS cluster, IAM execution role,
    CloudWatch log groups, security groups, ALB, and two Fargate services.
    """

    def __init__(
        self,
        name: str,
        args: AwsComputeArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skillpkg:aws:Compute", name, {}, opts)

        logger.debug("provisioning_aws_compute", extra={"name": name})

        api_repo = aws.ecr.Repository(
            f"{name}-ecr-api",
            aws.ecr.RepositoryArgs(name="skreg-api", image_tag_mutability="MUTABLE"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        worker_repo = aws.ecr.Repository(
            f"{name}-ecr-worker",
            aws.ecr.RepositoryArgs(name="skreg-worker", image_tag_mutability="MUTABLE"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        cluster = aws.ecs.Cluster(
            f"{name}-cluster",
            opts=pulumi.ResourceOptions(parent=self),
        )

        exec_role = aws.iam.Role(
            f"{name}-exec-role",
            aws.iam.RoleArgs(
                assume_role_policy=json.dumps(
                    {
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Principal": {"Service": "ecs-tasks.amazonaws.com"},
                                "Action": "sts:AssumeRole",
                            }
                        ],
                    }
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicyAttachment(
            f"{name}-exec-policy",
            aws.iam.RolePolicyAttachmentArgs(
                role=exec_role.name,
                policy_arn="arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.cloudwatch.LogGroup(
            f"{name}-api-logs",
            aws.cloudwatch.LogGroupArgs(name="/ecs/skreg-api", retention_in_days=30),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.cloudwatch.LogGroup(
            f"{name}-worker-logs",
            aws.cloudwatch.LogGroupArgs(name="/ecs/skreg-worker", retention_in_days=30),
            opts=pulumi.ResourceOptions(parent=self),
        )

        alb_sg = aws.ec2.SecurityGroup(
            f"{name}-alb-sg",
            aws.ec2.SecurityGroupArgs(
                vpc_id=args.vpc_id,
                ingress=[
                    aws.ec2.SecurityGroupIngressArgs(
                        protocol="tcp", from_port=80, to_port=80, cidr_blocks=["0.0.0.0/0"]
                    ),
                    aws.ec2.SecurityGroupIngressArgs(
                        protocol="tcp", from_port=443, to_port=443, cidr_blocks=["0.0.0.0/0"]
                    ),
                ],
                egress=[
                    aws.ec2.SecurityGroupEgressArgs(
                        protocol="-1", from_port=0, to_port=0, cidr_blocks=["0.0.0.0/0"]
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        api_sg = aws.ec2.SecurityGroup(
            f"{name}-api-sg",
            aws.ec2.SecurityGroupArgs(
                vpc_id=args.vpc_id,
                ingress=[
                    aws.ec2.SecurityGroupIngressArgs(
                        protocol="tcp",
                        from_port=8080,
                        to_port=8080,
                        security_groups=[alb_sg.id],
                    )
                ],
                egress=[
                    aws.ec2.SecurityGroupEgressArgs(
                        protocol="-1", from_port=0, to_port=0, cidr_blocks=["0.0.0.0/0"]
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        worker_sg = aws.ec2.SecurityGroup(
            f"{name}-worker-sg",
            aws.ec2.SecurityGroupArgs(
                vpc_id=args.vpc_id,
                egress=[
                    aws.ec2.SecurityGroupEgressArgs(
                        protocol="-1", from_port=0, to_port=0, cidr_blocks=["0.0.0.0/0"]
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        alb = aws.lb.LoadBalancer(
            f"{name}-alb",
            aws.lb.LoadBalancerArgs(
                load_balancer_type="application",
                internal=False,
                security_groups=[alb_sg.id],
                subnets=args.public_subnet_ids,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        tg = aws.lb.TargetGroup(
            f"{name}-tg",
            aws.lb.TargetGroupArgs(
                port=8080,
                protocol="HTTP",
                target_type="ip",
                vpc_id=args.vpc_id,
                health_check=aws.lb.TargetGroupHealthCheckArgs(path="/healthz"),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        listener = aws.lb.Listener(
            f"{name}-listener",
            aws.lb.ListenerArgs(
                load_balancer_arn=alb.arn,
                port=80,
                protocol="HTTP",
                default_actions=[
                    aws.lb.ListenerDefaultActionArgs(type="forward", target_group_arn=tg.arn)
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        api_image = args.api_image_uri
        api_container_defs = pulumi.Output.from_input(args.db_secret_arn).apply(
            lambda arn: json.dumps(
                [
                    {
                        "name": "skreg-api",
                        "image": api_image,
                        "portMappings": [{"containerPort": 8080, "protocol": "tcp"}],
                        "environment": [{"name": "BIND_ADDR", "value": "0.0.0.0:8080"}],
                        "secrets": [{"name": "DATABASE_URL", "valueFrom": arn}],
                        "logConfiguration": {
                            "logDriver": "awslogs",
                            "options": {
                                "awslogs-group": "/ecs/skreg-api",
                                "awslogs-region": "us-west-2",
                                "awslogs-stream-prefix": "ecs",
                            },
                        },
                    }
                ]
            )
        )

        api_task = aws.ecs.TaskDefinition(
            f"{name}-api-task",
            aws.ecs.TaskDefinitionArgs(
                family="skreg-api",
                cpu="512",
                memory="1024",
                network_mode="awsvpc",
                requires_compatibilities=["FARGATE"],
                execution_role_arn=exec_role.arn,
                container_definitions=api_container_defs,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        worker_image = args.worker_image_uri
        worker_container_defs = pulumi.Output.from_input(args.db_secret_arn).apply(
            lambda arn: json.dumps(
                [
                    {
                        "name": "skreg-worker",
                        "image": worker_image,
                        "secrets": [{"name": "DATABASE_URL", "valueFrom": arn}],
                        "logConfiguration": {
                            "logDriver": "awslogs",
                            "options": {
                                "awslogs-group": "/ecs/skreg-worker",
                                "awslogs-region": "us-west-2",
                                "awslogs-stream-prefix": "ecs",
                            },
                        },
                    }
                ]
            )
        )

        worker_task = aws.ecs.TaskDefinition(
            f"{name}-worker-task",
            aws.ecs.TaskDefinitionArgs(
                family="skreg-worker",
                cpu="256",
                memory="512",
                network_mode="awsvpc",
                requires_compatibilities=["FARGATE"],
                execution_role_arn=exec_role.arn,
                container_definitions=worker_container_defs,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.ecs.Service(
            f"{name}-api-svc",
            aws.ecs.ServiceArgs(
                cluster=cluster.arn,
                task_definition=api_task.arn,
                launch_type="FARGATE",
                desired_count=1,
                network_configuration=aws.ecs.ServiceNetworkConfigurationArgs(
                    subnets=args.private_subnet_ids,
                    security_groups=[api_sg.id],
                ),
                load_balancers=[
                    aws.ecs.ServiceLoadBalancerArgs(
                        target_group_arn=tg.arn,
                        container_name="skreg-api",
                        container_port=8080,
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[listener]),
        )

        worker_svc = aws.ecs.Service(
            f"{name}-worker-svc",
            aws.ecs.ServiceArgs(
                cluster=cluster.arn,
                task_definition=worker_task.arn,
                launch_type="FARGATE",
                desired_count=1,
                network_configuration=aws.ecs.ServiceNetworkConfigurationArgs(
                    subnets=args.private_subnet_ids,
                    security_groups=[worker_sg.id],
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: ComputeOutputs = ComputeOutputs(
            service_url=alb.dns_name.apply(lambda d: f"http://{d}"),
            worker_service_name=worker_svc.name,
        )
        self.ecr_api_repo: pulumi.Output[str] = api_repo.repository_url
        self.ecr_worker_repo: pulumi.Output[str] = worker_repo.repository_url

        self.register_outputs(
            {
                "service_url": self._outputs.service_url,
                "worker_service_name": self._outputs.worker_service_name,
                "ecr_api_repo": self.ecr_api_repo,
                "ecr_worker_repo": self.ecr_worker_repo,
            }
        )

    @property
    def outputs(self) -> ComputeOutputs:
        """Return the resolved compute outputs."""
        return self._outputs
