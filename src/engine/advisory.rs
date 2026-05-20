use anyhow::Result;
use serde::Deserialize;

const GITHUB_ADVISORY_API: &str =
    "https://api.github.com/advisories";

#[derive(Debug, Deserialize)]
struct GhAdvisory {
    ghsa_id: String,
    summary: String,
    severity: String,
    #[serde(default)]
    vulnerabilities: Vec<GhVulnerability>,
}

#[derive(Debug, Deserialize)]
struct GhVulnerability {
    package: GhPackage,
}

#[derive(Debug, Deserialize)]
struct GhPackage {
    name: String,
    ecosystem: String,
}

/// A normalised advisory from GitHub.
#[derive(Debug)]
pub struct Advisory {
    pub id: String,
    pub summary: String,
    pub severity: String,
}

/// Query GitHub Advisory DB for advisories affecting `package` in `ecosystem`.
/// Returns an empty vec when none are found.
/// Returns `Err` only on network/parse failure (caller should fail-open).
pub async fn query(package: &str, ecosystem: &str) -> Result<Vec<Advisory>> {
    let client = reqwest::Client::builder()
        .user_agent("motionstream/0.1")
        .build()?;

    let url = format!(
        "{}?affects={}&ecosystem={}",
        GITHUB_ADVISORY_API,
        urlencoding(package),
        urlencoding(ecosystem)
    );

    let advisories: Vec<GhAdvisory> = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let results = advisories
        .into_iter()
        .filter(|a| {
            a.vulnerabilities.iter().any(|v| {
                v.package.name.eq_ignore_ascii_case(package)
                    && v.package.ecosystem.eq_ignore_ascii_case(ecosystem)
            })
        })
        .map(|a| Advisory {
            id: a.ghsa_id,
            summary: a.summary,
            severity: a.severity.to_uppercase(),
        })
        .collect();

    Ok(results)
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}
