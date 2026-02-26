"""AWS VPC implementation of SkillpkgNetwork."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws

from skreg_infra.components.network import NetworkOutputs

logger: logging.Logger = logging.getLogger(__name__)


class AwsNetwork(pulumi.ComponentResource):
    """AWS VPC + subnets + IGW + NAT Gateway satisfying ``SkillpkgNetwork``.

    Provisions 10.0.0.0/16 across us-west-2a and us-west-2b with two public
    subnets (ALB), two private subnets (ECS/RDS), one IGW, one NAT Gateway
    (single AZ, cost-optimised), and the corresponding route tables.
    """

    def __init__(
        self,
        name: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:Network", name, {}, opts)

        logger.debug("provisioning_aws_network", extra={"name": name})

        vpc = aws.ec2.Vpc(
            f"{name}-vpc",
            aws.ec2.VpcArgs(
                cidr_block="10.0.0.0/16",
                enable_dns_support=True,
                enable_dns_hostnames=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        pub_a = aws.ec2.Subnet(
            f"{name}-pub-a",
            aws.ec2.SubnetArgs(
                vpc_id=vpc.id,
                cidr_block="10.0.1.0/24",
                availability_zone="us-west-2a",
                map_public_ip_on_launch=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        pub_b = aws.ec2.Subnet(
            f"{name}-pub-b",
            aws.ec2.SubnetArgs(
                vpc_id=vpc.id,
                cidr_block="10.0.2.0/24",
                availability_zone="us-west-2b",
                map_public_ip_on_launch=True,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        priv_a = aws.ec2.Subnet(
            f"{name}-priv-a",
            aws.ec2.SubnetArgs(
                vpc_id=vpc.id,
                cidr_block="10.0.10.0/24",
                availability_zone="us-west-2a",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        priv_b = aws.ec2.Subnet(
            f"{name}-priv-b",
            aws.ec2.SubnetArgs(
                vpc_id=vpc.id,
                cidr_block="10.0.20.0/24",
                availability_zone="us-west-2b",
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        igw = aws.ec2.InternetGateway(
            f"{name}-igw",
            aws.ec2.InternetGatewayArgs(vpc_id=vpc.id),
            opts=pulumi.ResourceOptions(parent=self),
        )

        eip = aws.ec2.Eip(
            f"{name}-nat-eip",
            aws.ec2.EipArgs(domain="vpc"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        nat = aws.ec2.NatGateway(
            f"{name}-nat",
            aws.ec2.NatGatewayArgs(
                subnet_id=pub_a.id,
                allocation_id=eip.id,
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[igw]),
        )

        pub_rt = aws.ec2.RouteTable(
            f"{name}-pub-rt",
            aws.ec2.RouteTableArgs(
                vpc_id=vpc.id,
                routes=[
                    aws.ec2.RouteTableRouteArgs(
                        cidr_block="0.0.0.0/0",
                        gateway_id=igw.id,
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.ec2.RouteTableAssociation(
            f"{name}-pub-rta-a",
            aws.ec2.RouteTableAssociationArgs(subnet_id=pub_a.id, route_table_id=pub_rt.id),
            opts=pulumi.ResourceOptions(parent=self),
        )
        aws.ec2.RouteTableAssociation(
            f"{name}-pub-rta-b",
            aws.ec2.RouteTableAssociationArgs(subnet_id=pub_b.id, route_table_id=pub_rt.id),
            opts=pulumi.ResourceOptions(parent=self),
        )

        priv_rt = aws.ec2.RouteTable(
            f"{name}-priv-rt",
            aws.ec2.RouteTableArgs(
                vpc_id=vpc.id,
                routes=[
                    aws.ec2.RouteTableRouteArgs(
                        cidr_block="0.0.0.0/0",
                        nat_gateway_id=nat.id,
                    )
                ],
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.ec2.RouteTableAssociation(
            f"{name}-priv-rta-a",
            aws.ec2.RouteTableAssociationArgs(subnet_id=priv_a.id, route_table_id=priv_rt.id),
            opts=pulumi.ResourceOptions(parent=self),
        )
        aws.ec2.RouteTableAssociation(
            f"{name}-priv-rta-b",
            aws.ec2.RouteTableAssociationArgs(subnet_id=priv_b.id, route_table_id=priv_rt.id),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: NetworkOutputs = NetworkOutputs(
            vpc_id=vpc.id,
            public_subnet_ids=[pub_a.id, pub_b.id],
            private_subnet_ids=[priv_a.id, priv_b.id],
        )

        self.register_outputs(
            {
                "vpc_id": self._outputs.vpc_id,
                "public_subnet_ids": self._outputs.public_subnet_ids,
                "private_subnet_ids": self._outputs.private_subnet_ids,
            }
        )

    @property
    def outputs(self) -> NetworkOutputs:
        """Return the resolved network outputs."""
        return self._outputs
