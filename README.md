# primer

Pre-install security interceptor for package managers. Scans packages against the [OSV vulnerability database](https://osv.dev/) before they hit your system — with an optional local AI summary, git hook integration, and CI mode.

## How it works

primer places lightweight shims ahead of your package managers in `$PATH`. When you run `pip install requests`, the shim intercepts the command, queries OSV, and either passes through silently (clean result) or prompts you before executing.

```
pip install pillow
  → primer shim intercepts
  → queries OSV
  → found 3 vulnerabilities (1 CRITICAL, 2 HIGH)
  → [prompt] View full details? (y/N)
  → [prompt] Continue install anyway? (y/N)
  → exits 1 on "N"
```

No vulnerability found — the install runs with no output and no delay (after the first cached query).

## Supported ecosystems

| Ecosystem | Intercepted commands |
|-----------|---------------------|
| Python    | `pip`, `uv`, `poetry` |
| Node.js   | `npm`, `yarn`, `pnpm` |
| Go        | `go get`, `go mod` |
| Rust      | `cargo add` |

## Installation

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/callezenwaka/primer/releases/latest/download/primer-installer.sh | sh
```

Then run once to set up shims:

```sh
primer init
```

This creates shims in `~/.primer/bin` and prepends it to your shell config (`.zshenv` for zsh, `.bashrc` for bash, fish function for fish). Restart your shell or `source` the config file.

**From source:**

```sh
cargo install --git https://github.com/callezenwaka/primer
primer init
```

## Commands

### Scanning

```sh
# Scan any package manually
primer scan requests --ecosystem pypi
primer scan express --ecosystem npm
primer scan github.com/gin-gonic/gin --ecosystem go
primer scan serde --ecosystem cargo

# Pin a version
primer scan pillow --ecosystem pypi --version 9.0.0

# Skip prompts (proceed regardless)
primer scan pillow --ecosystem pypi --force

# Show cache hit/miss
primer scan requests --ecosystem pypi --verbose

# Include AI-generated summary (requires update-models)
primer scan pillow --ecosystem pypi --ai
```

### Setup and teardown

```sh
primer init           # create shims, update PATH
primer uninit         # remove shims, strip PATH entry
primer uninit --purge # also delete cache and model files
primer doctor         # check PATH order, shim health, cache state, model state
```

### Allow-list

Add a package to `.primer-ignore` in the project root to skip the scan without `--force`:

```sh
primer allow pillow
primer allow pillow --ecosystem pypi   # scope to one ecosystem
```

### Cache

```sh
primer cache clear    # remove all cached OSV results
```

Results are cached in `~/.primer/cache/` with a 24-hour TTL. On network failure the most recent cached result is used (stale-on-error fallback).

### AI summary

```sh
# Download the default model (~80 MB, no account required)
primer update-models

# Use a local GGUF file
primer update-models --from /path/to/model.gguf --tokenizer /path/to/tokenizer.json

# Download a different model from HuggingFace Hub
primer update-models --repo <hf-repo> --file <filename>
```

Once a model is present, pass `--ai` to any `scan` command or shim invocation to get a plain-English CVE summary before the decision prompt.

Set `PRIMER_AI=0` to disable AI entirely (useful in CI pipelines).

### Git hook

Block commits that add vulnerable packages to manifests:

```sh
# Install the pre-commit hook in the current repo
primer hook install

# Run the check manually without committing
primer hook check
```

Monitored manifests: `requirements.txt`, `package.json`, `go.mod`, `Cargo.toml`.

## CI / non-interactive mode

When `CI=true` is set (standard on GitHub Actions, CircleCI, etc.) or stdin is not a TTY, primer switches to non-interactive mode automatically:

- No prompts
- Blocks on Critical and High findings (exit code `1`)
- Writes all findings to `primer-report.json` in the working directory

Override with `PRIMER_CI_MODE=allow-all` to disable blocking (audit-only pipelines).

## Diagnostics

```sh
primer doctor
```

Reports:
- Whether `~/.primer/bin` is correctly ordered ahead of version managers (`nvm`, `pyenv`, `asdf`, `volta`) in `$PATH`
- Resolved path of each shim and its real binary
- Cache entry count and total size
- Active AI model path and file size

## Uninstall

```sh
primer uninit --purge
```

Removes shims, strips the `$PATH` entry from your shell config, and deletes `~/.primer/` (cache, models, config).

## License

MIT
