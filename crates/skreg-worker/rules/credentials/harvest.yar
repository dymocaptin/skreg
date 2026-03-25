rule aws_credential_harvest {
    meta:
        description = "Detects attempts to read AWS credentials"
        severity = "Error"
    strings:
        $aws_creds = "~/.aws/credentials" nocase
        $aws_config = "~/.aws/config" nocase
        $aws_key_env = "AWS_SECRET_ACCESS_KEY" nocase
        $aws_id_env  = "AWS_ACCESS_KEY_ID" nocase
    condition:
        any of them
}

rule ssh_key_harvest {
    meta:
        description = "Detects attempts to read SSH private keys"
        severity = "Error"
    strings:
        $ssh_id_rsa  = "~/.ssh/id_rsa" nocase
        $ssh_id_ed   = "~/.ssh/id_ed25519" nocase
        $ssh_dir     = "~/.ssh/" nocase
    condition:
        $ssh_id_rsa or $ssh_id_ed or $ssh_dir
}

rule crypto_miner {
    meta:
        description = "Detects cryptocurrency miner indicators"
        severity = "Error"
    strings:
        $stratum1 = "stratum+tcp://" nocase
        $stratum2 = "stratum+ssl://" nocase
        $pool1    = "pool.minergate.com" nocase
        $pool2    = "xmrpool.eu" nocase
        $pool3    = "moneropool.com" nocase
        $xmrig    = "xmrig" nocase
    condition:
        any of ($stratum1, $stratum2) or 2 of ($pool1, $pool2, $pool3) or $xmrig
}

rule c2_framework {
    meta:
        description = "Detects known C2 framework signatures"
        severity = "Error"
    strings:
        $msf1    = "msfvenom" nocase
        $cs1     = "cobaltstrike" nocase
        $empire1 = "powershell-empire" nocase
        $sliver1 = "sliver-server" nocase
    condition:
        any of them
}

rule sensitive_paths {
    meta:
        description = "Detects access to sensitive credential/config paths beyond AWS and SSH"
        severity = "Error"
    strings:
        $gnupg     = "~/.gnupg" nocase
        $kube      = "~/.kube/config" nocase
        $docker    = "~/.docker/config.json" nocase
        $shadow    = "/etc/shadow" nocase
        $passwd    = "/etc/passwd" nocase
        $netrc     = "~/.netrc" nocase
        $xdg       = "~/.config" nocase
    condition:
        any of them
}

rule sensitive_env_vars {
    meta:
        description = "Detects use of sensitive environment variables via shell sigil"
        severity = "Error"
    strings:
        $gh_token   = "$GITHUB_TOKEN" nocase
        $ssh_auth   = "$SSH_AUTH_SOCK" nocase
        $password   = "$PASSWORD" nocase
        $api_key    = "$API_KEY" nocase
        $secret     = "$SECRET" nocase
        $token      = "$TOKEN" nocase
    condition:
        any of them
}
