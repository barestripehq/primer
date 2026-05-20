use std::fs;

use anyhow::Result;
use serde::Serialize;

use crate::engine::osv::Vulnerability;

const REPORT_FILE: &str = "motionstream-report.json";

#[derive(Serialize)]
struct Report<'a> {
    package: &'a str,
    ecosystem: &'a str,
    blocked: bool,
    findings: Vec<Finding<'a>>,
}

#[derive(Serialize)]
struct Finding<'a> {
    id: &'a str,
    severity: &'a str,
    summary: Option<&'a str>,
    cvss_vector: Option<&'a str>,
}

/// Write findings to `motionstream-report.json` in the current directory.
pub fn write(package: &str, ecosystem: &str, vulns: &[Vulnerability]) -> Result<()> {
    write_to_dir(std::path::Path::new("."), package, ecosystem, vulns)
}

pub(crate) fn write_to_dir(
    dir: &std::path::Path,
    package: &str,
    ecosystem: &str,
    vulns: &[Vulnerability],
) -> Result<()> {
    let blocked = vulns.iter().any(|v| matches!(v.severity_label(), "CRITICAL" | "HIGH"));

    let findings = vulns
        .iter()
        .map(|v| Finding {
            id: &v.id,
            severity: v.severity_label(),
            summary: v.summary.as_deref(),
            cvss_vector: v.cvss_vector.as_deref(),
        })
        .collect();

    let report = Report { package, ecosystem, blocked, findings };
    fs::write(dir.join(REPORT_FILE), serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::osv::Vulnerability;

    fn vuln(id: &str, severity: &str) -> Vulnerability {
        Vulnerability {
            id: id.to_owned(),
            summary: Some("test".into()),
            cvss_vector: None,
            severity: Some(severity.to_owned()),
        }
    }

    #[test]
    fn report_serialises_correctly() {
        let vulns = vec![vuln("GHSA-0001", "CRITICAL"), vuln("GHSA-0002", "LOW")];
        let dir = tempfile::tempdir().unwrap();

        write_to_dir(dir.path(), "requests", "PyPI", &vulns).unwrap();

        let contents = std::fs::read_to_string(dir.path().join(REPORT_FILE)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();

        assert_eq!(json["package"], "requests");
        assert_eq!(json["ecosystem"], "PyPI");
        assert_eq!(json["blocked"], true);
        assert_eq!(json["findings"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn report_not_blocked_when_no_critical_or_high() {
        let vulns = vec![vuln("GHSA-0001", "LOW")];
        let dir = tempfile::tempdir().unwrap();

        write_to_dir(dir.path(), "pkg", "PyPI", &vulns).unwrap();

        let contents = std::fs::read_to_string(dir.path().join(REPORT_FILE)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["blocked"], false);
    }
}
