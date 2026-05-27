"""GitHub Actions self-hosted runner via actions-runner-controller (ARC)."""
from __future__ import annotations

import pulumi
from pulumi_kubernetes.helm.v3 import Release, ReleaseArgs


class K8sCi(pulumi.ComponentResource):
    """Installs ARC controller + a RunnerSet for the skreg repo."""

    def __init__(
        self,
        name: str,
        github_repo: str,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        super().__init__("skreg:k8s:Ci", name, {}, opts)

        self._github_repo = github_repo

        controller = Release(
            f"{name}-arc-controller",
            ReleaseArgs(
                chart="oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set-controller",
                version="0.10.1",
                namespace="skreg-ci",
                create_namespace=True,
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
                namespace="skreg-ci",
                create_namespace=False,
                values={
                    "githubConfigUrl": f"https://github.com/{github_repo}",
                    "githubConfigSecret": "github-pat",
                    "minRunners": 1,
                    "maxRunners": 3,
                    "containerMode": {
                        "type": "dind",
                    },
                },
            ),
            opts=pulumi.ResourceOptions(parent=self, depends_on=[controller]),
        )

        self.register_outputs({"github_repo": github_repo})
