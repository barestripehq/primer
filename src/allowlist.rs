use std::fs;
use std::path::PathBuf;

use anyhow::Result;

const IGNORE_FILE: &str = ".motionstream-ignore";

/// Check if a package is in the nearest .motionstream-ignore file.
/// Walks up from the current directory, like .gitignore lookup.
pub fn is_allowed(package: &str, ecosystem: &str) -> bool {
    find_ignore_file()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|contents| file_allows(&contents, package, ecosystem))
        .unwrap_or(false)
}

/// Add a package to .motionstream-ignore in the current directory.
pub fn add(package: &str, ecosystem: Option<&str>) -> Result<()> {
    let path = PathBuf::from(IGNORE_FILE);
    let entry = match ecosystem {
        Some(eco) => format!("{}:{}\n", eco.to_lowercase(), package),
        None => format!("{}\n", package),
    };

    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == entry.trim()) {
        println!("  · {} is already in {}", package, IGNORE_FILE);
        return Ok(());
    }

    fs::write(&path, format!("{}{}", existing, entry))?;
    println!("  ✓ Added '{}' to {}", entry.trim(), IGNORE_FILE);
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_ignore_file() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(IGNORE_FILE);
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn file_allows(contents: &str, package: &str, ecosystem: &str) -> bool {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: "ecosystem:package" or just "package"
        if let Some((eco, pkg)) = line.split_once(':') {
            if eco.eq_ignore_ascii_case(ecosystem) && pkg.eq_ignore_ascii_case(package) {
                return true;
            }
        } else if line.eq_ignore_ascii_case(package) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_bare_package_name() {
        assert!(file_allows("requests\n", "requests", "PyPI"));
    }

    #[test]
    fn allows_ecosystem_qualified_entry() {
        assert!(file_allows("pypi:requests\n", "requests", "PyPI"));
    }

    #[test]
    fn rejects_different_package() {
        assert!(!file_allows("flask\n", "requests", "PyPI"));
    }

    #[test]
    fn rejects_wrong_ecosystem() {
        assert!(!file_allows("npm:requests\n", "requests", "PyPI"));
    }

    #[test]
    fn ignores_comment_lines() {
        let contents = "# allow requests for now\nrequests\n";
        assert!(file_allows(contents, "requests", "PyPI"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(file_allows("PyPI:Requests\n", "requests", "pypi"));
    }

    #[test]
    fn empty_file_allows_nothing() {
        assert!(!file_allows("", "requests", "PyPI"));
    }
}
