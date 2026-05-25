use std::fs;
use std::path::PathBuf;

use anyhow::Result;

const IGNORE_FILE: &str = ".primer-ignore";

/// Check if a package is in the nearest .primer-ignore file.
/// Walks up from the current directory, like .gitignore lookup.
pub fn is_allowed(package: &str, ecosystem: &str) -> bool {
    find_ignore_file()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|contents| file_allows(&contents, package, ecosystem))
        .unwrap_or(false)
}

/// Add a package to .primer-ignore in the current directory.
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

/// Remove a package from .primer-ignore in the current directory.
pub fn remove(package: &str, ecosystem: Option<&str>) -> Result<()> {
    let path = PathBuf::from(IGNORE_FILE);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let target = match ecosystem {
        Some(eco) => format!("{}:{}", eco.to_lowercase(), package),
        None => package.to_string(),
    };
    let (filtered, found) = filter_entry(&existing, &target);
    if !found {
        println!("  · '{}' was not in {}", target, IGNORE_FILE);
        return Ok(());
    }
    fs::write(&path, filtered)?;
    println!("  ✓ Removed '{}' from {}", target, IGNORE_FILE);
    Ok(())
}

/// Print all entries in the nearest .primer-ignore file.
pub fn list() -> Result<()> {
    match find_ignore_file() {
        None => println!("No {} found.", IGNORE_FILE),
        Some(path) => {
            let contents = fs::read_to_string(&path)?;
            let entries = visible_entries(&contents);
            if entries.is_empty() {
                println!("{} is empty.", path.display());
            } else {
                println!("Allow-list ({}):\n", path.display());
                for e in entries {
                    println!("  · {}", e);
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Filter `target` out of `contents`. Returns (filtered_string, was_found).
fn filter_entry(contents: &str, target: &str) -> (String, bool) {
    let filtered: String = contents
        .lines()
        .filter(|l| l.trim() != target)
        .flat_map(|l| [l, "\n"])
        .collect();
    let found = filtered != contents;
    (filtered, found)
}

/// Return the non-comment, non-blank lines from `contents`.
fn visible_entries(contents: &str) -> Vec<&str> {
    contents
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .collect()
}

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

    // --- filter_entry ---

    #[test]
    fn filter_entry_removes_matching_line() {
        let (out, found) = filter_entry("requests\nflask\n", "requests");
        assert!(found);
        assert!(!out.contains("requests"));
        assert!(out.contains("flask"));
    }

    #[test]
    fn filter_entry_reports_not_found() {
        let contents = "flask\n";
        let (out, found) = filter_entry(contents, "requests");
        assert!(!found);
        assert_eq!(out, contents);
    }

    #[test]
    fn filter_entry_removes_ecosystem_qualified_entry() {
        let (out, found) = filter_entry("pypi:requests\nflask\n", "pypi:requests");
        assert!(found);
        assert!(!out.contains("pypi:requests"));
        assert!(out.contains("flask"));
    }

    #[test]
    fn filter_entry_leaves_other_entries_intact() {
        let (out, found) = filter_entry("requests\nflask\ndjango\n", "flask");
        assert!(found);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, ["requests", "django"]);
    }

    // --- visible_entries ---

    #[test]
    fn visible_entries_skips_comments_and_blanks() {
        let contents = "# comment\n\nrequests\nflask\n";
        assert_eq!(visible_entries(contents), ["requests", "flask"]);
    }

    #[test]
    fn visible_entries_empty_when_all_comments() {
        assert!(visible_entries("# one\n# two\n").is_empty());
    }
}
