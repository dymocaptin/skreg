rule package_manager_creds {
    meta:
        description = "Detects attempts to read or exfiltrate package manager credential files"
        severity = "Error"
    strings:
        $npmrc       = "~/.npmrc"             nocase
        $pypirc      = "~/.pypirc"            nocase
        $gemrc       = "~/.gemrc"             nocase
        $cargo_creds = "~/.cargo/credentials" nocase
        $pip_conf    = "~/.pip/pip.conf"      nocase
        $curl        = "curl "                nocase
        $wget        = "wget "                nocase
        $cat_cmd     = "cat "                 nocase
        $grep_cmd    = "grep "                nocase
    condition:
        any of ($npmrc, $pypirc, $gemrc, $cargo_creds, $pip_conf)
        and any of ($curl, $wget, $cat_cmd, $grep_cmd)
}
