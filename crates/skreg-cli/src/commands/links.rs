//! `skreg links` — list all tracked skill symlinks.

use anyhow::Result;

use crate::linker::Linker;

/// Run `skreg links`.
///
/// Prints all symlinks tracked in `~/.skreg/links.toml`, grouped by package.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn run_links() -> Result<()> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let links_path = home.join(".skreg").join("links.toml");
    let linker = Linker::new(links_path);

    if linker.links().is_empty() {
        println!("No tracked symlinks.");
        return Ok(());
    }

    let mut current_pkg = String::new();
    for record in linker.links() {
        if record.package != current_pkg {
            println!("{}", record.package);
            current_pkg.clone_from(&record.package);
        }
        println!("  {}", record.path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_module_compiles() {}
}
