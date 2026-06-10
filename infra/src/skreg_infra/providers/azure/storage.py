"""Azure Blob Storage for skreg package objects.

# KNOWN LIMITATION — S3-compatibility gap
#
# Azure Blob Storage does NOT expose an S3-compatible API. The skreg-api and
# skreg-worker binaries talk to an S3-compatible endpoint (AWS SDK / MinIO).
# To bridge this gap an S3-compatibility gateway is required — for example
# MinIO running in Azure Gateway mode (``minio gateway azure``).
#
# This module provisions the Azure storage infrastructure (StorageAccount +
# BlobContainer) and stores the access credentials in a Kubernetes Secret so
# that a separately deployed gateway can mount them. The gateway deployment
# itself is out of scope here and flagged as a known limitation.
#
# Until a gateway is wired in, the contract ``endpoint`` points directly at
# the Azure Blob primary endpoint, which the application cannot use natively.

Provisions a StorageAccount and BlobContainer named ``skreg-packages``, then
writes the account name and key into the Kubernetes Secret ``skreg-minio``
(namespace ``skreg``) under the conventional ``AWS_ACCESS_KEY_ID`` /
``AWS_SECRET_ACCESS_KEY`` keys so that an S3-compatibility gateway (e.g. MinIO
Azure Gateway) can pick them up without code changes in the application.
"""

from __future__ import annotations

import pulumi
import pulumi_azure_native.storage as storage
import pulumi_kubernetes as k8s

from skreg_infra.contracts import ObjectStoreContract

_CONTAINER_NAME = "skreg-packages"
_SECRET_NAME = "skreg-minio"  # noqa: S105
_NAMESPACE = "skreg"


class BlobObjectStore(pulumi.ComponentResource):
    """Azure StorageAccount + BlobContainer with K8s credentials Secret.

    The ``contract.endpoint`` is ``https://{account_name}.blob.core.windows.net``.
    Because Azure storage account names must be globally unique and the account
    name is an explicit constructor argument (defaulting to ``"skregpackages"``),
    the endpoint string is known statically at plan time.

    See module-level comment for the S3-compatibility limitation.
    """

    def __init__(
        self,
        name: str,
        resource_group_name: str,
        location: str = "eastus",
        account_name: str = "skregpackages",
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:azure:BlobObjectStore", name, {}, opts)

        self._account_name = account_name

        account = storage.StorageAccount(
            f"{name}-account",
            account_name=account_name,
            resource_group_name=resource_group_name,
            location=location,
            sku=storage.SkuArgs(name=storage.SkuName.STANDARD_LRS),
            kind=storage.Kind.STORAGE_V2,
            opts=pulumi.ResourceOptions(parent=self),
        )

        container = storage.BlobContainer(
            f"{name}-container",
            account_name=account.name,
            container_name=_CONTAINER_NAME,
            resource_group_name=resource_group_name,
            public_access=storage.PublicAccess.NONE,
            opts=pulumi.ResourceOptions(parent=self, depends_on=[account]),
        )

        keys = pulumi.Output.all(resource_group_name, account.name).apply(
            lambda args: storage.list_storage_account_keys(
                resource_group_name=args[0],
                account_name=args[1],
            )
        )
        account_key = keys.apply(lambda k: k.keys[0].value)

        k8s.core.v1.Secret(
            f"{name}-secret",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_SECRET_NAME,
                namespace=_NAMESPACE,
            ),
            string_data={
                "AWS_ACCESS_KEY_ID": account.name,
                "AWS_SECRET_ACCESS_KEY": account_key,
            },
            opts=pulumi.ResourceOptions(parent=self, depends_on=[container]),
        )

        endpoint = f"https://{account_name}.blob.core.windows.net"
        self._contract = ObjectStoreContract(
            endpoint=endpoint,
            bucket=_CONTAINER_NAME,
            credentials_secret_name=_SECRET_NAME,
        )
        self.register_outputs({"endpoint": endpoint, "bucket": _CONTAINER_NAME})

    @property
    def contract(self) -> ObjectStoreContract:
        return self._contract
