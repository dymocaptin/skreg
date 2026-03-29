//! Creates a gzip-compressed `.skill` tarball from a directory.

use std::fs::File;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;
use log::debug;

use crate::error::PackError;

/// Files that MUST be present in the source directory.
const REQUIRED_FILES: &[&str] = &["SKILL.md"];

/// All files permitted at the root of a `.skill` tarball.
const ALLOWED_ROOT_FILES: &[&str] = &["SKILL.md", "manifest.json"];

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

        if name_str.starts_with('.') || name_str == ".git" || name_str.ends_with(".skill") {
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

/// Walk `source_dir` and append all allowed files to `tar`, skipping
/// `manifest.json` (which is injected synthetically by `pack_with_manifest`).
fn append_source_files<W: std::io::Write>(
    tar: &mut tar::Builder<W>,
    source_dir: &Path,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files, .git, and output .skill files.
        if name_str.starts_with('.') || name_str == ".git" || name_str.ends_with(".skill") {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            // Only allow known subdirectory names.
            let allowed_dirs = ["references", "scripts", "assets"];
            if !allowed_dirs.contains(&name_str.as_ref()) {
                debug!("skipping disallowed directory: {}", path.display());
                continue;
            }
            debug!("packing dir: {}", path.display());
            tar.append_dir_all(&name, &path)?;
        } else {
            // Skip manifest.json at the root — it is injected synthetically.
            if name_str == "manifest.json" {
                continue;
            }
            // Only allow known root files.
            if !ALLOWED_ROOT_FILES.contains(&name_str.as_ref()) {
                debug!("skipping disallowed root file: {}", path.display());
                continue;
            }
            debug!("packing file: {}", path.display());
            tar.append_path_with_name(&path, &name)?;
        }
    }
    Ok(())
}

/// Pack `source_dir` into a `.skill` tarball at `output_path`.
///
/// The `manifest` value is serialised to JSON and injected as a synthetic
/// `manifest.json` tar entry — the source directory is never modified.
///
/// # Errors
///
/// Returns an error if `SKILL.md` is missing, any required file is absent,
/// or any I/O or serialisation step fails.
pub fn pack_with_manifest(
    source_dir: &Path,
    manifest: &skreg_core::manifest::Manifest,
    output_path: &Path,
) -> anyhow::Result<()> {
    // Validate required files exist in source dir.
    for required in REQUIRED_FILES {
        let p = source_dir.join(required);
        if !p.exists() {
            anyhow::bail!("required file missing: {required}");
        }
    }

    let file = std::fs::File::create(output_path)?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Inject manifest.json as a synthetic in-memory entry (first).
    let manifest_json = serde_json::to_vec_pretty(manifest)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    tar.append_data(&mut header, "manifest.json", manifest_json.as_slice())?;

    // Append all source files (skip manifest.json if present in source).
    append_source_files(&mut tar, source_dir)?;

    tar.into_inner()?.finish()?;
    Ok(())
}
