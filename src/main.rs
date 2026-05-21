mod allowlist;
mod cache;
mod cli;
mod config;
mod engine;
mod manifest;
mod prompt;
mod report;
mod shim;
mod summary;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use engine::osv;

#[derive(Parser)]
#[command(
    name = "primer",
    about = "Pre-install security interceptor for package managers"
)]
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
        /// Generate an AI summary of findings using the local model
        #[arg(long)]
        ai: bool,
    },
    /// Manage the local vulnerability cache
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Download or register the AI summary model
    UpdateModels {
        /// Use a local GGUF file instead of downloading (skips network)
        #[arg(long, value_name = "PATH")]
        from: Option<std::path::PathBuf>,
        /// Tokenizer to pair with --from
        #[arg(long, value_name = "PATH")]
        tokenizer: Option<std::path::PathBuf>,
        /// HuggingFace repo to download from (use with --file)
        #[arg(long, value_name = "REPO")]
        repo: Option<String>,
        /// Filename within the HF repo
        #[arg(long, value_name = "FILE")]
        file: Option<String>,
    },
    /// Add a package to the project allow-list (.primer-ignore)
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
    /// Manage git hook integration
    Hook {
        #[command(subcommand)]
        command: HookCommands,
    },
}

#[derive(Subcommand)]
enum HookCommands {
    /// Write .git/hooks/pre-commit to block vulnerable package additions
    Install,
    /// Diff staged manifest changes and scan newly added packages (also called by the hook itself)
    Check,
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
        Commands::Scan {
            package,
            ecosystem,
            version,
            force,
            verbose,
            ai,
        } => {
            let eco = ecosystem.as_osv_str();
            println!("Scanning {} ({}) ...", package, eco);

            match osv::query(&package, eco, version.as_deref(), verbose).await {
                Ok(vulns) if vulns.is_empty() => {
                    println!("✓ No vulnerabilities found.");
                }
                Ok(vulns) => {
                    // Show AI summary when requested, before the decision prompt.
                    if ai {
                        show_ai_summary(&vulns);
                    }
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

        Commands::UpdateModels {
            from,
            tokenizer,
            repo,
            file,
        } => {
            #[cfg(feature = "ai")]
            {
                let opts = summary::download::DownloadOptions {
                    from,
                    tokenizer,
                    repo,
                    file,
                };
                summary::download::run(opts).await?;
            }
            #[cfg(not(feature = "ai"))]
            {
                let _ = (from, tokenizer, repo, file);
                eprintln!(
                    "AI features are not compiled in.\n\
                     Rebuild with:  cargo install primer --features ai"
                );
            }
        }

        Commands::Allow { package, ecosystem } => {
            let eco = ecosystem.as_ref().map(|e| e.as_osv_str());
            allowlist::add(&package, eco)?;
        }

        Commands::Cache {
            command: CacheCommands::Clear,
        } => match cache::clear() {
            Ok(0) => println!("Cache is already empty."),
            Ok(n) => println!(
                "Cleared {} cached entr{}.",
                n,
                if n == 1 { "y" } else { "ies" }
            ),
            Err(e) => eprintln!("Failed to clear cache: {}", e),
        },

        Commands::Init => cli::init::run()?,
        Commands::Uninit { purge } => cli::uninit::run(purge)?,
        Commands::Doctor => cli::doctor::run()?,
        Commands::Hook {
            command: HookCommands::Install,
        } => cli::hook::install()?,
        Commands::Hook {
            command: HookCommands::Check,
        } => cli::hook::check().await?,
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// AI summary display
// ---------------------------------------------------------------------------

fn show_ai_summary(vulns: &[osv::Vulnerability]) {
    if std::env::var("PRIMER_AI")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        return;
    }

    #[cfg(not(feature = "ai"))]
    {
        let _ = vulns;
        eprintln!("  ℹ  --ai requires the AI feature: cargo install primer --features ai");
    }

    #[cfg(feature = "ai")]
    {
        if !summary::model_present() {
            eprintln!("  ℹ  No model found. Run: primer update-models");
            return;
        }

        eprint!("  Generating summary … ");
        match summary::generate(vulns) {
            Some(s) => {
                eprintln!();
                eprintln!();
                eprintln!("  Summary");
                eprintln!("  ───────");
                eprintln!("  {}", s.text);
                eprintln!();
            }
            None => eprintln!("(unavailable)"),
        }
    }
}
