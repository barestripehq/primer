/// A newly added package extracted from a staged manifest diff.
#[derive(Debug, PartialEq)]
pub struct NewPackage {
    pub name: String,
    pub version: Option<String>,
    pub ecosystem: &'static str,
}

/// All manifest filenames that the hook monitors.
pub const MONITORED_MANIFESTS: &[&str] =
    &["requirements.txt", "package.json", "go.mod", "Cargo.toml"];

/// Parse `git diff --cached` output for a single manifest file and return
/// packages that were newly added (lines beginning with `+` that are not the
/// diff header).
pub fn parse_diff(filename: &str, diff_text: &str) -> Vec<NewPackage> {
    match filename {
        "requirements.txt" => parse_requirements(diff_text),
        "package.json" => parse_package_json(diff_text),
        "go.mod" => parse_go_mod(diff_text),
        "Cargo.toml" => parse_cargo_toml(diff_text),
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// requirements.txt
// ---------------------------------------------------------------------------

fn parse_requirements(diff: &str) -> Vec<NewPackage> {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .filter_map(|l| {
            let line = l[1..].trim();
            // Skip comments, blank lines, -r includes, options
            if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
                return None;
            }
            // Split on first version specifier character: ==, >=, <=, ~=, !=, >
            let (name, version) = split_requirement(line);
            Some(NewPackage {
                name: name.trim().to_lowercase(),
                version: version.map(|v| v.trim().to_string()),
                ecosystem: "PyPI",
            })
        })
        .collect()
}

/// Split `name==1.0` into `("name", Some("1.0"))`.
fn split_requirement(s: &str) -> (&str, Option<&str>) {
    // Find the first occurrence of a version specifier operator
    let ops = ["==", ">=", "<=", "~=", "!=", ">", "<", "@"];
    let mut split_at = s.len();
    let mut op_len = 0;
    for op in ops {
        if let Some(pos) = s.find(op)
            && pos < split_at
        {
            split_at = pos;
            op_len = op.len();
        }
    }
    if split_at == s.len() {
        (s, None)
    } else {
        (&s[..split_at], Some(&s[split_at + op_len..]))
    }
}

// ---------------------------------------------------------------------------
// package.json
// ---------------------------------------------------------------------------

fn parse_package_json(diff: &str) -> Vec<NewPackage> {
    let mut results = Vec::new();
    // Match lines like:  +    "package-name": "^1.2.3",
    // We are in a dependencies/devDependencies block when we see such lines.
    for line in diff.lines() {
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }
        let trimmed = line[1..].trim();
        // Expect: "name": "version"
        if let Some((name, version)) = parse_json_dep_line(trimmed) {
            results.push(NewPackage {
                name,
                version: Some(version),
                ecosystem: "npm",
            });
        }
    }
    results
}

/// Parse `"name": "version",` → `("name", "version")`.
fn parse_json_dep_line(s: &str) -> Option<(String, String)> {
    // Must start with a quoted key
    let s = s.strip_prefix('"')?;
    let (name, rest) = s.split_once('"')?;
    // Skip colon + whitespace
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    // Version value in quotes
    let rest = rest.strip_prefix('"')?;
    let (raw_ver, _) = rest.split_once('"')?;
    // Strip semver range prefixes (^, ~, >=, etc.)
    let version = raw_ver
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .to_string();
    Some((name.to_string(), version))
}

// ---------------------------------------------------------------------------
// go.mod
// ---------------------------------------------------------------------------

fn parse_go_mod(diff: &str) -> Vec<NewPackage> {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .filter_map(|l| {
            let line = l[1..].trim();
            // Lines inside a require block: "module/path v1.2.3"
            // or inline: "require module/path v1.2.3"
            let line = line
                .strip_prefix("require")
                .map(|s| s.trim())
                .unwrap_or(line);
            // Skip blank, comments, go directive, module directive, opening parens
            if line.is_empty()
                || line.starts_with("//")
                || line.starts_with("go ")
                || line.starts_with("module ")
                || line == "("
                || line == ")"
            {
                return None;
            }
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let version = parts.next().map(|v| v.trim_start_matches('v').to_string());
            Some(NewPackage {
                name: name.to_string(),
                version,
                ecosystem: "Go",
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cargo.toml
// ---------------------------------------------------------------------------

fn parse_cargo_toml(diff: &str) -> Vec<NewPackage> {
    let mut results = Vec::new();
    let mut in_deps = false;

    for line in diff.lines() {
        // Track section headers (unchanged or added)
        let bare = if line.starts_with('+') || line.starts_with(' ') {
            &line[1..]
        } else {
            continue;
        };
        let trimmed = bare.trim();

        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]"
                || trimmed == "[dev-dependencies]"
                || trimmed == "[build-dependencies]";
            continue;
        }

        if !in_deps || !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }

        if let Some((name, version)) = parse_cargo_dep_line(trimmed) {
            results.push(NewPackage {
                name,
                version,
                ecosystem: "crates.io",
            });
        }
    }
    results
}

/// Parse `name = "1.0"` or `name = { version = "1.0", ... }`.
fn parse_cargo_dep_line(s: &str) -> Option<(String, Option<String>)> {
    let (name, rest) = s.split_once('=')?;
    let name = name.trim().to_string();
    if name.is_empty() || name.starts_with('#') {
        return None;
    }
    let rest = rest.trim();

    // Simple string form: "1.0.0"
    if rest.starts_with('"') {
        let version = rest.trim_matches('"').to_string();
        return Some((name, Some(version)));
    }

    // Table form: { version = "1.0", ... }
    if rest.starts_with('{') {
        if let Some(ver) = extract_version_from_table(rest) {
            return Some((name, Some(ver)));
        }
        return Some((name, None));
    }

    None
}

fn extract_version_from_table(s: &str) -> Option<String> {
    // Find version = "..."
    let after = s.find("version")? + "version".len();
    let rest = s[after..].trim_start();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim_start().strip_prefix('"')?;
    let (ver, _) = rest.split_once('"')?;
    Some(ver.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- requirements.txt ---

    #[test]
    fn requirements_simple_pinned() {
        let diff = "+requests==2.25.1\n";
        let pkgs = parse_diff("requirements.txt", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "requests");
        assert_eq!(pkgs[0].version.as_deref(), Some("2.25.1"));
        assert_eq!(pkgs[0].ecosystem, "PyPI");
    }

    #[test]
    fn requirements_ge_specifier() {
        let diff = "+pillow>=9.0.0\n";
        let pkgs = parse_diff("requirements.txt", diff);
        assert_eq!(pkgs[0].name, "pillow");
        assert_eq!(pkgs[0].version.as_deref(), Some("9.0.0"));
    }

    #[test]
    fn requirements_no_version() {
        let diff = "+flask\n";
        let pkgs = parse_diff("requirements.txt", diff);
        assert_eq!(pkgs[0].name, "flask");
        assert_eq!(pkgs[0].version, None);
    }

    #[test]
    fn requirements_skips_comments_and_blank() {
        let diff = "+# comment\n+\n+requests==2.28.0\n";
        let pkgs = parse_diff("requirements.txt", diff);
        assert_eq!(pkgs.len(), 1);
    }

    #[test]
    fn requirements_skips_diff_header() {
        let diff = "+++ b/requirements.txt\n+requests==2.28.0\n";
        let pkgs = parse_diff("requirements.txt", diff);
        assert_eq!(pkgs.len(), 1);
    }

    // --- package.json ---

    #[test]
    fn package_json_caret_version() {
        let diff = "+    \"express\": \"^4.18.2\",\n";
        let pkgs = parse_diff("package.json", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "express");
        assert_eq!(pkgs[0].version.as_deref(), Some("4.18.2"));
        assert_eq!(pkgs[0].ecosystem, "npm");
    }

    #[test]
    fn package_json_tilde_version() {
        let diff = "+    \"lodash\": \"~4.17.21\",\n";
        let pkgs = parse_diff("package.json", diff);
        assert_eq!(pkgs[0].name, "lodash");
        assert_eq!(pkgs[0].version.as_deref(), Some("4.17.21"));
    }

    #[test]
    fn package_json_skips_unchanged() {
        let diff = " \"react\": \"^18.0.0\",\n+\"axios\": \"^1.4.0\",\n";
        let pkgs = parse_diff("package.json", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "axios");
    }

    // --- go.mod ---

    #[test]
    fn go_mod_inline_require() {
        let diff = "+require github.com/gin-gonic/gin v1.9.1\n";
        let pkgs = parse_diff("go.mod", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "github.com/gin-gonic/gin");
        assert_eq!(pkgs[0].version.as_deref(), Some("1.9.1"));
        assert_eq!(pkgs[0].ecosystem, "Go");
    }

    #[test]
    fn go_mod_block_entry() {
        let diff = "+\tgithub.com/stretchr/testify v1.8.4\n";
        let pkgs = parse_diff("go.mod", diff);
        assert_eq!(pkgs[0].name, "github.com/stretchr/testify");
        assert_eq!(pkgs[0].version.as_deref(), Some("1.8.4"));
    }

    #[test]
    fn go_mod_skips_directives() {
        let diff = "+go 1.21\n+module myapp\n+require golang.org/x/net v0.17.0\n";
        let pkgs = parse_diff("go.mod", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "golang.org/x/net");
    }

    // --- Cargo.toml ---

    #[test]
    fn cargo_toml_simple_string() {
        let diff = " [dependencies]\n+serde = \"1.0\"\n";
        let pkgs = parse_diff("Cargo.toml", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "serde");
        assert_eq!(pkgs[0].version.as_deref(), Some("1.0"));
        assert_eq!(pkgs[0].ecosystem, "crates.io");
    }

    #[test]
    fn cargo_toml_table_form() {
        let diff = " [dependencies]\n+tokio = { version = \"1.35\", features = [\"full\"] }\n";
        let pkgs = parse_diff("Cargo.toml", diff);
        assert_eq!(pkgs[0].name, "tokio");
        assert_eq!(pkgs[0].version.as_deref(), Some("1.35"));
    }

    #[test]
    fn cargo_toml_ignores_non_deps_section() {
        let diff = " [package]\n+name = \"myapp\"\n [dependencies]\n+anyhow = \"1.0\"\n";
        let pkgs = parse_diff("Cargo.toml", diff);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "anyhow");
    }

    #[test]
    fn unknown_manifest_returns_empty() {
        let pkgs = parse_diff("poetry.lock", "+foo = \"1.0\"\n");
        assert!(pkgs.is_empty());
    }
}
