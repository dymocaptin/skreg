"""GitHub Actions self-hosted runner via actions-runner-controller (ARC)."""

from __future__ import annotations

import pulumi
import pulumi_kubernetes as k8s
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs

_DEPLOYER_SA = "skreg-ci-deployer"
_NAMESPACE = "skreg-ci"


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
                        },
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[controller, deployer_sa]),
        )

        self.register_outputs({"github_repo": github_repo})
