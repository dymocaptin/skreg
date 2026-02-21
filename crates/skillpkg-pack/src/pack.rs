//! Creates a gzip-compressed `.skill` tarball from a directory.

use std::fs::File;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use log::debug;

use crate::error::PackError;

/// Files that MUST be present in the source directory.
const REQUIRED_FILES: &[&str] = &["SKILL.md", "manifest.json"];

/// Pack a directory into a gzip-compressed `.skill` tarball at `output_path`.
///
/// All files in `source_dir` are included. Hidden files and `.git` directories
/// are excluded. The output file is created or truncated.
///
/// # Errors
///
/// Returns [`PackError::MissingFile`] if any required file is absent, or
/// [`PackError::Io`] on any I/O failure.
pub fn pack_directory(source_dir: &Path, output_path: &Path) -> Result<(), PackError> {
    for required in REQUIRED_FILES {
        if !source_dir.join(required).exists() {
            return Err(PackError::MissingFile((*required).to_owned()));
        }
    }

    let file = File::create(output_path)?;
    let encoder = GzEncoder::new(file, Compression::best());
    let mut archive = tar::Builder::new(encoder);
    archive.follow_symlinks(false);

    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') || name_str == ".git" {
            continue;
        }

        let path = entry.path();
        debug!("packing: {}", path.display());

        if path.is_dir() {
            archive.append_dir_all(&name, &path)?;
        } else {
            archive.append_path_with_name(&path, &name)?;
        }
    }

    archive.finish()?;
    Ok(())
}
