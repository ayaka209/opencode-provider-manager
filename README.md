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

Published package names:

- crates.io: `opencode-provider-manager`
- crates.io: `opencode-provider-manager-gui`
- npm: `opencode-provider-manager`

The binary will be at `target/release/opm`.

## Usage

### TUI Mode (default)

```bash
opm                  # Launch TUI
opm --split          # Start in split view (global vs project)
opm --config PATH   # Use a custom config file path
opm --layer LAYER    # Start with a specific edit layer (default: project)
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
opm list-providers [--layer merged|global|project|custom]   # List providers as JSON (default: merged)
opm show-config [--layer merged|global|project|custom]      # Show config as JSON (default: merged)
opm validate                                                 # Validate configs, exit 0/1
opm import --input PATH_OR_URL_OR_SNIPPET --layer project    # Import JSON/TOML/YAML into a layer
```

When you **add a provider**, **edit a provider**, or **add a model** in the TUI,
the default edit target is the **project** layer unless you explicitly switch
it with `--layer`.

### GUI Mode

```bash
cargo run --bin opm-gui
```

The GUI is built as a separate `opm-gui` binary so the TUI binary does not link
egui/eframe. It provides a merged provider overview plus layer-aware import
controls for global, project, and custom configs.

### Importing provider/model snippets

Imports can target `global`, `project`, or `custom` layers and can merge into the
existing layer or replace it:

```bash
# Import a full opencode.json / JSONC config into the project layer
opm import --input ./opencode-provider.json --layer project

# Import a provider fragment that does not include its provider ID
opm import --input ./provider.toml --provider-id xiaomi-token-plan-cn --layer global

# Import a models.dev-style provider directory from GitHub
opm import --input https://github.com/MiyakoMeow/models.dev/tree/dev/providers/xiaomi-token-plan-cn --layer project

# Preview without saving
opm import --input ./providers --layer project --dry-run
```

Supported import shapes:

- Full `opencode.json` / JSONC configs containing `provider`, `model`, `$schema`, etc.
- Provider maps such as `{ "volcengine-plan": { "npm": "...", "models": { ... } } }`.
- Single provider fragments, when paired with `--provider-id` or imported from a named file/directory.
- Local directories containing JSON, JSONC, TOML, YAML, or models.dev-style `provider.toml` plus `models/*.toml`.
- GitHub `tree` URLs for models.dev-style provider directories.

Unknown provider/model fields such as `modalities`, `cost`, `family`, and docs are
preserved during import. Imported configs include a top-level `_opmImport`
metadata field with the source URL/path as provenance; secrets are not displayed
by `show-config`.

### Extending an existing provider vs adding a new provider

There are two common import/update flows:

1. **Extend an existing provider** — use the same provider ID as a provider that
   already exists in the target layer or merged config. The imported data is
   merged into that provider, which is ideal when a known provider adds a new
   model before `models.dev` catches up.
2. **Add a new provider** — use a brand-new provider ID. This creates a new
   provider entry, which is what you want for a new vendor, a new endpoint, or
   your own OpenAI-compatible deployment.

For example, suppose `volcengine-plan` already exists, but Volcano Engine has
released `ark-code-next` and `models.dev` has not listed it yet. You can extend
the existing provider by importing a small overlay into your project layer:

```json
{
  "provider": {
    "volcengine-plan": {
      "models": {
        "ark-code-next": {
          "name": "ark-code-next",
          "limit": {
            "context": 256000,
            "output": 8192
          },
          "modalities": {
            "input": ["text", "image"],
            "output": ["text"]
          }
        }
      }
    }
  }
}
```

```bash
opm import --input ./volcengine-ark-code-next.json --layer project
```

Because the provider ID is still `volcengine-plan`, this **extends** the
existing provider instead of creating a duplicate. In contrast, if you imported
the same model under a new provider ID such as `volcengine-plan-beta`, Provider
Manager would treat that as **adding a new provider**.

Use the project layer for these quick model overlays when you want to keep the
upstream global/built-in provider definition intact and only patch your current
workspace until `models.dev` or your shared config source is updated.

### Importing from GitHub / an unmerged PR

Provider Manager does not need a merge to happen first. If a provider/model
change lives in a GitHub branch or PR, you can still import it in two practical
ways:

1. **Import a raw file URL** — useful when the PR changes a single JSON, TOML,
   YAML, or JSONC file.
2. **Check out the PR locally and import the directory** — useful when the PR
   contains a models.dev-style provider directory with `provider.toml` plus
   `models/*.toml`.

For example, if PR `#123` adds a new model file before it is merged, you can
check out the PR branch locally and import from the working tree:

```bash
gh pr checkout 123
opm import --input ./providers/xiaomi-token-plan-cn --layer project
```

Or, if you just want one file from the PR branch, import the raw GitHub URL
directly:

```bash
opm import \
  --input https://raw.githubusercontent.com/OWNER/REPO/BRANCH/providers/xiaomi-token-plan-cn/models/new-model.toml \
  --provider-id xiaomi-token-plan-cn \
  --layer project
```

This works well for reviewing provider updates from a fork or feature branch
before the upstream PR is merged. If you are importing only a single model file,
remember to pass `--provider-id` so Provider Manager knows which existing
provider to extend.

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
