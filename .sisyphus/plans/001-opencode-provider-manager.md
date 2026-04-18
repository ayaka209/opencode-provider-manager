# OpenCode Provider Manager - Implementation Plan

## Overview

A Rust-based configuration manager for OpenCode, focused on managing `opencode.json` (provider/model/agent/MCP config) across user-level and project-level configs, with TUI-first interface and future GUI support.

## Design Decisions (Confirmed)

| Decision | Choice |
|---|---|
| Positioning | OpenCode config file manager (not proxy layer) |
| Target User | Individual developers |
| Scope | Full `opencode.json` management, `auth.json` read-only |
| Merge Semantics | Replicate OpenCode's deep merge (project overrides global) |
| View Mode | Merged view primary, switchable to split-pane (global vs project) |
| Import Sources | models.dev pull + opencode.json import + cross-provider inheritance + config export |
| Model Discovery | models.dev API + Provider API (dual channel) |
| TUI Framework | ratatui + crossterm |
| GUI Framework | egui (feature gate) |
| API Key Handling | Environment variable references `{env:}`, no plaintext key storage |
| Key Validation | Optional real-time validation (user-triggered), multi-provider format compatible |
| Tech Stack | serde + reqwest + tokio + thiserror/anyhow |
| Project Structure | workspace multi-crate: config-core / discovery / auth / tui / gui |
| Min Rust | 1.85+ (2025 edition) |
| Testing | TDD first |
| Caching | File cache at `~/.local/share/opencode-provider-manager/cache/` |
| Distribution | GitHub Releases + package managers + cargo install |
| Platforms | Windows/macOS/Linux/ARM64 |
| JSONC | Full read/write support with comment preservation |
| Edit Mode | Structured form + wizard-style provider addition |
| Config Paths | Fully compatible with OpenCode conventions |

## Architecture

### Crate Structure (Cargo Workspace)

```
opencode-provider-manager/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── config-core/              # Config file read/write/validate/merge
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── schema.rs         # OpenCode JSON schema types (serde)
│   │   │   ├── merge.rs          # Deep merge logic (global + project)
│   │   │   ├── validate.rs       # Schema validation
│   │   │   ├── jsonc.rs          # JSONC parser/preserve-comments
│   │   │   ├── paths.rs          # Platform-aware config path resolution
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── discovery/                # Model discovery from models.dev + provider APIs
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── models_dev.rs     # models.dev API client
│   │   │   ├── provider_api.rs   # Direct provider API queries (OpenAI, Anthropic, etc.)
│   │   │   ├── cache.rs           # File-based caching
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── auth/                     # auth.json read-only parsing + key status
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── parser.rs         # Parse auth.json
│   │   │   ├── status.rs         # Key status (configured/missing/expired)
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── app/                      # Application logic (glue layer)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── state.rs           # Application state management
│   │   │   ├── actions.rs         # User actions → state transitions
│   │   │   ├── import.rs          # Config import/export logic
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── tui/                      # Terminal UI (ratatui + crossterm)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── app.rs             # TUI application state
│   │   │   ├── ui/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── provider_list.rs
│   │   │   │   ├── provider_form.rs
│   │   │   │   ├── model_selector.rs
│   │   │   │   ├── config_editor.rs
│   │   │   │   ├── auth_status.rs
│   │   │   │   ├── import_wizard.rs
│   │   │   │   └── split_pane.rs
│   │   │   └── event.rs          # Key/mouse event handling
│   │   └── Cargo.toml
│   └── gui/                      # GUI (egui, feature-gated)
│       ├── src/
│       │   ├── main.rs
│       │   └── app.rs
│       └── Cargo.toml
├── tests/                        # Integration tests
│   ├── config_merge_test.rs
│   ├── discovery_test.rs
│   └── import_export_test.rs
└── .github/
    └── workflows/
        └── ci.yml                # CI: test + clippy + fmt
```

### Dependency Map

```
tui → app → {config-core, discovery, auth}
gui → app → {config-core, discovery, auth}
config-core → {serde, serde_json, jsonc-parser}
discovery → {reqwest, config-core}
auth → {serde, config-core}
```

## Implementation Phases

### Phase 0: Project Scaffolding & Foundation

**Goal**: Bootable workspace with all crates, CI, and basic types.

1. **Create Cargo workspace** with all 6 crates
2. **Define core types** in `config-core/schema.rs`:
   - `OpenCodeConfig` (full opencode.json schema, serde-derived)
   - `ProviderConfig` (provider entry with models, options, npm, name)
   - `ModelConfig` (model entry with name, options, variants, limit)
   - `AgentConfig`, `McpConfig`, `CommandConfig`, etc.
   - `ConfigLayer` enum: `Global` | `Project` | `Merged`
3. **Define path resolution** in `config-core/paths.rs`:
   - Global config paths (in priority order):
     - `~/.config/opencode/opencode.json` (primary)
     - `$HOME/.opencode.json` (fallback)
     - `$XDG_CONFIG_HOME/opencode/opencode.json` (XDG fallback)
   - Project config: `./opencode.json` (traversing up to git root)
   - Auth: `~/.local/share/opencode/auth.json`
   - Managed config (platform-specific, read-only awareness):
     - macOS: `/Library/Application Support/opencode/`
     - Linux: `/etc/opencode/`
     - Windows: `%ProgramData%\opencode`
   - Respect `OPENCODE_CONFIG`, `OPENCODE_CONFIG_DIR`, `OPENCODE_CONFIG_CONTENT` env vars
4. **JSONC parser** in `config-core/jsonc.rs`:
   - Parse JSONC preserving comments
   - Write back with comments intact (using rowan-based or custom approach)
5. **CI setup**: GitHub Actions (test, clippy, fmt on all 3 platforms)
6. **TDD**: Write tests first for all schema types, merge logic, path resolution

**Files**: ~15 files
**Estimated**: 2-3 days

### Phase 1: Config Core — Read/Write/Validate/Merge

**Goal**: Fully working config-core crate that can read, validate, merge, and write OpenCode config files.

1. **Merge logic** in `config-core/merge.rs`:
   - Deep merge: for objects, recurse; for arrays, replace; for scalars, project overrides global
   - Special handling for `provider` field: merge provider entries, within each provider merge `models` and `options`
   - **Decision**: Replicate OpenCode's **documented** behavior (project overrides global), NOT the known bugs. Test against documented precedence, not buggy current behavior.
   - Known OpenCode merge bugs to be aware of but NOT replicate:
     - #19296: Global config currently supersedes project config (opposite of docs)
     - #21307: `.opencode/` directory config precedence is inverted
     - #11628: `OPENCODE_CONFIG_CONTENT` doesn't have highest precedence
   - Acceptance test: documented behavior is the source of truth
2. **Validation** in `config-core/validate.rs`:
   - JSON schema validation (reference `https://opencode.ai/config.json`)
   - Validate provider IDs against known provider list
   - Validate model IDs format (`provider/model`)
   - Validate API key formats (sk-*, sk-ant-*, etc.)
3. **Read/Write** in `config-core/`:
   - `read_config(layer: ConfigLayer) -> Result<OpenCodeConfig>`
   - `write_config(config: &OpenCodeConfig, layer: ConfigLayer, preserve_comments: bool) -> Result<()>`
   - Handle file creation, permissions, atomic writes
4. **Environment variable substitution**: `{env:VAR}` and `{file:path}` support in config values
5. **TDD**: Extensive tests for each merge case, validation case, edge cases

**Files**: ~10 files
**Estimated**: 3-4 days

### Phase 2: Auth & Discovery Modules

**Goal**: Working auth status display and model discovery from models.dev + provider APIs.

1. **Auth module** in `auth/`:
   - Parse `auth.json` structure: `{ "provider_id": { "type": "api", "key": "sk-..." } }` (note: `type` + `key` fields, NOT `apiKey`)
   - Detect key presence and format (patterns per provider)
   - Expose `ProviderAuthStatus` enum: `Configured` | `Missing` | `InvalidFormat` | `EnvVar(String)`
2. **Discovery module** in `discovery/`:
   - `models_dev.rs`: HTTP client for `https://models.dev/api.json`
     - Parse provider list (ID, name, models with capabilities, pricing, context window)
     - Cache to `~/.local/share/opencode-provider-manager/cache/providers.json`
     - Cache TTL: configurable, default 24h
   - `provider_api.rs`: Direct API queries
     - OpenAI: `GET /v1/models` with auth header
     - Anthropic: not publicly queryable (limited)
     - Ollama: `GET /api/tags`
     - LM Studio: `GET /v1/models`
     - Extensible trait `ModelDiscovery` for adding more providers
   - `cache.rs`: File-based cache with TTL, refresh on demand
3. **TDD**: Mock HTTP responses, test parsing, test cache invalidation

**Files**: ~12 files
**Estimated**: 3-4 days

### Phase 3: Application Logic

**Goal**: Glue layer connecting config-core, auth, and discovery into coherent app state.

1. **State management** in `app/state.rs`:
   - `AppState`: holds configs (global, project, merged), auth status, discovery data
   - State transitions: `Idle` → `EditingProvider` → `ConfirmingSave` → `Saved`
   - Undo/redo for config edits
2. **Actions** in `app/actions.rs`:
   - `add_provider(provider_id: &str)`: Wizard flow - pick from built-in list → enter API key (or env var) → select models
   - `edit_provider(provider_id: &str, field: &str, value: &str)`: Edit specific field
   - `remove_provider(provider_id: &str)`: Remove with confirmation
   - `add_model(provider_id: &str, model_id: &str)`: Add model to provider
   - `remove_model(provider_id: &str, model_id: &str)`: Remove model from provider
   - `import_config(path: &Path)`: Import from another opencode.json
   - `export_config(path: &Path)`: Export current merged config
   - `validate_api_key(provider_id: &str, key: &str)`: Test key validity
   - `refresh_models()`: Refresh model list from cache or API
3. **Import/export** in `app/import.rs`:
   - Import: read external opencode.json → merge/replace providers
   - Export: write current config (merged, or specific layer) to file
   - Cross-provider inheritance: copy model list from one provider to another (with ID remapping)
4. **TDD**: Test all action flows with mock state

**Files**: ~8 files
**Estimated**: 3-4 days

### Phase 4: TUI Implementation

**Goal**: Fully functional TUI with all P0 features.

1. **Main layout** (`tui/app.rs`):
   - Tab-based navigation: Providers | Models | Agents | MCP | Auth | Advanced
   - Status bar showing: config layer (Global/Project/Merged), current file path
   - Help overlay (press ?)
2. **Provider list view** (`tui/ui/provider_list.rs`):
   - List all configured providers with auth status icons
   - Show: provider ID, name, model count, auth status (✅ configured / ❌ missing / 🔑 env var)
   - Actions: Add (wizard), Edit, Remove, Duplicate
3. **Provider form/edit** (`tui/ui/provider_form.rs`):
   - Structured form: provider ID, name, options (baseURL, timeout, etc.), models
   - API key field: dropdown (env var reference / auth.json / custom)
   - Level selector: Save to Global / Save to Project
4. **Model selector** (`tui/ui/model_selector.rs`):
   - Source toggle: models.dev / Provider API
   - List of available models with checkboxes
   - Search/filter by name, capability, pricing
   - Selected models auto-populate `provider.models` in config
5. **Config editor** (`tui/ui/config_editor.rs`):
   - Structured form for all opencode.json sections (agent, mcp, command, etc.)
   - JSONC preview panel (read-only or editable)
   - Level selector for each field
6. **Auth status panel** (`tui/ui/auth_status.rs`):
   - Read-only display of auth.json entries
   - Per-provider: key configured? env var? key format valid?
   - Quick actions: "Set env var" (copies command to clipboard), "Run /connect" (hint)
7. **Import wizard** (`tui/ui/import_wizard.rs`):
   - Step 1: Choose source (file path / URL / paste JSON)
   - Step 2: Preview what will be imported
   - Step 3: Conflicts resolution (keep existing / replace / merge)
   - Step 4: Save to layer (Global / Project)
8. **Split pane view** (`tui/ui/split_pane.rs`):
   - Left: Global config, Right: Project config
   - Highlight overridden fields in project view
   - Toggle between merged and split view

**Files**: ~20 files
**Estimated**: 5-7 days

### Phase 5: Polish & Distribution

**Goal**: Production-ready binary.

1. **CLI mode** (basic, for scripting):
   - `opencode-provider-manager list-providers`
   - `opencode-provider-manager show-config [--layer global|project|merged]`
   - `opencode-provider-manager validate [--layer global|project]`
   - Output JSON for piping
2. **Windows/macOS/Linux builds**: Cross-compile via GitHub Actions
3. **Package manager distribution**: homebrew, scoop, cargo install
4. **Documentation**: README with usage examples, screenshots
5. **Error handling polish**: User-friendly error messages, recovery suggestions

**Files**: ~5 files
**Estimated**: 2-3 days

### Total Estimated Timeline: 18-25 days

## Key Technical Notes

### JSONC Comment Preservation Strategy

Since `opencode.json` supports JSONC (JSON with Comments), we need to preserve comments when writing. Strategy:
1. Parse JSONC into a CST (Concrete Syntax Tree) that preserves comment nodes
2. On read: store original JSONC text + parsed config
3. On edit: apply changes to the CST, preserving all comment nodes in their original positions
4. On write: serialize CST back to JSONC

Libraries to evaluate:
- `jsonc-parser` (Node.js port or Rust native)
- Custom serde deserializer with comment tracking
- `rowan`-based parser for proper CST

### Config Merge Semantics

Following OpenCode's documented precedence:
1. Remote config (`.well-known/opencode`) — lowest priority
2. Global config (`~/.config/opencode/opencode.json`)
3. Custom config (`OPENCODE_CONFIG` env var)
4. Project config (`./opencode.json`)
5. `.opencode` directories
6. Inline config (`OPENCODE_CONFIG_CONTENT` env var)
7. Managed config files — highest priority

Our tool manages layers 2, 3, 4 (and optionally 6). Deep merge for nested objects, replace for arrays and scalars.

### Provider Inheritance

Cross-provider model inheritance works by:
1. Source provider has model list: `{ "gpt-4o": {...}, "gpt-5": {...} }`
2. Target provider (e.g., custom OpenAI-compatible) copies model IDs
3. Model IDs may need ID remapping (e.g., `openai/gpt-4o` → `custom-openai/gpt-4o`)

### API Key Format Patterns

| Provider | Key Pattern | Auth Method |
|---|---|---|
| OpenAI | `sk-...` | API Key header |
| Anthropic | `sk-ant-...` | x-api-key header |
| Google | Service account JSON | ADC |
| AWS Bedrock | Profile/Keys | SigV4 |
| GitHub Copilot | OAuth device flow | Token |
| Ollama | (none) | N/A |
| Azure | `KEY 1`/`KEY 2` | api-key header |

## Success Criteria

- [ ] Can read and display merged config from global + project layers
- [ ] Can add/edit/remove providers via TUI wizard
- [ ] Can discover models from models.dev and provider APIs
- [ ] Can select models and write to opencode.json
- [ ] Can import/export config files
- [ ] Can display auth status (read-only)
- [ ] JSONC comments are preserved on write
- [ ] Config merge matches OpenCode's documented behavior (NOT the known bugs)
- [ ] Cross-platform (Windows/macOS/Linux)
- [ ] All core tests pass

## QA Scenarios (Per-Phase)

### Phase 0 QA
- [ ] `cargo test` passes with 0 failures across all 6 crates
- [ ] `cargo clippy` passes with 0 warnings
- [ ] Can parse a sample opencode.json into `OpenCodeConfig` struct
- [ ] Can parse JSONC with comments (trailing comma, // comments, /* */ comments)
- [ ] Path resolution returns correct paths on all platforms for all layers

### Phase 1 QA
- [ ] Global + project config merge produces correct result for all documented override scenarios:
  - [ ] Non-conflicting keys: both preserved
  - [ ] Conflicting scalar: project value wins
  - [ ] Conflicting object: deep merge (project keys override, global keys preserved)
  - [ ] Conflicting array: project array replaces global array
  - [ ] Provider models: deep merge (project models added, global models preserved)
- [ ] Validation catches: invalid provider IDs, malformed model IDs, invalid JSONC
- [ ] Write preserves JSONC comments on round-trip (read → modify → write → read, comments match)

### Phase 2 QA
- [ ] Can parse auth.json with `{ "type": "api", "key": "..." }` format
- [ ] Auth status correctly shows: Configured, Missing, InvalidFormat, EnvVar
- [ ] models.dev API (`https://models.dev/api.json`) returns parseable provider list
- [ ] File cache is written, read, and TTL-expired correctly
- [ ] Provider API queries return model lists for OpenAI, Ollama, LM Studio (with mocks)

### Phase 3 QA
- [ ] `add_provider` wizard creates correct config entries
- [ ] `remove_provider` removes from correct layer
- [ ] `import_config` merges correctly with conflict resolution
- [ ] `export_config` produces valid, comment-preserved JSONC
- [ ] Undo/redo works for config edits

### Phase 4 QA
- [ ] TUI renders provider list with auth status icons
- [ ] Provider form correctly saves to selected layer (Global/Project)
- [ ] Model selector fetches from models.dev and displays with checkboxes
- [ ] Auth panel shows read-only status for all configured providers
- [ ] Split pane view shows global vs project side by side with overrides highlighted
- [ ] Import wizard walks through all 4 steps and produces correct result

### Phase 5 QA
- [ ] `opencode-provider-manager list-providers` outputs valid JSON
- [ ] `opencode-provider-manager validate` catches malformed configs
- [ ] Binary builds on Windows, macOS, Linux without errors
- [ ] Homebrew / scoop / cargo install all work

## Review Log

### Momus Review (2026-04-18)
- **Verdict**: Plan is well-structured and executable. No blocking issues.
- **Key Fixes Applied**:
  1. Fixed `models.dev` API endpoint: `/api.json` (not `/api/providers.json`)
  2. Fixed `auth.json` structure: `{ "type": "api", "key": "..." }` (not `{ "apiKey": "..." }`)
  3. Added merge semantics decision: replicate documented behavior, not known bugs
  4. Added missing config paths: `$HOME/.opencode.json`, `$XDG_CONFIG_HOME`, managed config paths
  5. Added per-phase QA scenarios
- **Non-blocking observations**:
  - Timeline is aggressive but not unrealistic
  - Schema types are incomplete but acceptable for incremental development
  - JSONC CST strategy is viable (`jsonc-parser` crate v0.26+)
  - Crate dependency direction is clean