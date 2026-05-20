use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::engine::osv;

// ---------------------------------------------------------------------------
// Package manager detection
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum PackageManager {
    Pip,
    Uv,
    Poetry,
    Npm,
    Yarn,
    Pnpm,
    Go,
    Cargo,
}

impl PackageManager {
    /// Detect which PM we are masquerading as from argv[0].
    pub fn from_argv0(argv0: &str) -> Option<Self> {
        let name = Path::new(argv0)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or(argv0);

        match name {
            "pip" | "pip3" => Some(Self::Pip),
            "uv" => Some(Self::Uv),
            "poetry" => Some(Self::Poetry),
            "npm" => Some(Self::Npm),
            "yarn" => Some(Self::Yarn),
            "pnpm" => Some(Self::Pnpm),
            "go" => Some(Self::Go),
            "cargo" => Some(Self::Cargo),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Pip => "pip",
            Self::Uv => "uv",
            Self::Poetry => "poetry",
            Self::Npm => "npm",
            Self::Yarn => "yarn",
            Self::Pnpm => "pnpm",
            Self::Go => "go",
            Self::Cargo => "cargo",
        }
    }

    pub fn ecosystem(&self) -> &'static str {
        match self {
            Self::Pip | Self::Uv | Self::Poetry => "PyPI",
            Self::Npm | Self::Yarn | Self::Pnpm => "npm",
            Self::Go => "Go",
            Self::Cargo => "crates.io",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Pip, Self::Uv, Self::Poetry,
            Self::Npm, Self::Yarn, Self::Pnpm,
            Self::Go, Self::Cargo,
        ]
    }
}

// ---------------------------------------------------------------------------
// Package argument extraction
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct PackageArg {
    pub name: String,
    pub version: Option<String>,
}

/// Returns the packages to scan if the args are an install command,
/// or None if the command should pass through immediately (e.g. `pip list`).
pub fn extract_packages(pm: &PackageManager, args: &[String]) -> Option<Vec<PackageArg>> {
    let subcommand = args.first().map(String::as_str)?;

    let is_install = match pm {
        PackageManager::Pip | PackageManager::Uv => subcommand == "install",
        PackageManager::Poetry => subcommand == "add",
        PackageManager::Npm => matches!(subcommand, "install" | "i" | "add"),
        PackageManager::Yarn => subcommand == "add",
        PackageManager::Pnpm => matches!(subcommand, "add" | "install"),
        PackageManager::Go => subcommand == "get",
        PackageManager::Cargo => subcommand == "add",
    };

    if !is_install {
        return None;
    }

    let packages: Vec<PackageArg> = args[1..]
        .iter()
        .filter(|a| !a.starts_with('-'))
        // Skip manifest files passed via -r / --requirements
        .filter(|a| !a.ends_with(".txt") && !a.ends_with(".toml") && !a.ends_with(".json"))
        .map(|spec| parse_package_spec(pm, spec))
        .collect();

    if packages.is_empty() { None } else { Some(packages) }
}

fn parse_package_spec(pm: &PackageManager, spec: &str) -> PackageArg {
    match pm {
        PackageManager::Npm
        | PackageManager::Yarn
        | PackageManager::Pnpm
        | PackageManager::Go
        | PackageManager::Cargo => {
            // spec formats: lodash@4.17.21, golang.org/x/net@v0.1.0, serde@1.0
            if let Some((name, ver)) = spec.split_once('@') {
                PackageArg { name: name.to_owned(), version: Some(ver.to_owned()) }
            } else {
                PackageArg { name: spec.to_owned(), version: None }
            }
        }
        PackageManager::Pip | PackageManager::Uv | PackageManager::Poetry => {
            // spec formats: requests, requests==2.28.0, requests>=2.0, requests[security]
            let name_end = spec
                .find(|c: char| c == '=' || c == '>' || c == '<' || c == '!' || c == '[')
                .unwrap_or(spec.len());
            let name = spec[..name_end].trim().to_owned();
            let version = spec
                .find("==")
                .map(|i| spec[i + 2..].split_whitespace().next().unwrap_or("").to_owned());
            PackageArg { name, version }
        }
    }
}

// ---------------------------------------------------------------------------
// Real binary lookup
// ---------------------------------------------------------------------------

fn motionstream_bin_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".motionstream").join("bin")
}

/// Find the real PM binary, excluding ~/.motionstream/bin to avoid loops.
pub fn find_real_binary(name: &str) -> Option<PathBuf> {
    let shim_dir = motionstream_bin_dir();
    let path_var = env::var_os("PATH")?;

    for dir in env::split_paths(&path_var) {
        if dir == shim_dir {
            continue;
        }
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Exec real binary (replaces current process on Unix)
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn exec_real_binary(binary: &Path, args: &[String]) -> anyhow::Error {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(binary).args(args).exec();
    anyhow::Error::from(err)
}

#[cfg(not(unix))]
pub fn exec_real_binary(binary: &Path, args: &[String]) -> anyhow::Error {
    // On Windows, spawn and wait instead of exec.
    match std::process::Command::new(binary).args(args).status() {
        Ok(status) => {
            std::process::exit(status.code().unwrap_or(1));
        }
        Err(e) => anyhow::Error::from(e),
    }
}

// ---------------------------------------------------------------------------
// Shim entry point
// ---------------------------------------------------------------------------

pub async fn run(pm: PackageManager, args: Vec<String>) -> Result<()> {
    let real_bin = find_real_binary(pm.name())
        .ok_or_else(|| anyhow::anyhow!("Could not find real '{}' binary in PATH", pm.name()))?;

    if let Some(packages) = extract_packages(&pm, &args) {
        let force = crate::prompt::force_flag();

        for pkg in &packages {
            // Skip allow-listed packages silently.
            if crate::allowlist::is_allowed(&pkg.name, pm.ecosystem()) {
                continue;
            }

            match osv::query(&pkg.name, pm.ecosystem(), pkg.version.as_deref()).await {
                Ok(vulns) if !vulns.is_empty() => {
                    match crate::prompt::evaluate(&pkg.name, pm.ecosystem(), &vulns, force) {
                        crate::prompt::Decision::Abort => std::process::exit(1),
                        crate::prompt::Decision::Proceed => {}
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("⚠  motionstream: scan skipped ({}) — proceeding", e),
            }
        }
    }

    // Hand off to the real binary — this replaces the current process on Unix.
    bail!(exec_real_binary(&real_bin, &args))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- from_argv0 ---

    #[test]
    fn detects_pip_from_full_path() {
        assert_eq!(PackageManager::from_argv0("/usr/bin/pip"), Some(PackageManager::Pip));
    }

    #[test]
    fn detects_pip3() {
        assert_eq!(PackageManager::from_argv0("pip3"), Some(PackageManager::Pip));
    }

    #[test]
    fn detects_npm() {
        assert_eq!(PackageManager::from_argv0("npm"), Some(PackageManager::Npm));
    }

    #[test]
    fn detects_cargo() {
        assert_eq!(PackageManager::from_argv0("/Users/user/.cargo/bin/cargo"), Some(PackageManager::Cargo));
    }

    #[test]
    fn returns_none_for_motionstream() {
        assert!(PackageManager::from_argv0("motionstream").is_none());
    }

    #[test]
    fn returns_none_for_unknown() {
        assert!(PackageManager::from_argv0("ruby").is_none());
    }

    // --- extract_packages ---

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn pip_install_single_package() {
        let pkgs = extract_packages(&PackageManager::Pip, &args("install requests")).unwrap();
        assert_eq!(pkgs[0].name, "requests");
        assert_eq!(pkgs[0].version, None);
    }

    #[test]
    fn pip_install_with_version() {
        let pkgs = extract_packages(&PackageManager::Pip, &args("install requests==2.28.0")).unwrap();
        assert_eq!(pkgs[0].name, "requests");
        assert_eq!(pkgs[0].version, Some("2.28.0".into()));
    }

    #[test]
    fn pip_list_passes_through() {
        assert!(extract_packages(&PackageManager::Pip, &args("list")).is_none());
    }

    #[test]
    fn pip_install_flags_only_passes_through() {
        assert!(extract_packages(&PackageManager::Pip, &args("install -r requirements.txt")).is_none());
    }

    #[test]
    fn npm_install_with_at_version() {
        let pkgs = extract_packages(&PackageManager::Npm, &args("install lodash@4.17.21")).unwrap();
        assert_eq!(pkgs[0].name, "lodash");
        assert_eq!(pkgs[0].version, Some("4.17.21".into()));
    }

    #[test]
    fn npm_alias_i() {
        let pkgs = extract_packages(&PackageManager::Npm, &args("i express")).unwrap();
        assert_eq!(pkgs[0].name, "express");
    }

    #[test]
    fn npm_run_passes_through() {
        assert!(extract_packages(&PackageManager::Npm, &args("run build")).is_none());
    }

    #[test]
    fn cargo_add_with_version() {
        let pkgs = extract_packages(&PackageManager::Cargo, &args("add serde@1.0")).unwrap();
        assert_eq!(pkgs[0].name, "serde");
        assert_eq!(pkgs[0].version, Some("1.0".into()));
    }

    #[test]
    fn go_get_module() {
        let pkgs = extract_packages(&PackageManager::Go, &args("get golang.org/x/net")).unwrap();
        assert_eq!(pkgs[0].name, "golang.org/x/net");
    }

    #[test]
    fn go_mod_download_passes_through() {
        assert!(extract_packages(&PackageManager::Go, &args("mod download")).is_none());
    }

    #[test]
    fn pip_install_multiple_packages() {
        let pkgs = extract_packages(&PackageManager::Pip, &args("install requests flask")).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "requests");
        assert_eq!(pkgs[1].name, "flask");
    }
}
