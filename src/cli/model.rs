use std::path::PathBuf;

use anyhow::Result;

// ---------------------------------------------------------------------------
// model add
// ---------------------------------------------------------------------------

pub async fn add(
    from: Option<PathBuf>,
    tokenizer: Option<PathBuf>,
    repo: Option<String>,
    file: Option<String>,
) -> Result<()> {
    #[cfg(feature = "ai")]
    {
        let dl = crate::summary::download::DownloadOptions { from, tokenizer, repo, file };
        crate::summary::download::run(dl).await?;
    }
    #[cfg(not(feature = "ai"))]
    {
        let _ = (from, tokenizer, repo, file);
        eprintln!(
            "AI features are not compiled in.\n\
             Rebuild with:  cargo install primer --features ai"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// model list
// ---------------------------------------------------------------------------

pub fn list() -> Result<()> {
    let models_dir = crate::summary::models_dir();

    if !models_dir.exists() {
        println!("No models directory found. Run: primer model add");
        return Ok(());
    }

    let cfg = crate::config::load().unwrap_or_default();
    let active_model = cfg.ai.model.as_deref().and_then(|p| p.to_str()).unwrap_or("");
    let backend = cfg.ai.backend.as_deref().unwrap_or("local");

    println!("Models: {}\n", models_dir.display());

    let mut found = false;
    let mut entries: Vec<_> = std::fs::read_dir(&models_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "gguf" || ext == "json")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let is_active = path.to_str().map(|s| s == active_model).unwrap_or(false);
        let marker = if is_active { "* " } else { "  " };
        println!(
            "{}{:<50} {:.1} MB",
            marker,
            path.file_name().unwrap_or_default().to_string_lossy(),
            size as f64 / (1024.0 * 1024.0)
        );
        found = true;
    }

    if !found {
        println!("  (no model files found)");
    }

    if backend == "ollama" {
        println!(
            "\n  * active backend: ollama ({})",
            cfg.ai.model.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "(none)".into())
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// model set
// ---------------------------------------------------------------------------

pub fn set(target: &str) -> Result<()> {
    let mut cfg = crate::config::load().unwrap_or_default();

    if let Some(model_name) = target.strip_prefix("ollama:") {
        // Ollama backend
        cfg.ai.backend = Some("ollama".to_string());
        cfg.ai.model = Some(PathBuf::from(model_name));
        crate::config::save(&cfg)?;
        println!("  ✓ backend = ollama");
        println!("  ✓ model   = {}", model_name);
        println!();
        println!("  Inference will be routed to http://localhost:11434");
        println!("  Make sure Ollama is running and '{}' is pulled.", model_name);
    } else {
        // Local GGUF path
        let path = PathBuf::from(target);
        if !path.exists() {
            anyhow::bail!(
                "model file not found: {}\n  Use `primer model add --from <path>` to import it first.",
                path.display()
            );
        }
        cfg.ai.backend = Some("local".to_string());
        cfg.ai.model = Some(path.clone());
        crate::config::save(&cfg)?;
        println!("  ✓ backend = local");
        println!("  ✓ model   = {}", path.display());
    }

    Ok(())
}
