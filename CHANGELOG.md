# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-04-25

### Added

- Standalone GUI binary crate published as `opencode-provider-manager-gui`.
- Rich import support for JSON/JSONC, TOML, YAML, models.dev directories, and GitHub tree URLs with import provenance metadata.
- Release automation for npm and crates.io, including two-crate crates publishing order.

### Changed

- Public crates.io packaging is now limited to `opencode-provider-manager` and `opencode-provider-manager-gui`.
- Shared app/config/auth/discovery logic is embedded in the public crate so internal workspace crates stay unpublished.

## [0.1.1] - 2026-04-19

### Added

- **Edit Provider view** — Press Enter on a provider to view details and edit name, SDK package, and base URL fields in-place.
- **npm/SDK package field** — Add Provider wizard now includes a 4th field for the SDK package (e.g. `@anthropic-ai/sdk`, `openai`).
- **Model discovery from models.dev** — Press `m` to fetch available models for the selected provider, showing context length and pricing. Press Enter to add a model to the provider config.
- **`--config` CLI flag** — Wire the `--config` flag to actually load a custom config file via the `custom` config layer. All CLI subcommands (`list-providers`, `show-config`, `validate`) now respect it.
- **`custom` config layer** — Full support for a third config layer (`OPENCODE_CONFIG` / `--config`) with correct merge precedence: `global < custom < project`.
- **Dirty check before refresh** — Pressing `r` with unsaved changes now shows a confirmation dialog instead of silently discarding.
- **Confirm refresh dialog** — New `ConfirmRefresh` overlay with y/n prompt.
- **`n:New` hint** — Provider list status bar and help overlay now show the `n` key binding for adding a new provider.

### Fixed

- **JSONC comment preservation on save** — `write_config` now reconciles the new value against the existing CST node-by-node, preserving comments and formatting around unchanged keys instead of rewriting the entire file as plain JSON.
- **Edit errors no longer swallowed** — EditProvider now surfaces the first error from `edit_provider_field` in the error bar, staying in the edit view on failure.
- **`r` key in Model Selector** — Refreshes the model list from models.dev instead of triggering a config reload.

## [0.1.0] - 2026-04-18

### Added

- Multi-layer config management (global, project) with proper merge precedence.
- Config merge visualization — merged view and split-pane (global vs project).
- Provider CRUD — add, delete (with confirmation), save, refresh.
- Authentication status view — check API key configuration per provider.
- Model discovery module — `models.dev` client, OpenAI/Ollama/LM Studio provider API discovery, file cache.
- CLI subcommands — `opm list-providers`, `opm show-config`, `opm validate`.
- TUI with 7 view modes, key-driven navigation, dirty indicator, error bar.
- JSONC parsing support (read) with `jsonc-parser`.
- Config validation — model ID format, provider ID rules, disabled/enabled conflict detection.
- Config import/export with merge or replace modes.
- Cross-platform CI — test + clippy + fmt on ubuntu/windows/macOS.
- Release workflow — 6 binary builds (linux/macos/windows x x64/arm64) on tag push.

[0.1.1]: https://github.com/ayaka209/opencode-provider-manager/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ayaka209/opencode-provider-manager/releases/tag/v0.1.0
