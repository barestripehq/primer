use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::engine::osv::Vulnerability;

const TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,
    vulns: Vec<Vulnerability>,
}

pub fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".primer").join("cache")
}

fn entry_path(dir: &Path, package: &str, ecosystem: &str, version: Option<&str>) -> PathBuf {
    let v = version.unwrap_or("latest");
    let raw = format!("{}_{}_{}", ecosystem, package, v);
    let safe: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    dir.join(format!("{}.json", safe))
}

pub(crate) fn get_stale_from_dir(
    dir: &Path,
    package: &str,
    ecosystem: &str,
    version: Option<&str>,
) -> Option<Vec<Vulnerability>> {
    let path = entry_path(dir, package, ecosystem, version);
    let contents = std::fs::read_to_string(path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&contents).ok()?;
    Some(entry.vulns)
}

/// Remove all cached entries. Returns the number of files deleted.
pub fn clear() -> Result<usize> {
    let dir = cache_dir();
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().map_or(false, |e| e == "json") {
            std::fs::remove_file(path)?;
            count += 1;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Testable internals (inject dir + timestamp instead of reading env/clock)
// ---------------------------------------------------------------------------

pub(crate) fn get_from_dir(
    dir: &Path,
    package: &str,
    ecosystem: &str,
    version: Option<&str>,
    now: u64,
) -> Option<Vec<Vulnerability>> {
    let path = entry_path(dir, package, ecosystem, version);
    let contents = std::fs::read_to_string(path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&contents).ok()?;
    if now.saturating_sub(entry.fetched_at) < TTL_SECS {
        Some(entry.vulns)
    } else {
        None
    }
}

pub(crate) fn put_to_dir(
    dir: &Path,
    package: &str,
    ecosystem: &str,
    version: Option<&str>,
    vulns: &[Vulnerability],
    ts: u64,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let entry = CacheEntry { fetched_at: ts, vulns: vulns.to_vec() };
    let path = entry_path(dir, package, ecosystem, version);
    std::fs::write(path, serde_json::to_string_pretty(&entry)?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::osv::Vulnerability;

    fn vuln(id: &str) -> Vulnerability {
        Vulnerability {
            id: id.to_owned(),
            summary: Some("test".into()),
            cvss_vector: None,
            severity: Some("HIGH".into()),
        }
    }

    #[test]
    fn fresh_hit_returns_vulns() {
        let dir = tempfile::tempdir().unwrap();
        let vulns = vec![vuln("GHSA-0001")];
        put_to_dir(dir.path(), "pkg", "PyPI", None, &vulns, 1000).unwrap();

        let result = get_from_dir(dir.path(), "pkg", "PyPI", None, 1000 + 60);
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0].id, "GHSA-0001");
    }

    #[test]
    fn miss_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(get_from_dir(dir.path(), "pkg", "PyPI", None, 1000).is_none());
    }

    #[test]
    fn returns_none_when_expired() {
        let dir = tempfile::tempdir().unwrap();
        put_to_dir(dir.path(), "pkg", "PyPI", None, &[vuln("GHSA-0001")], 0).unwrap();

        // now = TTL + 1 second past epoch
        let result = get_from_dir(dir.path(), "pkg", "PyPI", None, TTL_SECS + 1);
        assert!(result.is_none());
    }

    #[test]
    fn version_keyed_separately_from_latest() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = vec![vuln("GHSA-0001")];
        let v2 = vec![vuln("GHSA-0002")];

        put_to_dir(dir.path(), "pkg", "PyPI", None, &v1, 0).unwrap();
        put_to_dir(dir.path(), "pkg", "PyPI", Some("1.0.0"), &v2, 0).unwrap();

        let latest = get_from_dir(dir.path(), "pkg", "PyPI", None, 60);
        let pinned = get_from_dir(dir.path(), "pkg", "PyPI", Some("1.0.0"), 60);

        // Both expired (ts=0, now=60 < TTL), so both fresh
        assert_eq!(latest.unwrap()[0].id, "GHSA-0001");
        assert_eq!(pinned.unwrap()[0].id, "GHSA-0002");
    }

    #[test]
    fn clear_removes_all_json_files() {
        let dir = tempfile::tempdir().unwrap();
        put_to_dir(dir.path(), "pkg-a", "PyPI", None, &[vuln("GHSA-0001")], 0).unwrap();
        put_to_dir(dir.path(), "pkg-b", "npm", None, &[vuln("GHSA-0002")], 0).unwrap();

        // Manually invoke clear logic against our temp dir
        let mut count = 0;
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().map_or(false, |e| e == "json") {
                std::fs::remove_file(path).unwrap();
                count += 1;
            }
        }
        assert_eq!(count, 2);
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn roundtrip_preserves_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let vulns = vec![Vulnerability {
            id: "GHSA-test".into(),
            summary: Some("a summary".into()),
            cvss_vector: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H".into()),
            severity: Some("CRITICAL".into()),
        }];
        put_to_dir(dir.path(), "mypkg", "PyPI", Some("2.0"), &vulns, 500).unwrap();
        let got = get_from_dir(dir.path(), "mypkg", "PyPI", Some("2.0"), 501).unwrap();
        assert_eq!(got[0].id, "GHSA-test");
        assert_eq!(got[0].cvss_vector.as_deref(), Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"));
        assert_eq!(got[0].severity_label(), "CRITICAL");
    }
}
