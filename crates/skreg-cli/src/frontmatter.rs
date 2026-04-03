//! Parses SKILL.md frontmatter to extract skreg-required metadata.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use semver::Version;
use serde::Deserialize;

/// Metadata extracted from a `SKILL.md` frontmatter block.
#[derive(Debug)]
pub struct SkillMetadata {
    /// The skill name slug.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Semver version from `metadata.version`.
    pub version: Version,
}

#[derive(Deserialize)]
struct RawFrontmatter {
    name: Option<String>,
    description: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

/// Read and parse the YAML frontmatter from a `SKILL.md` file.
///
/// # Errors
///
/// Returns an error if the file cannot be read, frontmatter is missing or malformed,
/// or any required field (`name`, `description`, `metadata.version`) is absent or invalid.
pub fn read_skill_metadata(skill_md_path: &Path) -> Result<SkillMetadata> {
    let content = std::fs::read_to_string(skill_md_path)
        .with_context(|| format!("cannot read {}", skill_md_path.display()))?;
    parse_skill_metadata(&content)
}

fn parse_skill_metadata(content: &str) -> Result<SkillMetadata> {
    if !content.starts_with("---") {
        bail!("SKILL.md is missing YAML frontmatter");
    }
    let after_open = &content[3..];
    let close_pos = after_open
        .find("\n---\n")
        .or_else(|| after_open.strip_suffix("\n---").map(str::len))
        .context("SKILL.md frontmatter is not closed (missing closing ---)")?;
    let yaml_block = &after_open[..close_pos];

    let raw: RawFrontmatter =
        serde_yaml::from_str(yaml_block).context("SKILL.md frontmatter is not valid YAML")?;

    let name = raw
        .name
        .filter(|s| !s.is_empty())
        .context("SKILL.md frontmatter is missing required field 'name'")?;

    let description = raw
        .description
        .filter(|s| !s.is_empty())
        .context("SKILL.md frontmatter is missing required field 'description'")?;

    let version_str = raw
        .metadata
        .as_ref()
        .and_then(|m| m.get("version").cloned())
        .context("SKILL.md frontmatter is missing required field 'metadata.version'")?;

    let version = Version::parse(&version_str).with_context(|| {
        format!("SKILL.md metadata.version {version_str:?} is not valid semver")
    })?;

    Ok(SkillMetadata {
        name,
        description,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frontmatter() {
        let md = "---\nname: my-skill\ndescription: Does something useful\nmetadata:\n  version: \"1.2.3\"\n---\n# Body\n";
        let meta = parse_skill_metadata(md).unwrap();
        assert_eq!(meta.name, "my-skill");
        assert_eq!(meta.description, "Does something useful");
        assert_eq!(meta.version, Version::parse("1.2.3").unwrap());
    }

    #[test]
    fn missing_frontmatter_errors() {
        let err = parse_skill_metadata("# No frontmatter\n").unwrap_err();
        assert!(
            err.to_string().contains("missing YAML frontmatter"),
            "got: {err}"
        );
    }

    #[test]
    fn missing_version_errors() {
        let md = "---\nname: my-skill\ndescription: Something useful\n---\n";
        let err = parse_skill_metadata(md).unwrap_err();
        assert!(err.to_string().contains("metadata.version"), "got: {err}");
    }

    #[test]
    fn invalid_semver_errors() {
        let md = "---\nname: my-skill\ndescription: Something useful\nmetadata:\n  version: \"not-semver\"\n---\n";
        let err = parse_skill_metadata(md).unwrap_err();
        assert!(err.to_string().contains("not valid semver"), "got: {err}");
    }

    #[test]
    fn missing_name_errors() {
        let md = "---\ndescription: Something useful\nmetadata:\n  version: \"1.0.0\"\n---\n";
        let err = parse_skill_metadata(md).unwrap_err();
        assert!(err.to_string().contains("'name'"), "got: {err}");
    }

    #[test]
    fn missing_description_errors() {
        let md = "---\nname: my-skill\nmetadata:\n  version: \"1.0.0\"\n---\n";
        let err = parse_skill_metadata(md).unwrap_err();
        assert!(err.to_string().contains("'description'"), "got: {err}");
    }
}
