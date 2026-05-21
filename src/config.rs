use std::path::{Path, PathBuf};

use anyhow::Result;
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
    /// Absolute path to the active GGUF model file.
    pub model: Option<PathBuf>,
    /// Absolute path to the active tokenizer.json.
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
#[cfg(feature = "ai")]
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

#[cfg(feature = "ai")]
pub(crate) fn save_to(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(cfg)?)?;
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
    }

    #[test]
    #[cfg(feature = "ai")]
    fn roundtrip_with_model_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
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
}
