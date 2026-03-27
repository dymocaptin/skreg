rule wallet_harvest {
    meta:
        description = "Detects attempts to access cryptocurrency wallet files"
        severity = "Error"
    strings:
        $wallet_dat   = "wallet.dat"   nocase
        $bitcoin_dir  = "~/.bitcoin"   nocase
        $ethereum_dir = "~/.ethereum"  nocase
        $keystore_dir = "/keystore/"   nocase
    condition:
        any of them
}
