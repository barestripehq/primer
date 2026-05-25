use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::shim::PackageManager;

const VERSION_MANAGERS: &[&str] = &["nvm", "pyenv", "asdf", "volta", "fnm", "rtx", "mise"];

pub fn run() -> Result<()> {
    let ms_bin = primer_bin_dir();

    println!("primer doctor\n");

    check_path_order(&ms_bin);
    println!();
    check_shims(&ms_bin);
    println!();
    check_cache(&ms_bin);
    println!();
    check_config();
    println!();
    check_model(&ms_bin);

    Ok(())
}

// ---------------------------------------------------------------------------
// PATH order
// ---------------------------------------------------------------------------

fn check_path_order(ms_bin: &Path) {
    println!("PATH order");
    println!("----------");

    let path_var = env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = env::split_paths(&path_var).collect();

    let ms_pos = dirs.iter().position(|d| d == ms_bin);
    let vm_positions: Vec<(&str, usize)> = VERSION_MANAGERS
        .iter()
        .filter_map(|vm| {
            dirs.iter()
                .position(|d| d.to_str().map(|s| s.contains(vm)).unwrap_or(false))
                .map(|pos| (*vm, pos))
        })
        .collect();

    match ms_pos {
        None => println!("  ✗ ~/.primer/bin not found in PATH — run `primer init`"),
        Some(pos) => {
            println!("  ✓ ~/.primer/bin at position {}", pos);
            for (vm, vm_pos) in &vm_positions {
                if *vm_pos < pos {
                    println!(
                        "  ✗ {} is at position {} (before primer) — shims may be bypassed",
                        vm, vm_pos
                    );
                } else {
                    println!("  ✓ {} is at position {} (after primer)", vm, vm_pos);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shim status
// ---------------------------------------------------------------------------

fn check_shims(ms_bin: &Path) {
    println!("Shims");
    println!("-----");

    for pm in PackageManager::all() {
        let shim = ms_bin.join(pm.name());
        let real = crate::shim::find_real_binary(pm.name());

        match (shim.exists(), real) {
            (true, Some(real_path)) => {
                println!(
                    "  ✓ {}  →  {} (real: {})",
                    pm.name(),
                    shim.display(),
                    real_path.display()
                );
            }
            (true, None) => {
                println!(
                    "  ✗ {} shim exists but real binary not found in PATH",
                    pm.name()
                );
            }
            (false, Some(_)) => {
                println!(
                    "  · {} installed but not shimmed — run `primer init`",
                    pm.name()
                );
            }
            (false, None) => {
                println!("  · {} not installed", pm.name());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

fn check_cache(ms_bin: &Path) {
    println!("Cache");
    println!("-----");

    let cache_dir = ms_bin.parent().unwrap().join("cache");

    if !cache_dir.exists() {
        println!("  · Cache directory not yet created");
        return;
    }

    let (count, total_bytes) = walk_dir_stats(&cache_dir);
    println!(
        "  · {} entries, {:.1} KB  ({})",
        count,
        total_bytes as f64 / 1024.0,
        cache_dir.display()
    );
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

fn check_config() {
    println!("Config");
    println!("------");

    let cfg = crate::config::load().unwrap_or_default();

    if cfg.intercept_restore {
        println!("  ✓ intercept-restore = true");
        println!("    bare restore commands (npm install, pip install, …) will be scanned");
    } else {
        println!("  · intercept-restore = false");
        println!("    run `primer config set intercept-restore true` to enable manifest scanning");
    }

    let threshold = cfg.prompt_threshold.as_deref().unwrap_or("high");
    println!("  · prompt-threshold  = {}", threshold);
    if threshold == "high" {
        println!("    CRITICAL and HIGH findings block installs (default)");
    } else {
        println!(
            "    run `primer config set prompt-threshold high` to restore default blocking"
        );
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

fn check_model(_ms_bin: &Path) {
    println!("AI model");
    println!("--------");

    let (model_path, tokenizer_path) = crate::summary::active_paths();

    // Model file
    if model_path.exists() {
        let size = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);
        println!(
            "  ✓ model     {} ({:.1} MB)",
            model_path.display(),
            size as f64 / (1024.0 * 1024.0)
        );
    } else {
        println!("  ✗ model     not found — run `primer model add`");
        println!("             expected: {}", model_path.display());
    }

    // Tokenizer file
    if tokenizer_path.exists() {
        let size = std::fs::metadata(&tokenizer_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!(
            "  ✓ tokenizer {} ({:.1} KB)",
            tokenizer_path.display(),
            size as f64 / 1024.0
        );
    } else {
        println!("  ✗ tokenizer not found — run `primer model add`");
    }

    #[cfg(not(feature = "ai"))]
    println!("  ℹ  AI inference not compiled in — rebuild with: cargo build --features ai");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn walk_dir_stats(dir: &Path) -> (usize, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    entries
        .filter_map(|e| e.ok())
        .fold((0, 0), |(count, bytes), entry| {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            (count + 1, bytes + size)
        })
}

fn primer_bin_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".primer").join("bin")
}
