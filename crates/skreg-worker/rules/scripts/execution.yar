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
