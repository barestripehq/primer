use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::shim::PackageManager;

pub fn run() -> Result<()> {
    let ms_bin = motionstream_bin_dir();
    let self_path = env::current_exe().context("could not determine motionstream binary path")?;

    fs::create_dir_all(&ms_bin)
        .with_context(|| format!("could not create {}", ms_bin.display()))?;

    println!("Initialising motionstream...\n");

    // Create one shim per PM that is installed on this system.
    let mut created = 0;
    for pm in PackageManager::all() {
        if let Some(real) = crate::shim::find_real_binary(pm.name()) {
            let shim_path = ms_bin.join(pm.name());
            create_shim(&self_path, &shim_path, pm.name(), &real)?;
            created += 1;
        }
    }

    if created == 0 {
        println!("No supported package managers found — nothing to shim.");
        return Ok(());
    }

    println!("\nUpdating shell configs...\n");
    update_shell_configs(&ms_bin)?;

    println!(
        "Done. Restart your shell or run:\n\n  source ~/.zshenv   # zsh\n  source ~/.bashrc   # bash\n"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Shim creation
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn create_shim(self_path: &Path, shim_path: &Path, name: &str, real: &Path) -> Result<()> {
    // Remove stale shim if it exists.
    if shim_path.exists() || shim_path.symlink_metadata().is_ok() {
        fs::remove_file(shim_path)?;
    }
    std::os::unix::fs::symlink(self_path, shim_path)
        .with_context(|| format!("could not create shim for {}", name))?;
    println!("  ✓ {} → {} (real: {})", shim_path.display(), self_path.display(), real.display());
    Ok(())
}

#[cfg(not(unix))]
fn create_shim(self_path: &Path, shim_path: &Path, name: &str, real: &Path) -> Result<()> {
    // On Windows, copy the binary instead of symlinking.
    let shim_path = shim_path.with_extension("exe");
    fs::copy(self_path, &shim_path)
        .with_context(|| format!("could not create shim for {}", name))?;
    println!("  ✓ {} (real: {})", shim_path.display(), real.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Shell config update
// ---------------------------------------------------------------------------

const PATH_LINE: &str = r#"export PATH="$HOME/.motionstream/bin:$PATH""#;
const MARKER: &str = "# motionstream";

fn update_shell_configs(ms_bin: &Path) -> Result<()> {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let home = PathBuf::from(&home);

    // Files to update, in priority order.
    let candidates: &[(&str, &str)] = &[
        (".zshenv", "zsh (all shells)"),
        (".zshrc",  "zsh (interactive)"),
        (".bashrc", "bash"),
        (".bash_profile", "bash login"),
        (".config/fish/config.fish", "fish"),
    ];

    for (file, label) in candidates {
        let path = home.join(file);
        if !path.exists() {
            continue;
        }
        match append_path_line(&path, ms_bin) {
            Ok(true)  => println!("  ✓ Updated {} ({})", path.display(), label),
            Ok(false) => println!("  · Already configured in {} ({})", path.display(), label),
            Err(e)    => println!("  ✗ Could not update {}: {}", path.display(), e),
        }
    }

    Ok(())
}

/// Append the PATH export to `config_file` if not already present.
/// Returns true if the file was modified, false if already configured.
pub(crate) fn append_path_line(config_file: &Path, _ms_bin: &Path) -> Result<bool> {
    let contents = fs::read_to_string(config_file)?;
    if contents.contains(MARKER) {
        return Ok(false);
    }

    let addition = if config_file.extension().map(|e| e == "fish").unwrap_or(false) {
        // fish uses set -gx instead of export
        format!("\n{MARKER}\nfish_add_path \"$HOME/.motionstream/bin\"\n")
    } else {
        format!("\n{MARKER}\n{PATH_LINE}\n")
    };

    fs::write(config_file, format!("{contents}{addition}"))?;
    Ok(true)
}

pub fn motionstream_bin_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".motionstream").join("bin")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_file(content: &str) -> (tempfile::NamedTempFile, PathBuf) {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn appends_path_line_to_empty_file() {
        let (_f, path) = temp_file("");
        let result = append_path_line(&path, &PathBuf::new()).unwrap();
        assert!(result, "should return true (file modified)");
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains(MARKER));
        assert!(contents.contains(PATH_LINE));
    }

    #[test]
    fn append_is_idempotent() {
        let (_f, path) = temp_file("# existing content\n");
        append_path_line(&path, &PathBuf::new()).unwrap();
        let result = append_path_line(&path, &PathBuf::new()).unwrap();
        assert!(!result, "second call should return false (already configured)");
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches(MARKER).count(), 1, "marker should appear exactly once");
    }

    #[test]
    fn appends_fish_syntax_for_fish_files() {
        let (_f, path) = temp_file("# fish config\n");
        // Rename to .fish so the extension check triggers.
        let fish_path = path.with_extension("fish");
        fs::copy(&path, &fish_path).unwrap();
        append_path_line(&fish_path, &PathBuf::new()).unwrap();
        let contents = fs::read_to_string(&fish_path).unwrap();
        assert!(contents.contains("fish_add_path"));
        assert!(!contents.contains("export PATH"));
        let _ = fs::remove_file(&fish_path);
    }

    #[test]
    fn preserves_existing_content() {
        let original = "# my zshrc\nexport FOO=bar\n";
        let (_f, path) = temp_file(original);
        append_path_line(&path, &PathBuf::new()).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with(original));
    }
}
