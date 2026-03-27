rule encrypted_exfil {
    meta:
        description = "Detects data encryption combined with network exfiltration"
        severity = "Error"
    strings:
        $openssl_enc = "openssl enc"   nocase
        $openssl_aes = "openssl aes"   nocase
        $gpg_enc     = "gpg --encrypt" nocase
        $gpg_enc2    = "gpg -e "       nocase
        $curl        = "curl "         nocase
        $wget        = "wget "         nocase
    condition:
        ($openssl_enc or $openssl_aes or $gpg_enc or $gpg_enc2) and ($curl or $wget)
}
