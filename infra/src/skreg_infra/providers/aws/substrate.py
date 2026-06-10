"""EKS substrate: VPC, EKS cluster, managed node group, OIDC provider for IRSA.

Yields a SubstrateContract. Because the EKS-generated kubeconfig is a Pulumi
Output[str], it cannot be embedded in the frozen SubstrateContract dataclass at
construction time. Instead:
  - contract.kubeconfig is set to "" (empty string sentinel)
  - the actual kubeconfig is exported as a Pulumi stack output named "kubeconfig"
    (accessible via ``self.kubeconfig``)

contract.ingress_endpoint is also "" at construction time — the NLB hostname is
only known after Traefik's LoadBalancer Service is applied by the in-cluster
stack. Wire the NLB hostname into DnsContract after the k8s stack deploys.
"""

from __future__ import annotations

import json

import pulumi
import pulumi_aws as aws

from skreg_infra.contracts import SubstrateContract

_NODE_GROUP_INSTANCE_TYPE = "t3.large"
_NODE_GROUP_DESIRED = 2
_NODE_GROUP_MIN = 1
_NODE_GROUP_MAX = 4


class EksSubstrate(pulumi.ComponentResource):
    """VPC + EKS cluster (raw pulumi_aws resources, no pulumi_eks wrapper).

    Resources created:
    - VPC with 2 public subnets across 2 AZs (v1 simplification; private subnets
      + NAT gateway is a follow-up hardening step)
    - Internet gateway + route table
    - EKS control plane
    - IAM roles for the control plane and node group
    - Managed node group (t3.large × 2)
    - OIDC provider for IRSA

    The kubeconfig Output is stored on ``self.kubeconfig`` for export. Pass it
    to ``pulumi_kubernetes.Provider`` in __main__ to target the EKS cluster.
    """

    def __init__(
        self,
        name: str,
        region: str = "us-west-2",
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:aws:EksSubstrate", name, {}, opts)
        child_opts = pulumi.ResourceOptions(parent=self)

        # ── VPC ──────────────────────────────────────────────────────────────
        vpc = aws.ec2.Vpc(
            f"{name}-vpc",
            cidr_block="10.0.0.0/16",
            enable_dns_support=True,
            enable_dns_hostnames=True,
            tags={"Name": f"{name}-vpc"},
            opts=child_opts,
        )

        igw = aws.ec2.InternetGateway(
            f"{name}-igw",
            vpc_id=vpc.id,
            tags={"Name": f"{name}-igw"},
            opts=child_opts,
        )

        subnet_a = aws.ec2.Subnet(
            f"{name}-subnet-a",
            vpc_id=vpc.id,
            cidr_block="10.0.1.0/24",
            availability_zone=f"{region}a",
            map_public_ip_on_launch=True,
            tags={
                "Name": f"{name}-subnet-a",
                "kubernetes.io/role/elb": "1",
            },
            opts=child_opts,
        )

        subnet_b = aws.ec2.Subnet(
            f"{name}-subnet-b",
            vpc_id=vpc.id,
            cidr_block="10.0.2.0/24",
            availability_zone=f"{region}b",
            map_public_ip_on_launch=True,
            tags={
                "Name": f"{name}-subnet-b",
                "kubernetes.io/role/elb": "1",
            },
            opts=child_opts,
        )

        route_table = aws.ec2.RouteTable(
            f"{name}-rt",
            vpc_id=vpc.id,
            routes=[
                aws.ec2.RouteTableRouteArgs(
                    cidr_block="0.0.0.0/0",
                    gateway_id=igw.id,
                )
            ],
            tags={"Name": f"{name}-rt"},
            opts=child_opts,
        )

        aws.ec2.RouteTableAssociation(
            f"{name}-rta-a",
            subnet_id=subnet_a.id,
            route_table_id=route_table.id,
            opts=child_opts,
        )

        aws.ec2.RouteTableAssociation(
            f"{name}-rta-b",
            subnet_id=subnet_b.id,
            route_table_id=route_table.id,
            opts=child_opts,
        )

        # ── IAM: EKS cluster role ─────────────────────────────────────────────
        cluster_role = aws.iam.Role(
            f"{name}-cluster-role",
            assume_role_policy=json.dumps(
                {
                    "Version": "2012-10-17",
                    "Statement": [
                        {
                            "Effect": "Allow",
                            "Principal": {"Service": "eks.amazonaws.com"},
                            "Action": "sts:AssumeRole",
                        }
                    ],
                }
            ),
            opts=child_opts,
        )

        aws.iam.RolePolicyAttachment(
            f"{name}-cluster-policy",
            role=cluster_role.name,
            policy_arn="arn:aws:iam::aws:policy/AmazonEKSClusterPolicy",
            opts=child_opts,
        )

        # ── EKS control plane ─────────────────────────────────────────────────
        cluster = aws.eks.Cluster(
            f"{name}-cluster",
            role_arn=cluster_role.arn,
            vpc_config=aws.eks.ClusterVpcConfigArgs(
                subnet_ids=[subnet_a.id, subnet_b.id],
                endpoint_public_access=True,
            ),
            version="1.30",
            tags={"Name": f"{name}-cluster"},
            opts=child_opts,
        )

        # ── IAM: node group role ──────────────────────────────────────────────
        node_role = aws.iam.Role(
            f"{name}-node-role",
            assume_role_policy=json.dumps(
                {
                    "Version": "2012-10-17",
                    "Statement": [
                        {
                            "Effect": "Allow",
                            "Principal": {"Service": "ec2.amazonaws.com"},
                            "Action": "sts:AssumeRole",
                        }
                    ],
                }
            ),
            opts=child_opts,
        )

        for policy_arn in [
            "arn:aws:iam::aws:policy/AmazonEKSWorkerNodePolicy",
            "arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy",
            "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly",
        ]:
            short = policy_arn.split("/")[-1]
            aws.iam.RolePolicyAttachment(
                f"{name}-node-{short}",
                role=node_role.name,
                policy_arn=policy_arn,
                opts=child_opts,
            )

        # ── Managed node group ────────────────────────────────────────────────
        aws.eks.NodeGroup(
            f"{name}-nodes",
            cluster_name=cluster.name,
            node_role_arn=node_role.arn,
            subnet_ids=[subnet_a.id, subnet_b.id],
            instance_types=[_NODE_GROUP_INSTANCE_TYPE],
            scaling_config=aws.eks.NodeGroupScalingConfigArgs(
                desired_size=_NODE_GROUP_DESIRED,
                min_size=_NODE_GROUP_MIN,
                max_size=_NODE_GROUP_MAX,
            ),
            tags={"Name": f"{name}-nodes"},
            opts=child_opts,
        )

        # ── OIDC provider for IRSA ────────────────────────────────────────────
        oidc_url = cluster.identities[0].oidcs[0].issuer

        # Thumbprint is required by AWS; use a well-known EKS thumbprint as a
        # placeholder — in a real deploy this should be fetched from the OIDC
        # discovery endpoint.
        _thumbprint_placeholder = "9e99a48a9960b14926bb7f3b02e22da2b0ab7280"

        oidc_provider = aws.iam.OpenIdConnectProvider(
            f"{name}-oidc",
            url=oidc_url,
            client_id_lists=["sts.amazonaws.com"],
            thumbprint_lists=[_thumbprint_placeholder],
            opts=child_opts,
        )

        # ── kubeconfig Output ─────────────────────────────────────────────────
        # Build a kubeconfig that uses the aws CLI token exec plugin.
        self.kubeconfig: pulumi.Output[str] = pulumi.Output.all(
            cluster.endpoint,
            cluster.certificate_authorities[0].data,
            cluster.name,
        ).apply(
            lambda args: json.dumps(
                {
                    "apiVersion": "v1",
                    "kind": "Config",
                    "clusters": [
                        {
                            "name": args[2],
                            "cluster": {
                                "server": args[0],
                                "certificate-authority-data": args[1],
                            },
                        }
                    ],
                    "users": [
                        {
                            "name": args[2],
                            "user": {
                                "exec": {
                                    "apiVersion": "client.authentication.k8s.io/v1beta1",
                                    "command": "aws",
                                    "args": [
                                        "eks",
                                        "get-token",
                                        "--cluster-name",
                                        args[2],
                                    ],
                                }
                            },
                        }
                    ],
                    "contexts": [
                        {
                            "name": args[2],
                            "context": {"cluster": args[2], "user": args[2]},
                        }
                    ],
                    "current-context": args[2],
                }
            )
        )

        # contract.kubeconfig is "" because the Output cannot be inlined into a
        # frozen dataclass. Export self.kubeconfig and construct a
        # pulumi_kubernetes.Provider from it in __main__.
        self._contract = SubstrateContract(kubeconfig="", ingress_endpoint="")

        self.register_outputs(
            {
                "kubeconfig": self.kubeconfig,
                "oidc_provider_arn": oidc_provider.arn,
            }
        )

    @property
    def contract(self) -> SubstrateContract:
        """SubstrateContract with empty kubeconfig and ingress_endpoint sentinels.

        Use self.kubeconfig (an Output[str]) to build a pulumi_kubernetes.Provider.
        ingress_endpoint is filled in after Traefik's LoadBalancer Service applies.
        """
        return self._contract
