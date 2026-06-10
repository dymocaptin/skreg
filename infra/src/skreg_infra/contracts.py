"""Provider-neutral connection contracts and component interfaces.

A contract is the only thing the application stack consumes from a backing
service. Implementations (in-cluster or managed) differ; the contract does not.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol


@dataclass(frozen=True)
class SubstrateContract:
    kubeconfig: str
    ingress_endpoint: str


@dataclass(frozen=True)
class DatabaseContract:
    dsn_secret_name: str
    dsn_secret_key: str


@dataclass(frozen=True)
class ObjectStoreContract:
    endpoint: str
    bucket: str
    credentials_secret_name: str


@dataclass(frozen=True)
class DnsContract:
    ingress_endpoint: str


class ClusterSubstrate(Protocol):
    @property
    def contract(self) -> SubstrateContract: ...


class DatabaseComponent(Protocol):
    @property
    def contract(self) -> DatabaseContract: ...


class ObjectStoreComponent(Protocol):
    @property
    def contract(self) -> ObjectStoreContract: ...


class DnsComponent(Protocol):
    @property
    def contract(self) -> DnsContract: ...
