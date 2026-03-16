//! Registry policy limits used across the worker and CLI.
//!
//! All stage logic MUST reference these constants by name — no inline
//! magic numbers. The limits are surfaced in `skreg publish --help` and
//! the TUI help panel.

/// Maximum total number of files in a package.
pub const LIMIT_MAX_FILES: usize = 50;

/// Maximum total package size in bytes (5 MB).
pub const LIMIT_PACKAGE_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum size of any individual file in bytes (512 KB).
pub const LIMIT_FILE_SIZE: u64 = 512 * 1024;

/// Maximum size of any file under `scripts/` in bytes (128 KB).
pub const LIMIT_SCRIPT_FILE_SIZE: u64 = 128 * 1024;

/// Maximum number of lines in `SKILL.md`.
pub const LIMIT_SKILL_MD_LINES: usize = 1000;

/// Maximum size of `manifest.json` in bytes (64 KB).
pub const LIMIT_MANIFEST_SIZE: u64 = 64 * 1024;

const _: () = {
    const _: () = assert!(LIMIT_SCRIPT_FILE_SIZE < LIMIT_FILE_SIZE);
    const _: () = assert!(LIMIT_MANIFEST_SIZE <= LIMIT_FILE_SIZE);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_size_sanity() {
        assert_eq!(LIMIT_PACKAGE_SIZE, 5 * 1024 * 1024);
    }
}
