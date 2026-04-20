# OpenCode Provider Manager

A cross-platform TUI/GUI tool for managing [OpenCode](https://opencode.ai)'s `opencode.json` provider configuration files, with model discovery and config merge visualization.

## Features

- **Multi-layer config management** — View and edit global, project, and custom config layers with proper merge precedence
- **Config merge visualization** — See the merged result of global + project configs, or inspect them side by side
- **Authentication status** — Check which providers have API keys configured (via `auth.json` or environment variables)
- **Model discovery** — Browse available models from [models.dev](https://models.dev) and provider APIs (OpenAI-compatible, Ollama, LM Studio) with file caching
- **Config import/export** — Import and export `opencode.json` configs with merge or replace modes
- **Validation** — Validate config files against OpenCode's schema (model ID format, provider ID rules, disabled/enabled conflicts)
- **Cross-platform** — Windows, macOS, Linux, ARM64

## Architecture

The project is organized as a Cargo workspace with 6 crates:

| Crate | Description |
|-------|-------------|
| `config-core` | Schema types, JSONC parsing, deep merge, validation, path resolution |
| `discovery` | models.dev client, provider API discovery (OpenAI/Ollama/LM Studio), file cache |
| `auth` | auth.json parser, key format detection, auth status |
| `app` | Application state, provider/model actions, import/export |
| `tui` | Terminal UI (ratatui + crossterm) |
| `gui` | Graphical UI (egui) — feature-gated, not yet functional |

## Installation

### npx (recommended)

No installation needed — run directly with npx:

```bash
npx opencode-provider-manager
```

The binary is automatically downloaded for your platform (Windows/macOS/Linux, x64/ARM64).

Global install:

```bash
npm install -g opencode-provider-manager
opm
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/ayaka209/opencode-provider-manager/releases).

### From source

```bash
git clone https://github.com/ayaka209/opencode-provider-manager.git
cd opencode-provider-manager
cargo build --release
```

The binary will be at `target/release/opm`.

## Usage

### TUI Mode (default)

```bash
opm                  # Launch TUI
opm --split          # Start in split view (global vs project)
opm --config PATH   # Use a custom config file path
opm --layer LAYER    # Start with a specific config layer (global/project/custom)
```

#### Key Bindings

| Key | Action |
|-----|--------|
| `1` | Merged config view |
| `2` | Split view (global vs project) |
| `p` | Provider list |
| `n` | Add new provider |
| `Enter` | Edit selected provider |
| `d` | Delete provider (with confirmation) |
| `s` | Save config |
| `r` | Refresh from disk |
| `a` | Auth status |
| `m` | Model discovery (from models.dev) |
| `c` | Config detail |
| `?` | Help |
| `j`/`↓` | Move down |
| `k`/`↑` | Move up |
| `q`/`Esc` | Quit / Cancel |

### CLI Mode

```bash
opm list-providers [--layer merged|global|project|custom]   # List providers as JSON
opm show-config [--layer merged|global|project|custom]      # Show config as JSON
opm validate                                                 # Validate configs, exit 0/1
```

### GUI Mode

```bash
cargo run --bin opm-gui --features gui
```

> Note: The GUI is a placeholder and not yet functional. It will be developed as a feature-gated alternative to the TUI.

## Configuration

OpenCode uses a layered config system:

1. **Global config**: `~/.config/opencode/opencode.json` (or `$XDG_CONFIG_HOME`, or `~/.opencode.json`)
2. **Project config**: `./opencode.json` (traversing up to git root)
3. **Custom config**: `$OPENCODE_CONFIG` env var
4. **Managed config**: Platform-specific managed config paths

Provider Manager merges these according to OpenCode's documented precedence:

> Project config overrides global config. For objects, deep merge; for arrays, project replaces global.

### Auth

Provider authentication is read from `~/.local/share/opencode/auth.json` (or platform equivalent):

```json
{
  "openai": { "type": "api", "key": "sk-..." },
  "anthropic": { "type": "api", "key": "sk-ant-..." }
}
```

Environment variable references are also detected: `{env:OPENAI_API_KEY}`.

> **⚠️ Do not put API keys in `opencode.json`.** After adding a provider, run `/connect <provider-id>` in OpenCode to securely save your API key to `auth.json`.

## Development

```bash
# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Run TUI locally
cargo run --bin opm
```

## License

MIT