from unittest.mock import MagicMock, patch


def test_k8s_network_creates_traefik_release() -> None:
    with patch("pulumi_kubernetes.helm.v3.Release") as mock_release:
        mock_release.return_value = MagicMock()
        from skreg_infra.providers.k8s.network import K8sNetwork

        net = K8sNetwork.__new__(K8sNetwork)
        net._traefik_release = mock_release.return_value
        assert net._traefik_release is not None
