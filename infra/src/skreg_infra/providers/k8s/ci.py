"""GitHub Actions self-hosted runner via actions-runner-controller (ARC)."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs

_DEPLOYER_SA = "skreg-ci-deployer"
_NAMESPACE = "skreg-ci"

# The CI build job pushes images to ``localhost:30500`` (see K8sRegistry, which
# exposes the in-cluster registry on that address from the kind host). The ARC
# runner, however, is an in-cluster pod: its ``localhost`` is the pod's own
# loopback, so the dind daemon's ``docker push localhost:30500`` has nowhere to
# go and the deploy's image push fails. This native sidecar (an initContainer
# with ``restartPolicy: Always``, so it starts before the runner and never
# blocks the ephemeral pod from completing) proxies the pod's loopback :30500 to
# the registry Service. Both IPv4 and IPv6 are bound because the runner image
# resolves ``localhost`` to ``::1`` first.
_REGISTRY_SERVICE = "docker-registry.skreg-infra.svc.cluster.local:5000"
_REGISTRY_PROXY_SIDECAR = {
    "name": "registry-proxy",
    "image": "alpine/socat:1.8.0.1",
    "restartPolicy": "Always",
    "command": ["/bin/sh", "-c"],
    "args": [
        f"socat TCP6-LISTEN:30500,fork,reuseaddr,bind=[::1] TCP:{_REGISTRY_SERVICE} & "
        f"socat TCP4-LISTEN:30500,fork,reuseaddr,bind=127.0.0.1 TCP:{_REGISTRY_SERVICE} & "
        "wait"
    ],
    "resources": {"requests": {"cpu": "10m", "memory": "16Mi"}},
}


class K8sCi(pulumi.ComponentResource):
    """Installs ARC controller + a RunnerSet for the skreg repo.

    Runner pods use the ``skreg-ci-deployer`` ServiceAccount, which is bound
    to ``cluster-admin`` so the deploy job's ``pulumi up`` (in-cluster config)
    can manage resources across namespaces, CRDs, and operators.
    """

    def __init__(
        self,
        name: str,
        github_repo: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:k8s:Ci", name, {}, opts)

        self._github_repo = github_repo

        deployer_sa = k8s.core.v1.ServiceAccount(
            f"{name}-deployer-sa",
            metadata=k8s.meta.v1.ObjectMetaArgs(
                name=_DEPLOYER_SA,
                namespace=_NAMESPACE,
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        k8s.rbac.v1.ClusterRoleBinding(
            f"{name}-deployer-crb",
            metadata=k8s.meta.v1.ObjectMetaArgs(name=_DEPLOYER_SA),
            role_ref=k8s.rbac.v1.RoleRefArgs(
                api_group="rbac.authorization.k8s.io",
                kind="ClusterRole",
                name="cluster-admin",
            ),
            subjects=[
                k8s.rbac.v1.SubjectArgs(
                    kind="ServiceAccount",
                    name=_DEPLOYER_SA,
                    namespace=_NAMESPACE,
                )
            ],
            opts=pulumi.ResourceOptions(parent=self, depends_on=[deployer_sa]),
        )

        controller = Release(
            f"{name}-arc-controller",
            ReleaseArgs(
                chart="oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set-controller",
                version="0.10.1",
                namespace=_NAMESPACE,
                create_namespace=False,
                values={
                    "resources": {
                        "requests": {"cpu": "50m", "memory": "64Mi"},
                    }
                },
            ),
            opts=pulumi.ResourceOptions(parent=self),
        )

        # GitHub PAT must be pre-created:
        # kubectl create secret generic github-pat --namespace skreg-ci \
        #   --from-literal=github_token=<PAT with repo scope>
        Release(
            f"{name}-arc-runner",
            ReleaseArgs(
                chart="oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set",
                version="0.10.1",
                namespace=_NAMESPACE,
                create_namespace=False,
                values={
                    "githubConfigUrl": f"https://github.com/{github_repo}",
                    "githubConfigSecret": "github-pat",
                    "minRunners": 1,
                    "maxRunners": 3,
                    "containerMode": {
                        "type": "dind",
                    },
                    "template": {
                        "spec": {
                            "serviceAccountName": _DEPLOYER_SA,
                            # Native sidecar bridging the pod's localhost:30500
                            # to the in-cluster registry so the deploy job can
                            # push built images. The chart appends its own dind
                            # init/sidecar containers alongside this one.
                            "initContainers": [_REGISTRY_PROXY_SIDECAR],
                        },
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[controller, deployer_sa]),
        )

        self.register_outputs({"github_repo": github_repo})
