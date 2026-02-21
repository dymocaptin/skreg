//! Extracts a gzip-compressed `.skill` tarball into a target directory.

use std::fs::File;
use std::path::Path;

use flate2::read::GzDecoder;
use log::debug;

use crate::error::PackError;

/// Unpack a `.skill` tarball into `dest_dir`.
///
/// The destination directory is created if it does not exist.
/// Existing files in `dest_dir` are overwritten.
///
/// # Errors
///
/// Returns [`PackError::Io`] on any I/O or decompression failure.
pub fn unpack_tarball(tarball_path: &Path, dest_dir: &Path) -> Result<(), PackError> {
    std::fs::create_dir_all(dest_dir)?;
    let file = File::open(tarball_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        debug!("unpacking: {}", path.display());
        entry.unpack_in(dest_dir)?;
    }

    Ok(())
}
