rule curl_pipe_bash {
    meta:
        description = "Detects curl|bash or wget|sh remote execution"
        severity = "Error"
    strings:
        $curl_bash1 = "curl " nocase
        $curl_bash2 = "| bash" nocase
        $curl_bash3 = "| sh" nocase
        $wget_bash1 = "wget " nocase
        $pipe_exec  = "|bash" nocase
    condition:
        ($curl_bash1 and ($curl_bash2 or $curl_bash3)) or
        ($wget_bash1 and ($curl_bash2 or $curl_bash3)) or
        $pipe_exec
}

rule base64_exec {
    meta:
        description = "Detects base64 decode combined with eval/exec"
        severity = "Error"
    strings:
        $b64_1 = "base64 -d" nocase
        $b64_2 = "base64 --decode" nocase
        $b64_py = "base64.b64decode" nocase
        $eval1  = "eval(" nocase
        $exec1  = "exec(" nocase
        $sh_eval = "| bash" nocase
    condition:
        (($b64_1 or $b64_2) and $sh_eval) or
        ($b64_py and ($eval1 or $exec1))
}

rule network_tools_exfil {
    meta:
        description = "Detects netcat-family tools used for data exfiltration or reverse shells"
        severity = "Error"
    strings:
        $nc1    = "nc "    nocase
        $nc2    = "ncat "  nocase
        $nc3    = "netcat " nocase
        $flag_e = "-e"     nocase
        $flag_l = "-lv"    nocase
        $pipe   = "|"
        $devnull = "> /dev/"
    condition:
        ($nc1 or $nc2 or $nc3) and ($flag_e or $flag_l or $pipe or $devnull)
}

rule network_transfer {
    meta:
        description = "Detects network transfer tools that may indicate data exfiltration"
        severity = "Warning"
    strings:
        $scp     = "scp "     nocase
        $rsync   = "rsync "   nocase
        $ftp     = "ftp "     nocase
        $telnet  = "telnet "  nocase
        $ssh     = "ssh "     nocase
    condition:
        any of them
}

rule privilege_escalation_setuid {
    meta:
        description = "Detects setuid/setgid bit manipulation — strong privilege escalation indicator"
        severity = "Error"
    strings:
        $chmod_s  = "chmod +s"  nocase
        $chown_r  = "chown root" nocase
    condition:
        any of them
}

rule privilege_escalation_sudo {
    meta:
        description = "Detects use of privilege escalation utilities"
        severity = "Warning"
    strings:
        $sudo   = "sudo "   nocase
        $pkexec = "pkexec " nocase
        $doas   = "doas "   nocase
    condition:
        any of them
}

rule inline_interpreter_exec {
    meta:
        description = "Detects inline interpreter one-liners used to execute arbitrary code"
        severity = "Error"
    strings:
        $py2   = "python -c"   nocase
        $py3   = "python3 -c"  nocase
        $perl  = "perl -e"     nocase
        $ruby  = "ruby -e"     nocase
        $node  = "node -e"     nocase
        $nodejs = "nodejs -e"  nocase
        $php   = "php -r"      nocase
    condition:
        any of them
}
