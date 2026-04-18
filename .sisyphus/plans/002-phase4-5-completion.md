# Phase 4-5 Completion Plan

> **Goal:** Make the TUI interactive (save, add/edit/remove providers, model discovery) and add CLI subcommands.

**Architecture:** The app layer (crates/app) already has all CRUD operations. The TUI just needs to wire key events to app actions. CLI mode uses the same app layer via clap subcommands.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28, clap 4, tokio (async for discovery)

---

## Priority Order

1. **P0: Save + Dirty State** — Without save, all edits are ephemeral
2. **P0: CLI Subcommands** — Minimum viable scripting use
3. **P1: Provider Add/Edit/Remove** — Core interaction loop
4. **P1: Model Discovery** — Fetch from models.dev
5. **P2: Import/Export** — Nice to have
6. **P2: Polish** — Error display, args wiring, clippy

---

## Task 1: Wire Save & Dirty State to TUI

**Files:**
- Modify: `crates/tui/src/tui_app.rs` — Add save keybinding (Ctrl+S), dirty indicator in status bar
- Modify: `crates/tui/src/ui.rs` — Show dirty state in provider list status bar
- Modify: `crates/tui/src/event.rs` — Add Save, LoadRefresh events

**What:**
- Wire Ctrl+S to call `state.save(state.edit_layer)` via AppEvent::Save
- Show "[unsaved]" indicator in status bar when `state.dirty || app.dirty`
- Show "[saved]" briefly after successful save
- Handle save errors by displaying them in the UI

**Key changes to tui_app.rs:**
```rust
KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    app.on_event(AppEvent::Save);
}
```

**Key changes to ui.rs status bar:**
```rust
let dirty = if state.dirty { " [UNSAVED]" } else { "" };
```

---

## Task 2: CLI Subcommands (Phase 5)

**Files:**
- Modify: `crates/tui/src/main.rs` — Add clap subcommands

**What:**
- `opm` — Launch TUI (default, current behavior)
- `opm list-providers [--layer merged|global|project]` — Output JSON to stdout
- `opm show-config [--layer merged|global|project]` — Output config JSON to stdout
- `opm validate` — Validate config files, output errors to stderr

**Implementation:**
```rust
#[derive(Parser)]
enum Command {
    /// Launch TUI (default)
    Tui {
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        layer: Option<String>,
        #[arg(long)]
        split: bool,
    },
    /// List providers
    ListProviders {
        #[arg(long, default_value = "merged")]
        layer: String,
    },
    /// Show config
    ShowConfig {
        #[arg(long, default_value = "merged")]
        layer: String,
    },
    /// Validate config files
    Validate,
}
```

---

## Task 3: Provider Add/Edit/Remove in TUI

**Files:**
- Modify: `crates/tui/src/tui_app.rs` — Add form state (input fields, cursor position)
- Modify: `crates/tui/src/ui.rs` — Render form views
- Modify: `crates/tui/src/event.rs` — Add text input handling

**What:**
- `a` key in ProviderList → Enter AddProvider mode (form wizard)
- `e` key on selected provider → Enter EditProvider mode
- `d` key on selected provider → Remove with confirmation
- Form has fields: provider_id, name, base_url (for options)
- Enter submits, Esc cancels
- On submit, call `state.add_provider()` or `state.edit_provider_field()`

**Key design:**
- Add `InputMode` enum to App: `Normal`, `Editing(field_name)`, `Adding`
- Text input: accumulate chars, backspace deletes, Enter submits
- Form fields rendered as labeled input areas
- Validation feedback inline

---

## Task 4: Model Discovery Integration

**Files:**
- Modify: `crates/tui/src/main.rs` — Pass discovery client to app state
- Modify: `crates/tui/src/tui_app.rs` — Add model list state
- Modify: `crates/tui/src/ui.rs` — Render model list with selection

**What:**
- `m` key on selected provider → Fetch models from cache/API
- Display list of available models (from discovery crate)
- Toggle models with Space key
- Selected models auto-added to provider config

**Async challenge:** models.dev fetch is async, TUI is sync render loop. Solution: spawn tokio task, store result in App state, render spinner while loading.

---

## Task 5: Wire CLI Args to TUI State

**Files:**
- Modify: `crates/tui/src/main.rs` — Use parsed args

**What:**
- `--config PATH` → pass to `ConfigPaths::from_custom()` or similar
- `--layer LAYER` → set initial `state.edit_layer`
- `--split` → set initial `app.mode = AppMode::SplitView`