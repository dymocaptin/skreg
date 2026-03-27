rule reverse_shell_bash {
    meta:
        description = "Detects common bash reverse shell one-liners"
        severity = "Error"
    strings:
        $tcp1 = "bash -i >& /dev/tcp/" nocase
        $tcp2 = "bash -i>&/dev/tcp/" nocase
        $nc_e1 = "nc -e /bin/bash" nocase
        $nc_e2 = "nc -e /bin/sh" nocase
        $mkfifo = "mkfifo /tmp/" nocase
        $python_rs = "socket.connect(" nocase
    condition:
        any of ($tcp1, $tcp2, $nc_e1, $nc_e2) or ($mkfifo and $python_rs)
}

rule reverse_shell_python {
    meta:
        description = "Detects Python reverse shell patterns"
        severity = "Error"
    strings:
        $import_socket = "import socket" nocase
        $connect = ".connect((" nocase
        $dup2 = "os.dup2(" nocase
        $execl = "os.execl(" nocase
    condition:
        $import_socket and $connect and ($dup2 or $execl)
}
