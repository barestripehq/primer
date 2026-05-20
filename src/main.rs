mod cli;
mod engine;
mod shim;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use engine::osv;

#[derive(Parser)]
#[command(name = "motionstream", about = "Pre-install security interceptor for package managers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a package for known vulnerabilities
    Scan {
        /// Package name to scan
        package: String,
        /// Package ecosystem
        #[arg(long, value_enum, default_value = "pypi")]
        ecosystem: Ecosystem,
        /// Specific version to check (defaults to latest)
        #[arg(long)]
        version: Option<String>,
    },
    /// Generate shims and update PATH
    Init,
    /// Remove shims and PATH entries
    Uninit {
        /// Also delete cache and model files
        #[arg(long)]
        purge: bool,
    },
    /// Check PATH order, shim health, cache, and model state
    Doctor,
}

#[derive(Clone, ValueEnum)]
enum Ecosystem {
    #[value(name = "pypi")]
    PyPI,
    #[value(name = "npm")]
    Npm,
    #[value(name = "go")]
    Go,
    #[value(name = "cargo")]
    Cargo,
}

impl Ecosystem {
    fn as_osv_str(&self) -> &'static str {
        match self {
            Ecosystem::PyPI => "PyPI",
            Ecosystem::Npm => "npm",
            Ecosystem::Go => "Go",
            Ecosystem::Cargo => "crates.io",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Check if we are being invoked as a PM shim (e.g. argv[0] == "pip").
    let argv0 = std::env::args().next().unwrap_or_default();
    if let Some(pm) = shim::PackageManager::from_argv0(&argv0) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        return shim::run(pm, args).await;
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { package, ecosystem, version } => {
            let eco = ecosystem.as_osv_str();
            println!("Scanning {} ({}) ...", package, eco);

            match osv::query(&package, eco, version.as_deref()).await {
                Ok(vulns) if vulns.is_empty() => {
                    println!("✓ No vulnerabilities found.");
                }
                Ok(vulns) => {
                    println!("Found {} vulnerability/vulnerabilities:\n", vulns.len());
                    for v in &vulns {
                        println!("  [{}] {}", v.severity_label(), v.id);
                        if let Some(summary) = &v.summary {
                            println!("      {}", summary);
                        }
                        if let Some(vector) = &v.cvss_vector {
                            println!("      CVSS: {}", vector);
                        }
                        println!();
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Scan skipped: {} (proceeding with install)", e);
                }
            }
        }

        Commands::Init => cli::init::run()?,
        Commands::Uninit { purge } => cli::uninit::run(purge)?,
        Commands::Doctor => cli::doctor::run()?,
    }

    Ok(())
}
