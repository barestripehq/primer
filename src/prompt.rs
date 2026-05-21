use colored::Colorize;
use inquire::Confirm;

use crate::engine::osv::Vulnerability;

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum Decision {
    Proceed,
    Abort,
}

// ---------------------------------------------------------------------------
// Environment helpers
// ---------------------------------------------------------------------------

/// True when stdin is not a TTY or CI=true is set.
pub fn is_ci() -> bool {
    use std::io::IsTerminal;
    std::env::var("CI").is_ok() || !std::io::stdin().is_terminal()
}

pub fn ci_allow_all() -> bool {
    std::env::var("PRIMER_CI_MODE")
        .map(|v| v.to_lowercase() == "allow-all")
        .unwrap_or(false)
}

pub fn force_flag() -> bool {
    std::env::var("PRIMER_FORCE").is_ok()
}

// ---------------------------------------------------------------------------
// Severity helpers
// ---------------------------------------------------------------------------

fn is_blocking(label: &str) -> bool {
    matches!(label, "CRITICAL" | "HIGH")
}

fn color_severity(label: &str) -> colored::ColoredString {
    match label {
        "CRITICAL" => label.red().bold(),
        "HIGH" => label.yellow().bold(),
        "MEDIUM" => label.blue().bold(),
        "LOW" => label.green().bold(),
        _ => label.white().dimmed(),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Evaluate findings and return whether the install should proceed.
/// Handles force, CI, and interactive modes.
pub fn evaluate(package: &str, ecosystem: &str, vulns: &[Vulnerability], force: bool) -> Decision {
    evaluate_inner(
        package,
        ecosystem,
        vulns,
        force || force_flag(),
        is_ci(),
        ci_allow_all(),
    )
}

/// Testable inner function with explicit flags instead of env var reads.
pub(crate) fn evaluate_inner(
    package: &str,
    ecosystem: &str,
    vulns: &[Vulnerability],
    force: bool,
    ci: bool,
    allow_all: bool,
) -> Decision {
    if vulns.is_empty() {
        return Decision::Proceed;
    }

    let blocking: Vec<&Vulnerability> = vulns
        .iter()
        .filter(|v| is_blocking(v.severity_label()))
        .collect();

    if force {
        eprintln!(
            "{} {} {} {} — proceeding (--force)",
            "⚠".yellow(),
            blocking.len(),
            if blocking.len() == 1 {
                "blocking vulnerability"
            } else {
                "blocking vulnerabilities"
            },
            format!("in {}", package).bold(),
        );
        return Decision::Proceed;
    }

    if ci {
        return ci_decision_inner(package, ecosystem, vulns, &blocking, allow_all);
    }

    interactive_decision(package, vulns, &blocking)
}

// ---------------------------------------------------------------------------
// CI mode
// ---------------------------------------------------------------------------

fn ci_decision_inner(
    package: &str,
    ecosystem: &str,
    vulns: &[Vulnerability],
    blocking: &[&Vulnerability],
    allow_all: bool,
) -> Decision {
    if allow_all {
        eprintln!(
            "primer: PRIMER_CI_MODE=allow-all — scan skipped for {}",
            package
        );
        return Decision::Proceed;
    }

    // Print findings to stderr for CI logs.
    print_findings(package, vulns);

    if !blocking.is_empty() {
        // Write JSON report then block.
        if let Err(e) = crate::report::write(package, ecosystem, vulns) {
            eprintln!("primer: could not write report: {}", e);
        }
        eprintln!(
            "{} Blocking install of {} ({} CRITICAL/HIGH {}). Report: primer-report.json",
            "✗".red().bold(),
            package.bold(),
            blocking.len(),
            if blocking.len() == 1 {
                "finding"
            } else {
                "findings"
            },
        );
        return Decision::Abort;
    }

    Decision::Proceed
}

// ---------------------------------------------------------------------------
// Interactive mode
// ---------------------------------------------------------------------------

fn interactive_decision(
    package: &str,
    vulns: &[Vulnerability],
    blocking: &[&Vulnerability],
) -> Decision {
    // Header.
    eprintln!();
    eprintln!(
        "{} {} {} found for {}",
        "⚠".yellow().bold(),
        vulns.len(),
        if vulns.len() == 1 {
            "vulnerability"
        } else {
            "vulnerabilities"
        },
        package.bold(),
    );
    eprintln!();

    // Show top-level CVE list.
    for v in vulns.iter().take(5) {
        eprintln!("  [{}] {}", color_severity(v.severity_label()), v.id);
        if let Some(s) = &v.summary {
            eprintln!("       {}", s.dimmed());
        }
    }
    if vulns.len() > 5 {
        eprintln!("  … and {} more", vulns.len() - 5);
    }
    eprintln!();

    // Prompt 1: offer full details.
    let show_details = Confirm::new("View full vulnerability details?")
        .with_default(false)
        .prompt()
        .unwrap_or(false);

    if show_details {
        eprintln!();
        print_findings(package, vulns);
    }

    if blocking.is_empty() {
        // No blocking findings — no need to prompt further.
        return Decision::Proceed;
    }

    eprintln!();
    eprintln!(
        "  {} {} CRITICAL/HIGH {} detected.",
        "!".red().bold(),
        blocking.len(),
        if blocking.len() == 1 {
            "vulnerability"
        } else {
            "vulnerabilities"
        },
    );
    eprintln!();

    // Prompt 2: continue or abort.
    let proceed = Confirm::new("Continue install anyway?")
        .with_default(false)
        .prompt()
        .unwrap_or(false);

    eprintln!();

    if proceed {
        Decision::Proceed
    } else {
        eprintln!(
            "  Aborted. To bypass: {} {} install {}",
            "PRIMER_FORCE=1".dimmed(),
            package,
            package,
        );
        Decision::Abort
    }
}

// ---------------------------------------------------------------------------
// Shared display
// ---------------------------------------------------------------------------

fn print_findings(package: &str, vulns: &[Vulnerability]) {
    eprintln!("  Security findings for {}:\n", package.bold());
    for v in vulns {
        eprintln!("  [{}] {}", color_severity(v.severity_label()), v.id.bold());
        if let Some(s) = &v.summary {
            eprintln!("       {}", s);
        }
        if let Some(cv) = &v.cvss_vector {
            eprintln!("       CVSS: {}", cv.dimmed());
        }
        eprintln!();
    }
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
            summary: Some(format!("Test vuln {}", id)),
            cvss_vector: None,
            severity: Some(severity.to_owned()),
        }
    }

    #[test]
    fn empty_vulns_always_proceeds() {
        assert_eq!(evaluate("pkg", "PyPI", &[], false), Decision::Proceed);
    }

    #[test]
    fn force_true_proceeds_despite_critical() {
        let vulns = vec![vuln("GHSA-0001", "CRITICAL")];
        assert_eq!(evaluate("pkg", "PyPI", &vulns, true), Decision::Proceed);
    }

    #[test]
    fn ci_allow_all_proceeds() {
        let vulns = vec![vuln("GHSA-0001", "CRITICAL")];
        assert_eq!(
            evaluate_inner("pkg", "PyPI", &vulns, false, true, true),
            Decision::Proceed
        );
    }

    #[test]
    fn ci_blocks_on_critical() {
        let vulns = vec![vuln("GHSA-0001", "CRITICAL")];
        assert_eq!(
            evaluate_inner("pkg", "PyPI", &vulns, false, true, false),
            Decision::Abort
        );
    }

    #[test]
    fn ci_proceeds_on_low_only() {
        let vulns = vec![vuln("GHSA-0001", "LOW")];
        assert_eq!(
            evaluate_inner("pkg", "PyPI", &vulns, false, true, false),
            Decision::Proceed
        );
    }

    #[test]
    fn ci_proceeds_on_medium_only() {
        let vulns = vec![vuln("GHSA-0001", "MEDIUM")];
        assert_eq!(
            evaluate_inner("pkg", "PyPI", &vulns, false, true, false),
            Decision::Proceed
        );
    }

    #[test]
    fn is_blocking_for_critical_and_high() {
        assert!(is_blocking("CRITICAL"));
        assert!(is_blocking("HIGH"));
        assert!(!is_blocking("MEDIUM"));
        assert!(!is_blocking("LOW"));
        assert!(!is_blocking("UNKNOWN"));
    }
}
