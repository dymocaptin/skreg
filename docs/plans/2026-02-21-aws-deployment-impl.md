# skreg AWS Deployment — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the Pulumi Python infra to deploy skreg to AWS us-west-2 with all missing components, Dockerfiles, and CI pipeline.

**Architecture:** Nine tasks: foundation interface changes, bootstrap script, AwsNetwork / AwsPki / AwsCompute / AwsOidc components, stack wiring, Dockerfiles, CI build-push job.

**Tech Stack:** Python 3.12, Pulumi 3 (pulumi-aws 6), cryptography>=42, Rust 1.85, Docker, GitHub Actions OIDC (aws-actions/configure-aws-credentials@v4, aws-actions/amazon-ecr-login@v2).

---

## Task 1: Foundation changes

**Files:**
- Modify: `infra/pyproject.toml`
- Modify: `infra/src/skillpkg_infra/config.py`
- Modify: `infra/src/skillpkg_infra/components/network.py`
- Modify: `infra/src/skillpkg_infra/components/database.py`
- Modify: `infra/src/skillpkg_infra/providers/aws/database.py`
- Modify: `infra/tests/test_config.py`
- Modify: `infra/tests/test_components.py`
- Modify: `infra/tests/test_aws_database.py`

**Step 1: Write the failing tests**

Replace the full content of `infra/tests/test_config.py`:

```python
"""Tests for the StackConfig environment-driven settings class."""
from __future__ import annotations

import pytest

from skillpkg_infra.config import CloudProvider, HsmBackend, StackConfig


def test_cloud_provider_values() -> None:
    assert CloudProvider.AWS == "aws"
    assert CloudProvider.GCP == "gcp"
    assert CloudProvider.AZURE == "azure"


def test_hsm_backend_values() -> None:
    assert HsmBackend.HSM == "hsm"
    assert HsmBackend.SOFTWARE == "software"


def test_stack_config_load(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.AWS
    assert config.hsm_backend == HsmBackend.HSM
    assert config.multi_az is False
    assert config.environment == "prod"
    assert config.api_image_uri == ""
    assert config.worker_image_uri == ""


def test_stack_config_defaults(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "gcp")
    monkeypatch.setenv("SKILLPKG_HSM_BACKEND", "software")
    monkeypatch.setenv("SKILLPKG_MULTI_AZ", "true")
    monkeypatch.setenv("SKILLPKG_ENVIRONMENT", "staging")
    config = StackConfig.load()
    assert config.cloud_provider == CloudProvider.GCP
    assert config.hsm_backend == HsmBackend.SOFTWARE
    assert config.multi_az is True
    assert config.environment == "staging"


def test_stack_config_image_uris(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SKILLPKG_CLOUD_PROVIDER", "aws")
    monkeypatch.setenv("SKILLPKG_API_IMAGE_URI", "123.dkr.ecr.us-west-2.amazonaws.com/skreg-api:latest")
    monkeypatch.setenv("SKILLPKG_WORKER_IMAGE_URI", "123.dkr.ecr.us-west-2.amazonaws.com/skreg-worker:latest")
    config = StackConfig.load()
    assert config.api_image_uri == "123.dkr.ecr.us-west-2.amazonaws.com/skreg-api:latest"
    assert config.worker_image_uri == "123.dkr.ecr.us-west-2.amazonaws.com/skreg-worker:latest"
```

Replace the full content of `infra/tests/test_components.py`:

```python
"""Verify that component Protocol interfaces are importable and well-typed."""
from __future__ import annotations

import pulumi

from skillpkg_infra.components.compute import ComputeOutputs, SkillpkgCompute
from skillpkg_infra.components.database import DatabaseOutputs, SkillpkgDatabase
from skillpkg_infra.components.network import NetworkOutputs, SkillpkgNetwork
from skillpkg_infra.components.pki import PkiOutputs, SkillpkgPki
from skillpkg_infra.components.storage import SkillpkgStorage, StorageOutputs


def test_database_protocol_is_importable() -> None:
    assert SkillpkgDatabase is not None
    assert DatabaseOutputs is not None


def test_storage_protocol_is_importable() -> None:
    assert SkillpkgStorage is not None
    assert StorageOutputs is not None


def test_pki_protocol_is_importable() -> None:
    assert SkillpkgPki is not None
    assert PkiOutputs is not None


def test_database_outputs_constructible() -> None:
    outputs = DatabaseOutputs(
        connection_secret_name=pulumi.Output.from_input("secret"),
        connection_secret_arn=pulumi.Output.from_input("arn:aws:secretsmanager:us-west-2:123456789:secret:skreg"),
        host=pulumi.Output.from_input("localhost"),
        port=pulumi.Output.from_input(5432),
        database_name=pulumi.Output.from_input("skreg"),
    )
    assert outputs is not None


def test_storage_outputs_constructible() -> None:
    outputs = StorageOutputs(
        bucket_name=pulumi.Output.from_input("skreg-packages"),
        cdn_base_url=pulumi.Output.from_input("https://cdn.example.com"),
        service_account_secret_name=pulumi.Output.from_input("sa-secret"),
    )
    assert outputs is not None


def test_pki_outputs_constructible() -> None:
    outputs = PkiOutputs(
        hsm_key_id=pulumi.Output.from_input("key-id"),
        intermediate_ca_cert_secret_name=pulumi.Output.from_input("ca-secret"),
        crl_bucket_path=pulumi.Output.from_input("s3://bucket/crl.pem"),
        hsm_backend="software",
    )
    assert outputs is not None


def test_compute_outputs_constructible() -> None:
    outputs = ComputeOutputs(
        service_url=pulumi.Output.from_input("https://api.example.com"),
        worker_service_name=pulumi.Output.from_input("skreg-worker"),
    )
    assert outputs is not None


def test_network_outputs_constructible() -> None:
    outputs = NetworkOutputs(
        vpc_id=pulumi.Output.from_input("vpc-123"),
        public_subnet_ids=[
            pulumi.Output.from_input("subnet-pub-1"),
            pulumi.Output.from_input("subnet-pub-2"),
        ],
        private_subnet_ids=[
            pulumi.Output.from_input("subnet-priv-1"),
            pulumi.Output.from_input("subnet-priv-2"),
        ],
    )
    assert outputs is not None
    assert len(outputs.public_subnet_ids) == 2
    assert len(outputs.private_subnet_ids) == 2
```

Replace the full content of `infra/tests/test_aws_database.py`:

```python
"""Unit tests for the AWS database component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs  # noqa: E402


@pulumi.runtime.test
def test_database_port_is_5432() -> None:
    db = AwsDatabase("test-db", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_port(port: int) -> None:
        assert port == 5432

    return db.outputs.port.apply(assert_port)


@pulumi.runtime.test
def test_database_name_is_skillpkg() -> None:
    db = AwsDatabase("test-db2", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_name(name: str) -> None:
        assert name == "skillpkg"

    return db.outputs.database_name.apply(assert_name)


@pulumi.runtime.test
def test_database_connection_secret_arn_is_set() -> None:
    db = AwsDatabase("test-db3", AwsDatabaseArgs(vpc_id="vpc-abc", subnet_ids=["subnet-1"]))

    def assert_arn(arn: str) -> None:
        assert arn, f"Expected non-empty connection_secret_arn, got {arn!r}"

    return db.outputs.connection_secret_arn.apply(assert_arn)
```

**Step 2: Run tests — confirm failures**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_config.py tests/test_components.py tests/test_aws_database.py -v
```

Expected: failures on `image_uri`, `public_subnet_ids`, `connection_secret_arn`.

**Step 3: Update `pyproject.toml` — add `cryptography>=42,<43`**

```toml
[project]
name = "skillpkg-infra"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
    "pulumi>=3,<4",
    "pulumi-aws>=6,<7",
    "pulumi-gcp>=7,<8",
    "pulumi-azure-native>=2,<3",
    "pydantic-settings>=2,<3",
    "structlog>=24,<25",
    "cryptography>=42,<43",
]
```

```bash
cd /home/dymo/skreg/infra && uv lock
```

**Step 4: Update `config.py`**

```python
"""Typed configuration loaded from environment variables at startup."""

from __future__ import annotations

import logging
from enum import StrEnum
from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict

logger: logging.Logger = logging.getLogger(__name__)


class CloudProvider(StrEnum):
    AWS = "aws"
    GCP = "gcp"
    AZURE = "azure"


class HsmBackend(StrEnum):
    HSM = "hsm"
    SOFTWARE = "software"


class StackConfig(BaseSettings):
    """Fully validated infrastructure stack configuration."""

    model_config = SettingsConfigDict(
        env_prefix="SKILLPKG_",
        env_file=".env",
        env_file_encoding="utf-8",
    )

    cloud_provider: CloudProvider
    api_image_uri: str = ""
    worker_image_uri: str = ""
    hsm_backend: HsmBackend = HsmBackend.HSM
    multi_az: bool = False
    environment: Literal["prod", "staging", "dev"] = "prod"

    @classmethod
    def load(cls) -> StackConfig:
        """Load and validate configuration from the environment."""
        config = cls()  # type: ignore[call-arg]
        logger.debug(
            "stack_config_loaded",
            extra={
                "cloud_provider": config.cloud_provider.value,
                "hsm_backend": config.hsm_backend.value,
                "multi_az": config.multi_az,
                "environment": config.environment,
            },
        )
        return config
```

**Step 5: Update `components/network.py`**

```python
"""Provider-agnostic network component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class NetworkOutputs:
    """Resolved outputs from a provisioned network component."""

    def __init__(
        self,
        vpc_id: pulumi.Output[str],
        public_subnet_ids: list[pulumi.Output[str]],
        private_subnet_ids: list[pulumi.Output[str]],
    ) -> None:
        self.vpc_id: pulumi.Output[str] = vpc_id
        self.public_subnet_ids: list[pulumi.Output[str]] = public_subnet_ids
        self.private_subnet_ids: list[pulumi.Output[str]] = private_subnet_ids


class SkillpkgNetwork(Protocol):
    @property
    def outputs(self) -> NetworkOutputs: ...
```

**Step 6: Update `components/database.py`**

```python
"""Provider-agnostic database component interface."""

from __future__ import annotations

import logging
from typing import Protocol

import pulumi

logger: logging.Logger = logging.getLogger(__name__)


class DatabaseOutputs:
    """Resolved connection outputs from a provisioned database component."""

    def __init__(
        self,
        connection_secret_name: pulumi.Output[str],
        connection_secret_arn: pulumi.Output[str],
        host: pulumi.Output[str],
        port: pulumi.Output[int],
        database_name: pulumi.Output[str],
    ) -> None:
        self.connection_secret_name: pulumi.Output[str] = connection_secret_name
        self.connection_secret_arn: pulumi.Output[str] = connection_secret_arn
        self.host: pulumi.Output[str] = host
        self.port: pulumi.Output[int] = port
        self.database_name: pulumi.Output[str] = database_name


class SkillpkgDatabase(Protocol):
    @property
    def outputs(self) -> DatabaseOutputs: ...
```

**Step 7: Update `providers/aws/database.py` — expose `password_secret.arn`**

Add `connection_secret_arn=password_secret.arn` to `DatabaseOutputs(...)` constructor and to `register_outputs(...)`:

```python
self._outputs: DatabaseOutputs = DatabaseOutputs(
    connection_secret_name=password_secret.name,
    connection_secret_arn=password_secret.arn,
    host=instance.address,
    port=pulumi.Output.from_input(5432),
    database_name=pulumi.Output.from_input("skillpkg"),
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
```

**Step 8: Verify tests pass**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_config.py tests/test_components.py tests/test_aws_database.py -v && uv run mypy src/
```

Expected: all pass, `Success: no issues found`.

**Step 9: Commit**

```bash
cd /home/dymo/skreg && git add infra/pyproject.toml infra/uv.lock infra/src/skillpkg_infra/config.py infra/src/skillpkg_infra/components/network.py infra/src/skillpkg_infra/components/database.py infra/src/skillpkg_infra/providers/aws/database.py infra/tests/test_config.py infra/tests/test_components.py infra/tests/test_aws_database.py && git commit -m "feat: foundation — split image_uri, add public_subnet_ids and connection_secret_arn"
```

---

## Task 2: Bootstrap script

**Files:**
- Create: `scripts/bootstrap.sh`

**Step 1: Create `scripts/bootstrap.sh`**

```bash
#!/usr/bin/env bash
# bootstrap.sh — Create the SSE-S3 encrypted Pulumi state bucket for skreg.
# Usage: ./scripts/bootstrap.sh <aws-account-id> [region]
set -euo pipefail

ACCOUNT_ID="${1:?Usage: bootstrap.sh <aws-account-id> [region]}"
REGION="${2:-us-west-2}"
BUCKET="skreg-pulumi-state-${ACCOUNT_ID}"

echo "Creating Pulumi state bucket: s3://${BUCKET} in ${REGION}"

if [[ "${REGION}" == "us-east-1" ]]; then
    aws s3api create-bucket --bucket "${BUCKET}" --region "${REGION}"
else
    aws s3api create-bucket \
        --bucket "${BUCKET}" \
        --region "${REGION}" \
        --create-bucket-configuration LocationConstraint="${REGION}"
fi

aws s3api put-bucket-encryption \
    --bucket "${BUCKET}" \
    --server-side-encryption-configuration '{
        "Rules": [{
            "ApplyServerSideEncryptionByDefault": {"SSEAlgorithm": "AES256"},
            "BucketKeyEnabled": true
        }]
    }'

aws s3api put-public-access-block \
    --bucket "${BUCKET}" \
    --public-access-block-configuration \
        "BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true"

aws s3api put-bucket-versioning \
    --bucket "${BUCKET}" \
    --versioning-configuration Status=Enabled

echo ""
echo "Done. Run the following to configure Pulumi:"
echo "  pulumi login s3://${BUCKET}"
```

**Step 2: Make executable**

```bash
chmod +x /home/dymo/skreg/scripts/bootstrap.sh
```

**Step 3: Commit**

```bash
cd /home/dymo/skreg && git add scripts/bootstrap.sh && git commit -m "feat: add bootstrap.sh to create SSE-S3 Pulumi state bucket"
```

---

## Task 3: AwsNetwork

**Files:**
- Create: `infra/src/skillpkg_infra/providers/aws/network.py`
- Create: `infra/tests/test_aws_network.py`

**Step 1: Write the failing test**

Create `infra/tests/test_aws_network.py`:

```python
"""Unit tests for the AWS network component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.network import AwsNetwork  # noqa: E402


def test_network_public_subnet_count() -> None:
    net = AwsNetwork("test-net")
    assert len(net.outputs.public_subnet_ids) == 2


def test_network_private_subnet_count() -> None:
    net = AwsNetwork("test-net2")
    assert len(net.outputs.private_subnet_ids) == 2


@pulumi.runtime.test
def test_network_vpc_id_is_set() -> None:
    net = AwsNetwork("test-net3")

    def check(vpc_id: str) -> None:
        assert vpc_id

    return net.outputs.vpc_id.apply(check)


@pulumi.runtime.test
def test_network_first_public_subnet_is_set() -> None:
    net = AwsNetwork("test-net4")

    def check(subnet_id: str) -> None:
        assert subnet_id

    return net.outputs.public_subnet_ids[0].apply(check)


@pulumi.runtime.test
def test_network_first_private_subnet_is_set() -> None:
    net = AwsNetwork("test-net5")

    def check(subnet_id: str) -> None:
        assert subnet_id

    return net.outputs.private_subnet_ids[0].apply(check)
```

**Step 2: Run — confirm `ModuleNotFoundError`**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_network.py -v
```

**Step 3: Implement `providers/aws/network.py`**

```python
"""AWS VPC implementation of SkillpkgNetwork."""

from __future__ import annotations

import logging

import pulumi
import pulumi_aws as aws

from skillpkg_infra.components.network import NetworkOutputs

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
        super().__init__("skillpkg:aws:Network", name, {}, opts)

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
```

**Step 4: Verify tests pass**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_network.py -v
```

Expected: 5 passed.

**Step 5: Full suite + type check**

```bash
cd /home/dymo/skreg/infra && uv run pytest -v && uv run mypy src/
```

**Step 6: Commit**

```bash
cd /home/dymo/skreg && git add infra/src/skillpkg_infra/providers/aws/network.py infra/tests/test_aws_network.py && git commit -m "feat: add AwsNetwork — VPC, subnets, IGW, NAT Gateway, route tables"
```

---

## Task 4: AwsPki (software backend)

**Files:**
- Create: `infra/src/skillpkg_infra/providers/aws/pki.py`
- Create: `infra/tests/test_aws_pki.py`

**Step 1: Write the failing tests**

Create `infra/tests/test_aws_pki.py`:

```python
"""Unit tests for the AWS PKI component using Pulumi mocks."""
from __future__ import annotations

from unittest.mock import patch

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.pki import AwsPki, AwsPkiArgs, _generate_root_ca  # noqa: E402


def test_generate_root_ca_returns_pem_strings() -> None:
    """_generate_root_ca returns two PEM-encoded strings."""
    key_pem, cert_pem = _generate_root_ca()
    assert "PRIVATE KEY" in key_pem
    assert "CERTIFICATE" in cert_pem


def test_generate_root_ca_key_is_4096_bits() -> None:
    """_generate_root_ca produces a 4096-bit RSA key."""
    from cryptography.hazmat.primitives.serialization import load_pem_private_key

    key_pem, _ = _generate_root_ca()
    key = load_pem_private_key(key_pem.encode(), password=None)
    assert key.key_size == 4096  # type: ignore[attr-defined]


@pulumi.runtime.test
def test_pki_root_ca_cert_pem_is_exposed() -> None:
    """AwsPki must expose root_ca_cert_pem as a non-empty Output."""
    with patch(
        "skillpkg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ):
        pki = AwsPki("test-pki", AwsPkiArgs(bucket_name="test-bucket"))

    def check(cert: str) -> None:
        assert cert

    return pki.root_ca_cert_pem.apply(check)


@pulumi.runtime.test
def test_pki_hsm_key_id_is_set() -> None:
    """AwsPki outputs.hsm_key_id must be non-empty."""
    with patch(
        "skillpkg_infra.providers.aws.pki._generate_root_ca",
        return_value=("---KEY---", "---CERT---"),
    ):
        pki = AwsPki("test-pki2", AwsPkiArgs(bucket_name="test-bucket"))

    def check(key_id: str) -> None:
        assert key_id

    return pki.outputs.hsm_key_id.apply(check)
```

**Step 2: Run — confirm `ModuleNotFoundError`**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_pki.py -v
```

**Step 3: Implement `providers/aws/pki.py`**

```python
"""AWS Secrets Manager + S3 software PKI implementation of SkillpkgPki."""

from __future__ import annotations

import datetime
import logging

import pulumi
import pulumi_aws as aws
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

from skillpkg_infra.components.pki import PkiOutputs

logger: logging.Logger = logging.getLogger(__name__)


def _generate_root_ca() -> tuple[str, str]:
    """Generate a self-signed RSA-4096 root CA key and certificate.

    Returns:
        ``(pem_key, pem_cert)`` — both as ASCII strings. Validity: 10 years.
    """
    private_key = rsa.generate_private_key(public_exponent=65537, key_size=4096)

    subject = x509.Name(
        [
            x509.NameAttribute(NameOID.COMMON_NAME, "skreg Root CA"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "skreg"),
        ]
    )
    now = datetime.datetime.now(tz=datetime.timezone.utc)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(subject)
        .public_key(private_key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now)
        .not_valid_after(now + datetime.timedelta(days=3650))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .sign(private_key, hashes.SHA256())
    )
    pem_key = private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.TraditionalOpenSSL,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode("ascii")
    pem_cert = cert.public_bytes(serialization.Encoding.PEM).decode("ascii")
    return pem_key, pem_cert


class AwsPkiArgs:
    """Arguments for the software PKI component."""

    def __init__(self, bucket_name: pulumi.Input[str]) -> None:
        self.bucket_name: pulumi.Input[str] = bucket_name


class AwsPki(pulumi.ComponentResource):
    """AWS Secrets Manager + S3 software PKI satisfying ``SkillpkgPki``.

    Generates an RSA-4096 root CA on first deploy; ``ignore_changes`` on the
    key secret prevents unintentional rotation on subsequent ``pulumi up`` runs.
    """

    def __init__(
        self,
        name: str,
        args: AwsPkiArgs,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skillpkg:aws:Pki", name, {}, opts)

        logger.debug("provisioning_aws_pki", extra={"name": name})

        pem_key, pem_cert = _generate_root_ca()

        ca_key_secret = aws.secretsmanager.Secret(
            f"{name}-ca-key",
            aws.secretsmanager.SecretArgs(name="skreg/pki/root-ca-key"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.secretsmanager.SecretVersion(
            f"{name}-ca-key-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=ca_key_secret.id,
                secret_string=pem_key,
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["secret_string"]),
        )

        ca_cert_secret = aws.secretsmanager.Secret(
            f"{name}-ca-cert",
            aws.secretsmanager.SecretArgs(name="skreg/pki/root-ca-cert"),
            opts=pulumi.ResourceOptions(parent=self),
        )

        ca_cert_version = aws.secretsmanager.SecretVersion(
            f"{name}-ca-cert-version",
            aws.secretsmanager.SecretVersionArgs(
                secret_id=ca_cert_secret.id,
                secret_string=pem_cert,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.s3.BucketObject(
            f"{name}-crl",
            aws.s3.BucketObjectArgs(
                bucket=args.bucket_name,
                key=".well-known/crl.pem",
                content="",
            ),
            opts=pulumi.ResourceOptions(parent=self, ignore_changes=["content"]),
        )

        self._outputs: PkiOutputs = PkiOutputs(
            hsm_key_id=ca_key_secret.id,
            intermediate_ca_cert_secret_name=ca_cert_secret.name,
            crl_bucket_path=pulumi.Output.from_input(args.bucket_name).apply(
                lambda b: f"s3://{b}/.well-known/crl.pem"
            ),
            hsm_backend="software",
        )
        self.root_ca_cert_pem: pulumi.Output[str] = ca_cert_version.secret_string

        self.register_outputs(
            {
                "hsm_key_id": self._outputs.hsm_key_id,
                "intermediate_ca_cert_secret_name": self._outputs.intermediate_ca_cert_secret_name,
                "crl_bucket_path": self._outputs.crl_bucket_path,
                "root_ca_cert_pem": self.root_ca_cert_pem,
            }
        )

    @property
    def outputs(self) -> PkiOutputs:
        """Return the resolved PKI outputs."""
        return self._outputs
```

**Step 4: Verify tests pass**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_pki.py -v
```

Expected: 4 passed. Note: `test_generate_root_ca_*` tests run the real RSA-4096 key generation (~2–5 s). That is expected.

**Step 5: Full suite + type check**

```bash
cd /home/dymo/skreg/infra && uv run pytest -v && uv run mypy src/
```

**Step 6: Commit**

```bash
cd /home/dymo/skreg && git add infra/src/skillpkg_infra/providers/aws/pki.py infra/tests/test_aws_pki.py && git commit -m "feat: add AwsPki — RSA-4096 root CA in Secrets Manager, S3 CRL placeholder"
```

---

## Task 5: AwsCompute

**Files:**
- Create: `infra/src/skillpkg_infra/providers/aws/compute.py`
- Create: `infra/tests/test_aws_compute.py`

**Step 1: Write the failing tests**

Create `infra/tests/test_aws_compute.py`:

```python
"""Unit tests for the AWS compute component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.compute import AwsCompute, AwsComputeArgs  # noqa: E402


def _args() -> AwsComputeArgs:
    return AwsComputeArgs(
        vpc_id="vpc-test",
        public_subnet_ids=["subnet-pub-1", "subnet-pub-2"],
        private_subnet_ids=["subnet-priv-1", "subnet-priv-2"],
        db_secret_arn="arn:aws:secretsmanager:us-west-2:123456789:secret:test-db",
    )


@pulumi.runtime.test
def test_compute_service_url_starts_with_http() -> None:
    compute = AwsCompute("test-cmp", _args())

    def check(url: str) -> None:
        assert url.startswith("http://"), url

    return compute.outputs.service_url.apply(check)


@pulumi.runtime.test
def test_compute_worker_service_name_is_set() -> None:
    compute = AwsCompute("test-cmp2", _args())

    def check(name: str) -> None:
        assert name

    return compute.outputs.worker_service_name.apply(check)


@pulumi.runtime.test
def test_compute_ecr_api_repo_is_set() -> None:
    compute = AwsCompute("test-cmp3", _args())

    def check(url: str) -> None:
        assert url

    return compute.ecr_api_repo.apply(check)


@pulumi.runtime.test
def test_compute_ecr_worker_repo_is_set() -> None:
    compute = AwsCompute("test-cmp4", _args())

    def check(url: str) -> None:
        assert url

    return compute.ecr_worker_repo.apply(check)
```

**Step 2: Run — confirm `ModuleNotFoundError`**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_compute.py -v
```

**Step 3: Implement `providers/aws/compute.py`**

```python
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

        aws.lb.Listener(
            f"{name}-listener",
            aws.lb.ListenerArgs(
                load_balancer_arn=alb.arn,
                port=80,
                protocol="HTTP",
                default_actions=[
                    aws.lb.ListenerDefaultActionArgs(
                        type="forward", target_group_arn=tg.arn
                    )
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
            opts=pulumi.ResourceOptions(parent=self),
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
```

**Step 4: Verify tests pass**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_compute.py -v
```

Expected: 4 passed.

**Step 5: Full suite + type check**

```bash
cd /home/dymo/skreg/infra && uv run pytest -v && uv run mypy src/
```

**Step 6: Commit**

```bash
cd /home/dymo/skreg && git add infra/src/skillpkg_infra/providers/aws/compute.py infra/tests/test_aws_compute.py && git commit -m "feat: add AwsCompute — ECR, ECS Fargate, ALB, security groups, CloudWatch"
```

---

## Task 6: AwsOidc

**Files:**
- Create: `infra/src/skillpkg_infra/providers/aws/oidc.py`
- Create: `infra/tests/test_aws_oidc.py`

**Step 1: Write the failing tests**

Create `infra/tests/test_aws_oidc.py`:

```python
"""Unit tests for the AWS OIDC component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class SkillpkgMocks(Mocks):
    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        return (f"{args.name}-id", args.inputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(SkillpkgMocks())

from skillpkg_infra.providers.aws.oidc import AwsOidc  # noqa: E402


@pulumi.runtime.test
def test_oidc_role_arn_is_set() -> None:
    oidc = AwsOidc("test-oidc", github_repo="dymocaptin/skreg")

    def check(arn: str) -> None:
        assert arn

    return oidc.outputs.role_arn.apply(check)


@pulumi.runtime.test
def test_oidc_role_arn_different_repo() -> None:
    oidc = AwsOidc("test-oidc2", github_repo="org/other-repo")

    def check(arn: str) -> None:
        assert arn

    return oidc.outputs.role_arn.apply(check)
```

**Step 2: Run — confirm `ModuleNotFoundError`**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_oidc.py -v
```

**Step 3: Implement `providers/aws/oidc.py`**

```python
"""AWS IAM OIDC provider + ECR push role for GitHub Actions."""

from __future__ import annotations

import json
import logging

import pulumi
import pulumi_aws as aws

logger: logging.Logger = logging.getLogger(__name__)

_GITHUB_THUMBPRINTS: list[str] = [
    "6938fd4d98bab03faadb97b34396831e3780aea1",
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd",
]


class AwsOidcOutputs:
    """Resolved outputs from the OIDC component."""

    def __init__(self, role_arn: pulumi.Output[str]) -> None:
        self.role_arn: pulumi.Output[str] = role_arn


class AwsOidc(pulumi.ComponentResource):
    """GitHub Actions OIDC identity provider + least-privilege ECR push role.

    Trust policy constrains assumption to pushes from ``github_repo`` on ``main`` only.
    """

    def __init__(
        self,
        name: str,
        github_repo: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skillpkg:aws:Oidc", name, {}, opts)

        logger.debug("provisioning_aws_oidc", extra={"name": name, "repo": github_repo})

        provider = aws.iam.OpenIdConnectProvider(
            f"{name}-gh-oidc",
            aws.iam.OpenIdConnectProviderArgs(
                url="https://token.actions.githubusercontent.com",
                client_id_list=["sts.amazonaws.com"],
                thumbprint_list=_GITHUB_THUMBPRINTS,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        trust_policy = provider.arn.apply(
            lambda arn: json.dumps(
                {
                    "Version": "2012-10-17",
                    "Statement": [
                        {
                            "Effect": "Allow",
                            "Principal": {"Federated": arn},
                            "Action": "sts:AssumeRoleWithWebIdentity",
                            "Condition": {
                                "StringEquals": {
                                    "token.actions.githubusercontent.com:aud": "sts.amazonaws.com",
                                    "token.actions.githubusercontent.com:sub": (
                                        f"repo:{github_repo}:ref:refs/heads/main"
                                    ),
                                }
                            },
                        }
                    ],
                }
            )
        )

        role = aws.iam.Role(
            f"{name}-gh-role",
            aws.iam.RoleArgs(assume_role_policy=trust_policy),
            opts=pulumi.ResourceOptions(parent=self),
        )

        aws.iam.RolePolicy(
            f"{name}-gh-policy",
            aws.iam.RolePolicyArgs(
                role=role.name,
                policy=json.dumps(
                    {
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Action": ["ecr:GetAuthorizationToken"],
                                "Resource": "*",
                            },
                            {
                                "Effect": "Allow",
                                "Action": [
                                    "ecr:BatchCheckLayerAvailability",
                                    "ecr:PutImage",
                                    "ecr:InitiateLayerUpload",
                                    "ecr:UploadLayerPart",
                                    "ecr:CompleteLayerUpload",
                                ],
                                "Resource": "arn:aws:ecr:us-west-2:*:repository/skreg-*",
                            },
                        ],
                    }
                ),
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        self._outputs: AwsOidcOutputs = AwsOidcOutputs(role_arn=role.arn)
        self.register_outputs({"role_arn": self._outputs.role_arn})

    @property
    def outputs(self) -> AwsOidcOutputs:
        """Return the resolved OIDC outputs."""
        return self._outputs
```

**Step 4: Verify tests pass**

```bash
cd /home/dymo/skreg/infra && uv run pytest tests/test_aws_oidc.py -v
```

Expected: 2 passed.

**Step 5: Full suite + type check**

```bash
cd /home/dymo/skreg/infra && uv run pytest -v && uv run mypy src/
```

**Step 6: Commit**

```bash
cd /home/dymo/skreg && git add infra/src/skillpkg_infra/providers/aws/oidc.py infra/tests/test_aws_oidc.py && git commit -m "feat: add AwsOidc — GitHub Actions OIDC provider and least-privilege ECR push role"
```

---

## Task 7: Wire `SkillpkgStack.run()`

**Files:**
- Modify: `infra/src/skillpkg_infra/__main__.py`

**Step 1: Replace `__main__.py`**

```python
"""Pulumi stack entry point for skreg infrastructure."""

from __future__ import annotations

import logging

import pulumi
import structlog

from skillpkg_infra.config import CloudProvider, StackConfig
from skillpkg_infra.providers.aws.compute import AwsCompute, AwsComputeArgs
from skillpkg_infra.providers.aws.database import AwsDatabase, AwsDatabaseArgs
from skillpkg_infra.providers.aws.network import AwsNetwork
from skillpkg_infra.providers.aws.oidc import AwsOidc
from skillpkg_infra.providers.aws.pki import AwsPki, AwsPkiArgs
from skillpkg_infra.providers.aws.storage import AwsStorage

logger: logging.Logger = logging.getLogger(__name__)


class SkillpkgStack:
    """Orchestrates all provider-agnostic infrastructure components."""

    def __init__(self, config: StackConfig) -> None:
        """Initialise the stack with resolved configuration."""
        self._config: StackConfig = config

    def run(self) -> None:
        """Provision the full infrastructure stack."""
        logger.info(
            "stack_run_started",
            extra={"cloud_provider": self._config.cloud_provider.value},
        )
        if self._config.cloud_provider == CloudProvider.AWS:
            self._run_aws()
        else:
            raise NotImplementedError(
                f"Provider '{self._config.cloud_provider}' not yet implemented."
            )

    def _run_aws(self) -> None:
        config = self._config

        network = AwsNetwork("skreg-network")
        storage = AwsStorage("skreg-storage")
        pki = AwsPki("skreg-pki", AwsPkiArgs(bucket_name=storage.outputs.bucket_name))
        database = AwsDatabase(
            "skreg-db",
            AwsDatabaseArgs(
                vpc_id=network.outputs.vpc_id,
                subnet_ids=network.outputs.private_subnet_ids,
                multi_az=config.multi_az,
            ),
        )
        compute = AwsCompute(
            "skreg-compute",
            AwsComputeArgs(
                vpc_id=network.outputs.vpc_id,
                public_subnet_ids=network.outputs.public_subnet_ids,
                private_subnet_ids=network.outputs.private_subnet_ids,
                db_secret_arn=database.outputs.connection_secret_arn,
                api_image_uri=config.api_image_uri,
                worker_image_uri=config.worker_image_uri,
            ),
        )
        oidc = AwsOidc("skreg-oidc", github_repo="dymocaptin/skreg")

        pulumi.export("api_url", compute.outputs.service_url)
        pulumi.export("cdn_base_url", storage.outputs.cdn_base_url)
        pulumi.export("root_ca_cert", pki.root_ca_cert_pem)
        pulumi.export("ecr_api_repo", compute.ecr_api_repo)
        pulumi.export("ecr_worker_repo", compute.ecr_worker_repo)
        pulumi.export("oidc_role_arn", oidc.outputs.role_arn)


if __name__ == "__main__":
    structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
    SkillpkgStack(config=StackConfig.load()).run()
```

**Step 2: Type check + full test run**

```bash
cd /home/dymo/skreg/infra && uv run mypy src/ && uv run pytest -v
```

Expected: `Success: no issues found`, all tests pass, coverage >= 90%.

If mypy reports `list[Output[str]]` vs `list[Input[str]]` mismatch, cast as:

```python
subnet_ids=list(network.outputs.private_subnet_ids),
```

**Step 3: Commit**

```bash
cd /home/dymo/skreg && git add infra/src/skillpkg_infra/__main__.py && git commit -m "feat: wire SkillpkgStack.run() — full AWS stack provisioning with all exports"
```

---

## Task 8: Dockerfiles

**Files:**
- Create: `crates/skreg-api/Dockerfile`
- Create: `crates/skreg-worker/Dockerfile`

**Step 1: Create `crates/skreg-api/Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p skreg-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/skreg-api /usr/local/bin/skreg-api
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/skreg-api"]
```

**Step 2: Create `crates/skreg-worker/Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p skreg-worker

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/skreg-worker /usr/local/bin/skreg-worker
ENTRYPOINT ["/usr/local/bin/skreg-worker"]
```

**Step 3: Verify both build (optional — requires Docker)**

```bash
cd /home/dymo/skreg && docker build -f crates/skreg-api/Dockerfile -t skreg-api:local .
cd /home/dymo/skreg && docker build -f crates/skreg-worker/Dockerfile -t skreg-worker:local .
```

Expected: both complete with exit code 0.

**Step 4: Commit**

```bash
cd /home/dymo/skreg && git add crates/skreg-api/Dockerfile crates/skreg-worker/Dockerfile && git commit -m "feat: add multi-stage Dockerfiles for skreg-api and skreg-worker"
```

---

## Task 9: CI `build-push` job

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Replace `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    name: Rust — test, lint, format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all --check

      - name: Clippy (deny warnings)
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test --workspace

  python-infra:
    name: Python infra — lint, type-check, test
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: infra
    steps:
      - uses: actions/checkout@v4

      - name: Install uv
        uses: astral-sh/setup-uv@v4

      - name: Install dependencies
        run: uv sync --extra dev

      - name: ruff check
        run: uv run ruff check src/

      - name: black check
        run: uv run black --check src/

      - name: mypy
        run: uv run mypy src/

      - name: pytest
        run: uv run pytest --cov-fail-under=90

  build-push:
    name: Build and push Docker images to ECR
    needs: [rust, python-infra]
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main' && github.event_name == 'push'
    permissions:
      id-token: write
      contents: read
    steps:
      - uses: actions/checkout@v4

      - name: Configure AWS credentials via OIDC
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: ${{ vars.AWS_ROLE_ARN }}
          aws-region: us-west-2

      - name: Login to Amazon ECR
        id: ecr-login
        uses: aws-actions/amazon-ecr-login@v2

      - name: Build and push skreg-api
        env:
          REGISTRY: ${{ steps.ecr-login.outputs.registry }}
          SHA: ${{ github.sha }}
        run: |
          docker build \
            -f crates/skreg-api/Dockerfile \
            -t "$REGISTRY/skreg-api:$SHA" \
            -t "$REGISTRY/skreg-api:latest" \
            .
          docker push "$REGISTRY/skreg-api:$SHA"
          docker push "$REGISTRY/skreg-api:latest"

      - name: Build and push skreg-worker
        env:
          REGISTRY: ${{ steps.ecr-login.outputs.registry }}
          SHA: ${{ github.sha }}
        run: |
          docker build \
            -f crates/skreg-worker/Dockerfile \
            -t "$REGISTRY/skreg-worker:$SHA" \
            -t "$REGISTRY/skreg-worker:latest" \
            .
          docker push "$REGISTRY/skreg-worker:$SHA"
          docker push "$REGISTRY/skreg-worker:latest"
```

**Step 2: Validate YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('/home/dymo/skreg/.github/workflows/ci.yml'))" && echo "YAML valid"
```

**Step 3: Final full infra test run**

```bash
cd /home/dymo/skreg/infra && uv run pytest -v --cov-fail-under=90 && uv run mypy src/
```

Expected: all tests pass, coverage >= 90%, mypy clean.

**Step 4: Commit**

```bash
cd /home/dymo/skreg && git add .github/workflows/ci.yml && git commit -m "feat: add CI build-push job — OIDC auth to ECR, tag :sha and :latest"
```

---

## Post-implementation bootstrap (run once, locally)

```bash
# 1. Create Pulumi state bucket
./scripts/bootstrap.sh <your-aws-account-id> us-west-2

# 2. Log Pulumi into the S3 backend
pulumi login s3://skreg-pulumi-state-<your-aws-account-id>

# 3. Set required env vars
export SKILLPKG_CLOUD_PROVIDER=aws
export SKILLPKG_API_IMAGE_URI=""
export SKILLPKG_WORKER_IMAGE_URI=""

# 4. Deploy
cd infra && pulumi stack init prod && pulumi up

# 5. Add oidc_role_arn output to GitHub:
#    Settings → Secrets and variables → Actions → Repository variables
#    Name: AWS_ROLE_ARN  Value: <oidc_role_arn from pulumi stack output>

# 6. Push a commit to main — CI will build and push the first Docker images
```
