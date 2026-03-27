rule package_manager_creds {
    meta:
        description = "Detects attempts to read package manager credential files"
        severity = "Error"
    strings:
        $npmrc       = "~/.npmrc"             nocase
        $pypirc      = "~/.pypirc"            nocase
        $gemrc       = "~/.gemrc"             nocase
        $cargo_creds = "~/.cargo/credentials" nocase
        $pip_conf    = "~/.pip/pip.conf"      nocase
    condition:
        any of them
}
