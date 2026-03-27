rule destructive_ops {
    meta:
        description = "Detects commands that irreversibly destroy data or storage"
        severity = "Error"
    strings:
        $rm_rf1  = "rm -rf"  nocase
        $rm_rf2  = "rm -fr"  nocase
        $dd      = "dd if="  nocase
        $shred   = "shred "  nocase
        $mkfs    = "mkfs."   nocase
        $wipefs  = "wipefs " nocase
    condition:
        any of them
}
