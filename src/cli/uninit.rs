use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::shim::PackageManager;

const MARKER: &str = "# motionstream";

pub fn run(purge: bool) -> Result<()> {
    println!("Removing motionstream shims...\n");

    let ms_bin = motionstream_bin_dir();

    // Remove individual shim symlinks.
    let mut removed = 0;
    for pm in PackageManager::all() {
        let shim = ms_bin.join(pm.name());
        if shim.exists() || shim.symlink_metadata().is_ok() {
            fs::remove_file(&shim)?;
            println!("  ✓ Removed {}", shim.display());
            removed += 1;
        }
    }

    if removed == 0 {
        println!("  · No shims found.");
    }

    // Remove PATH entries from shell configs.
    println!("\nCleaning shell configs...\n");
    remove_path_lines()?;

    // Optionally purge cache and models.
    if purge {
        let ms_home = ms_bin.parent().unwrap().to_path_buf();
        for subdir in &["cache", "models"] {
            let dir = ms_home.join(subdir);
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
                println!("  ✓ Purged {}", dir.display());
            }
        }
    }

    println!("\nDone. Restart your shell to complete removal.");
    Ok(())
}

fn remove_path_lines() -> Result<()> {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let home = PathBuf::from(&home);

    let candidates = [
        ".zshenv", ".zshrc", ".bashrc", ".bash_profile",
        ".config/fish/config.fish",
    ];

    for file in &candidates {
        let path = home.join(file);
        if !path.exists() {
            continue;
        }
        match strip_marker_block(&path) {
            Ok(true)  => println!("  ✓ Cleaned {}", path.display()),
            Ok(false) => {}
            Err(e)    => println!("  ✗ Could not clean {}: {}", path.display(), e),
        }
    }

    Ok(())
}

/// Remove the `# motionstream` block from a config file.
/// Returns true if the file was modified.
fn strip_marker_block(path: &std::path::Path) -> Result<bool> {
    let contents = fs::read_to_string(path)?;
    if !contents.contains(MARKER) {
        return Ok(false);
    }

    // Drop lines from the marker through the next blank line.
    let mut filtered = Vec::new();
    let mut skip = false;
    for line in contents.lines() {
        if line.trim() == MARKER {
            skip = true;
            continue;
        }
        if skip && line.trim().is_empty() {
            skip = false;
            continue;
        }
        if !skip {
            filtered.push(line);
        }
    }

    fs::write(path, filtered.join("\n") + "\n")?;
    Ok(true)
}

fn motionstream_bin_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".motionstream").join("bin")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn temp_file(content: &str) -> (tempfile::NamedTempFile, PathBuf) {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn strips_marker_block_from_config() {
        let content = "# existing\nexport FOO=bar\n\n# motionstream\nexport PATH=\"$HOME/.motionstream/bin:$PATH\"\n\n# after\n";
        let (_f, path) = temp_file(content);
        let result = strip_marker_block(&path).unwrap();
        assert!(result, "should return true (file modified)");
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains(MARKER));
        assert!(!contents.contains(".motionstream"));
        assert!(contents.contains("export FOO=bar"));
        assert!(contents.contains("# after"));
    }

    #[test]
    fn strip_is_noop_when_marker_absent() {
        let content = "# my config\nexport BAR=baz\n";
        let (_f, path) = temp_file(content);
        let result = strip_marker_block(&path).unwrap();
        assert!(!result, "should return false (nothing to remove)");
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn strip_and_re_append_roundtrips_cleanly() {
        use crate::cli::init::append_path_line;
        let original = "# my zshrc\nexport FOO=bar\n";
        let (_f, path) = temp_file(original);

        append_path_line(&path, &PathBuf::new()).unwrap();
        strip_marker_block(&path).unwrap();

        let after = fs::read_to_string(&path).unwrap();
        assert!(!after.contains(MARKER), "marker should be gone after strip");
        assert!(after.contains("export FOO=bar"), "original content preserved");
    }
}
