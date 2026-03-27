//! Extracts a gzip-compressed `.skill` tarball into a target directory.

use std::fs::File;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use log::debug;
use tempfile::TempDir;

use skreg_core::manifest::Manifest;

use crate::error::PackError;

/// Validate a single tar entry: reject symlinks and path-traversal components.
///
/// Returns the entry's path on success.
fn validate_entry<R: std::io::Read>(entry: &tar::Entry<R>) -> Result<PathBuf, PackError> {
    // Reject symlinks.
    if entry.header().entry_type().is_symlink() {
        let path = entry.path()?.display().to_string();
        return Err(PackError::Symlink(path));
    }

    // Reject path traversal.
    let path = entry.path()?.to_path_buf();
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PackError::PathTraversal(path.display().to_string()));
            }
            _ => {}
        }
    }

    Ok(path)
}

/// Unpack a `.skill` tarball into `dest_dir`.
///
/// The destination directory is created if it does not exist.
/// Existing files in `dest_dir` are overwritten.
///
/// # Errors
///
/// Returns [`PackError::Symlink`] if any entry is a symlink.
/// Returns [`PackError::PathTraversal`] if any entry path contains `..` or an absolute component.
/// Returns [`PackError::Io`] on any other I/O or decompression failure.
pub fn unpack_tarball(tarball_path: &Path, dest_dir: &Path) -> Result<(), PackError> {
    std::fs::create_dir_all(dest_dir)?;
    let file = File::open(tarball_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = validate_entry(&entry)?;
        debug!("unpacking: {}", path.display());
        entry.unpack_in(dest_dir)?;
    }

    Ok(())
}

/// Unpack a `.skill` tarball from an in-memory byte slice into a new temporary directory.
///
/// Rejects any entry that is a symlink or contains path traversal components (`..` or
/// absolute paths). The returned [`TempDir`] owns the directory; it is deleted when dropped.
///
/// # Errors
///
/// Returns [`PackError::Symlink`] if any entry is a symlink.
/// Returns [`PackError::PathTraversal`] if any entry path contains `..` or an absolute component.
/// Returns [`PackError::Io`] on any other I/O or decompression failure.
pub fn unpack_to_tempdir(bytes: &[u8]) -> Result<TempDir, PackError> {
    let tmp = TempDir::new()?;
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = validate_entry(&entry)?;
        debug!("unpacking: {}", path.display());
        entry.unpack_in(tmp.path())?;
    }

    Ok(tmp)
}

/// Read and deserialize `manifest.json` from an in-memory `.skill` tarball.
///
/// # Errors
///
/// Returns [`PackError::MissingFile`] if `manifest.json` is absent,
/// [`PackError::Io`] on decompression failure, or [`PackError::ManifestParse`]
/// if the JSON is malformed.
pub fn read_manifest_from_bytes(bytes: &[u8]) -> Result<Manifest, PackError> {
    use std::io::Read as _;

    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path == Path::new("manifest.json") {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            return serde_json::from_str(&contents)
                .map_err(|e| PackError::ManifestParse(e.to_string()));
        }
    }

    Err(PackError::MissingFile("manifest.json".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write as _;

    /// Build a gzip-compressed tar from normal (safe-path) entries via the `tar` crate.
    fn make_tarball(entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
        let buf = Vec::new();
        let enc = GzEncoder::new(buf, Compression::default());
        let mut ar = tar::Builder::new(enc);
        for (path, data, entry_type) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_entry_type(*entry_type);
            header.set_mode(0o644);
            header.set_cksum();
            ar.append_data(&mut header, path, *data).unwrap();
        }
        ar.into_inner().unwrap().finish().unwrap()
    }

    /// Build a single-entry tar with a raw (potentially malicious) path string,
    /// bypassing the `tar` crate's own path validation.
    ///
    /// Writes a POSIX-compatible 512-byte header block followed by the data block(s),
    /// then two 512-byte zero end-of-archive blocks, all wrapped in gzip.
    fn make_tarball_raw(path: &str, data: &[u8], type_flag: u8) -> Vec<u8> {
        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut w = enc;

        // Build a 512-byte header block.
        let mut header = [0u8; 512];

        // name field: bytes 0..100
        let name_bytes = path.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // mode: bytes 100..108  "0000644\0"
        header[100..107].copy_from_slice(b"0000644");

        // uid/gid: bytes 108..116, 116..124  — leave as zeros (null-terminated octal "0")
        header[108] = b'0';
        header[116] = b'0';

        // size: bytes 124..136  octal, null-terminated
        let size_octal = format!("{:011o}\0", data.len());
        header[124..136].copy_from_slice(size_octal.as_bytes());

        // mtime: bytes 136..148
        header[136..147].copy_from_slice(b"00000000000");

        // typeflag: byte 156
        header[156] = type_flag;

        // magic/version: bytes 257..265  (ustar\0  00)
        header[257..262].copy_from_slice(b"ustar");
        header[263..265].copy_from_slice(b"00");

        // Compute checksum: sum of all bytes with checksum field treated as spaces (0x20).
        for b in &mut header[148..156] {
            *b = b' ';
        }
        let cksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        let cksum_str = format!("{cksum:06o}\0 ");
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        w.write_all(&header).unwrap();

        // Data blocks (rounded up to 512-byte boundary).
        if !data.is_empty() {
            w.write_all(data).unwrap();
            let pad = (512 - (data.len() % 512)) % 512;
            if pad > 0 {
                w.write_all(&vec![0u8; pad]).unwrap();
            }
        }

        // Two end-of-archive blocks.
        w.write_all(&[0u8; 1024]).unwrap();

        w.finish().unwrap()
    }

    #[test]
    fn rejects_symlink_entry() {
        let bytes = make_tarball(&[("SKILL.md", b"content", tar::EntryType::Symlink)]);
        let err = unpack_to_tempdir(&bytes).unwrap_err();
        assert!(matches!(err, PackError::Symlink(_)), "got: {err}");
    }

    #[test]
    fn rejects_path_traversal_dotdot() {
        // type flag '0' = regular file; path contains ".."
        let bytes = make_tarball_raw("../evil.txt", b"bad", b'0');
        let err = unpack_to_tempdir(&bytes).unwrap_err();
        assert!(matches!(err, PackError::PathTraversal(_)), "got: {err}");
    }

    #[test]
    fn rejects_absolute_path() {
        let bytes = make_tarball_raw("/etc/passwd", b"bad", b'0');
        let err = unpack_to_tempdir(&bytes).unwrap_err();
        assert!(matches!(err, PackError::PathTraversal(_)), "got: {err}");
    }

    #[test]
    fn accepts_normal_entries() {
        let bytes = make_tarball(&[
            ("SKILL.md", b"# skill", tar::EntryType::Regular),
            ("manifest.json", b"{}", tar::EntryType::Regular),
        ]);
        assert!(unpack_to_tempdir(&bytes).is_ok());
    }
}
