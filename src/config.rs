use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".primer")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub ai: AiConfig,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct AiConfig {
    /// Inference backend: "local" (candle GGUF) or "ollama".
    pub backend: Option<String>,
    /// Local GGUF path (backend = "local") or Ollama model name (backend = "ollama").
    pub model: Option<PathBuf>,
    /// Absolute path to tokenizer.json (local backend only).
    pub tokenizer: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Read / write
// ---------------------------------------------------------------------------

/// Load config from `~/.primer/config.toml`.
/// Returns a default (empty) config if the file doesn't exist.
pub fn load() -> Result<Config> {
    load_from(&config_path())
}

/// Write config to `~/.primer/config.toml`.
pub fn save(cfg: &Config) -> Result<()> {
    save_to(&config_path(), cfg)
}

pub(crate) fn load_from(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub(crate) fn save_to(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(cfg)?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Key-based get / set / list  (for `primer config` subcommands)
// ---------------------------------------------------------------------------

/// Supported dot-separated config keys.
const VALID_KEYS: &[&str] = &["ai.backend", "ai.model", "ai.tokenizer"];

pub fn get(key: &str) -> Result<Option<String>> {
    get_from(&config_path(), key)
}

pub(crate) fn get_from(path: &Path, key: &str) -> Result<Option<String>> {
    let cfg = load_from(path)?;
    let value = match key {
        "ai.backend" => cfg.ai.backend.clone(),
        "ai.model" => cfg.ai.model.map(|p| p.to_string_lossy().into_owned()),
        "ai.tokenizer" => cfg.ai.tokenizer.map(|p| p.to_string_lossy().into_owned()),
        _ => bail!(
            "unknown config key '{}'. Valid keys: {}",
            key,
            VALID_KEYS.join(", ")
        ),
    };
    Ok(value)
}

pub fn set(key: &str, value: &str) -> Result<()> {
    set_to(&config_path(), key, value)
}

pub(crate) fn set_to(path: &Path, key: &str, value: &str) -> Result<()> {
    let mut cfg = load_from(path)?;
    match key {
        "ai.backend" => {
            if value != "local" && value != "ollama" {
                bail!("ai.backend must be 'local' or 'ollama'");
            }
            cfg.ai.backend = Some(value.to_string());
        }
        "ai.model" => cfg.ai.model = Some(PathBuf::from(value)),
        "ai.tokenizer" => cfg.ai.tokenizer = Some(PathBuf::from(value)),
        _ => bail!(
            "unknown config key '{}'. Valid keys: {}",
            key,
            VALID_KEYS.join(", ")
        ),
    }
    save_to(path, &cfg)?;
    println!("  ✓ {} = {}", key, value);
    Ok(())
}

pub fn list() -> Result<()> {
    let cfg = load()?;
    let path = config_path();
    println!("Config: {}\n", path.display());
    println!(
        "  ai.backend   = {}",
        cfg.ai.backend.as_deref().unwrap_or("(not set)")
    );
    println!(
        "  ai.model     = {}",
        cfg.ai
            .model
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "(not set)".into())
    );
    println!(
        "  ai.tokenizer = {}",
        cfg.ai
            .tokenizer
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "(not set)".into())
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_model() {
        let cfg = Config::default();
        assert!(cfg.ai.model.is_none());
        assert!(cfg.ai.tokenizer.is_none());
        assert!(cfg.ai.backend.is_none());
    }

    #[test]
    fn roundtrip_with_model_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.ai.backend = Some("local".into());
        cfg.ai.model = Some(PathBuf::from("/home/user/.primer/models/smollm2.gguf"));
        cfg.ai.tokenizer = Some(PathBuf::from("/home/user/.primer/models/tokenizer.json"));

        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path).unwrap();

        assert_eq!(loaded, cfg);
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let cfg = load_from(&path).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn partial_config_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[ai]\n").unwrap();
        let cfg = load_from(&path).unwrap();
        assert!(cfg.ai.model.is_none());
    }

    #[test]
    fn get_unknown_key_errors() {
        // get() calls load() which reads ~/.primer/config.toml; we just check error path
        assert!(get("unknown.key").is_err());
    }

    // --- set_to / get_from ---

    #[test]
    fn set_backend_local_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        set_to(&path, "ai.backend", "local").unwrap();
        let v = get_from(&path, "ai.backend").unwrap();
        assert_eq!(v.as_deref(), Some("local"));
    }

    #[test]
    fn set_backend_ollama_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        set_to(&path, "ai.backend", "ollama").unwrap();
        let v = get_from(&path, "ai.backend").unwrap();
        assert_eq!(v.as_deref(), Some("ollama"));
    }

    #[test]
    fn set_invalid_backend_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(set_to(&path, "ai.backend", "openai").is_err());
    }

    #[test]
    fn set_model_path_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        set_to(&path, "ai.model", "/tmp/model.gguf").unwrap();
        let v = get_from(&path, "ai.model").unwrap();
        assert_eq!(v.as_deref(), Some("/tmp/model.gguf"));
    }

    #[test]
    fn get_unset_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let v = get_from(&path, "ai.backend").unwrap();
        assert!(v.is_none());
    }

    #[test]
    fn set_unknown_key_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(set_to(&path, "ai.unknown", "value").is_err());
    }
}
