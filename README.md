# primer

Pre-install security interceptor for package managers. Scans packages against the [OSV vulnerability database](https://osv.dev/) before they hit your system — with an optional local AI summary, git hook integration, and CI mode.

## How it works

primer places lightweight shims ahead of your package managers in `$PATH`. When you run `pip install requests`, the shim intercepts the command, queries OSV, and either passes through silently (clean result) or prompts you before executing.

```
pip install pillow
  → primer shim intercepts
  → queries OSV
  → found 3 vulnerabilities (1 CRITICAL, 2 HIGH)
    pillow 9.0.0 — GHSA-56pw-mpj4-fxjw [CRITICAL]
      Summary: Heap buffer overflow in TIFF image parser
      Fixed in: 9.0.1
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

# Include AI-generated summary (requires primer model add)
primer scan pillow --ecosystem pypi --ai

# Scan all packages declared in a manifest file (no install)
primer scan --file requirements.txt
primer scan --file package.json
primer scan --file go.mod
primer scan --file Cargo.toml
```

Each finding now shows the patched version when OSV provides one:

```
pillow 9.0.0 — GHSA-56pw-mpj4-fxjw [CRITICAL]
  Summary: Heap buffer overflow in TIFF image parser
  Fixed in: 9.0.1
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

### AI model

```sh
# Download the default model (~80 MB, no account required)
primer model add

# Import a local GGUF file
primer model add --from /path/to/model.gguf --tokenizer /path/to/tokenizer.json

# Download a specific model from HuggingFace Hub
primer model add --repo <hf-repo> --file <filename>

# List registered models (* = active)
primer model list

# Set the active inference target
primer model set ~/.primer/models/smollm2.gguf   # local candle inference
primer model set ollama:llama3.2                 # route to local Ollama instance

# Remove models
primer model remove                              # interactive select
primer model remove smollm2.gguf ollama:llama3.2 # remove by name (no prompt)
primer model remove --all                        # remove all, clear config
```

Once a model is present, pass `--ai` to any `scan` command to get a plain-English CVE summary before the decision prompt.

Set `PRIMER_AI=0` to disable AI entirely (useful in CI pipelines).

### Git hook

Block commits that add vulnerable packages to manifests:

```sh
# Install the pre-commit hook in the current repo
primer hook install

# Run the check manually without committing
primer hook check
```

Monitored manifests: `requirements.txt`, `pyproject.toml`, `package.json`, `go.mod`, `Cargo.toml`.

### Intercept bare restore commands

By default, bare restore commands (`npm install` with no packages, `go mod download`, etc.) pass straight through. Enable interception to scan the manifest before dependencies are installed:

```sh
primer config set intercept-restore true
```

When enabled, primer scans the relevant manifest for each PM's "install all" form before passing through:

| Command | Manifest scanned |
|---------|-----------------|
| `npm install` / `pnpm install` / `yarn` | `package.json` |
| `pip install` (no packages) / `uv sync` | `requirements.txt` → `pyproject.toml` |
| `poetry install` | `pyproject.toml` |
| `go mod download` | `go.mod` |
| `cargo build` / `cargo fetch` / `cargo check` | `Cargo.toml` |

Disabled by default — large projects can have many deps. Cache makes repeat scans instant.

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
- `intercept-restore` config status with enable hint
- Active AI model path and file size

## Uninstall

```sh
primer uninit --purge
```

Removes shims, strips the `$PATH` entry from your shell config, and deletes `~/.primer/` (cache, models, config).

## License

MIT
