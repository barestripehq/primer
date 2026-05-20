mod allowlist;
mod cache;
mod cli;
mod engine;
mod prompt;
mod report;
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
        /// Proceed regardless of findings
        #[arg(long)]
        force: bool,
        /// Print cache hit/miss and fetch source
        #[arg(long)]
        verbose: bool,
    },
    /// Manage the local vulnerability cache
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Add a package to the project allow-list (.motionstream-ignore)
    Allow {
        /// Package name to allow
        package: String,
        /// Ecosystem to scope the allow (optional)
        #[arg(long, value_enum)]
        ecosystem: Option<Ecosystem>,
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

#[derive(Subcommand)]
enum CacheCommands {
    /// Remove all cached vulnerability results
    Clear,
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
        Commands::Scan { package, ecosystem, version, force, verbose } => {
            let eco = ecosystem.as_osv_str();
            println!("Scanning {} ({}) ...", package, eco);

            match osv::query(&package, eco, version.as_deref(), verbose).await {
                Ok(vulns) if vulns.is_empty() => {
                    println!("✓ No vulnerabilities found.");
                }
                Ok(vulns) => {
                    match prompt::evaluate(&package, eco, &vulns, force) {
                        prompt::Decision::Abort => std::process::exit(1),
                        prompt::Decision::Proceed => {}
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Scan skipped: {} (proceeding)", e);
                }
            }
        }

        Commands::Allow { package, ecosystem } => {
            let eco = ecosystem.as_ref().map(|e| e.as_osv_str());
            allowlist::add(&package, eco)?;
        }

        Commands::Cache { command: CacheCommands::Clear } => {
            match cache::clear() {
                Ok(0) => println!("Cache is already empty."),
                Ok(n) => println!("Cleared {} cached entr{}.", n, if n == 1 { "y" } else { "ies" }),
                Err(e) => eprintln!("Failed to clear cache: {}", e),
            }
        }

        Commands::Init => cli::init::run()?,
        Commands::Uninit { purge } => cli::uninit::run(purge)?,
        Commands::Doctor => cli::doctor::run()?,
    }

    Ok(())
}
