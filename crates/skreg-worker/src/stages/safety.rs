//! Stage 3: safety checks — name squatting and yanked re-upload detection.

use thiserror::Error;

/// Errors returned by [`check_safety`].
#[derive(Debug, Error)]
pub enum SafetyError {
    /// The submitted package name is too similar to an existing one.
    #[error("name '{0}' is too similar to existing package '{1}' (Levenshtein distance {2})")]
    NameSquatting(String, String, usize),
    /// The package version was previously yanked and cannot be re-published.
    #[error("package '{0}' was previously yanked and cannot be re-published at the same version")]
    YankedVersion(String),
}

/// Compute the Levenshtein edit distance between two strings.
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Return `true` if `name` is within Levenshtein distance 2 of any existing name.
#[must_use]
pub fn is_squatting(name: &str, existing: &[String]) -> bool {
    existing.iter().any(|e| {
        let dist = levenshtein(name, e);
        dist > 0 && dist <= 2
    })
}

/// Run Stage 3 safety checks.
///
/// # Errors
///
/// Returns [`SafetyError::NameSquatting`] if the name is too close to an existing package,
/// or [`SafetyError::YankedVersion`] if this version was previously yanked.
pub fn check_safety(
    name: &str,
    version: &str,
    existing_names: &[String],
    yanked_versions: &[(String, String)],
) -> Result<(), SafetyError> {
    // Squatting check
    if let Some(existing) = existing_names.iter().find(|e| {
        let d = levenshtein(name, e);
        d > 0 && d <= 2
    }) {
        let dist = levenshtein(name, existing);
        return Err(SafetyError::NameSquatting(
            name.to_owned(),
            existing.clone(),
            dist,
        ));
    }

    // Yanked re-upload check
    if yanked_versions
        .iter()
        .any(|(n, v)| n == name && v == version)
    {
        return Err(SafetyError::YankedVersion(format!("{name}@{version}")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_identical_is_zero() {
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(levenshtein("abc", "abx"), 1);
    }

    #[test]
    fn levenshtein_two_edits() {
        assert_eq!(levenshtein("abc", "xyz"), 3);
    }

    #[test]
    fn squatting_detected_within_two() {
        assert!(is_squatting("reakt", &["react".to_owned()]));
    }

    #[test]
    fn no_squatting_when_clear() {
        assert!(!is_squatting(
            "my-unique-skill-name-xyz",
            &["react".to_owned()]
        ));
    }
}
