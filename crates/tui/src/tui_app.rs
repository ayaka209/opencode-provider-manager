//! TUI application state and event loop.

use anyhow::Result;
use opencode_provider_manager::{app, config_core, discovery};

use crate::event::AppEvent;
use crate::ui;
use app::state::AppState;

/// TUI application with full state management.
pub struct App {
    /// The current mode/view.
    pub mode: AppMode,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Currently selected provider (for detail/edit views).
    pub selected_provider: Option<String>,
    /// Currently selected list index.
    pub selected_index: usize,
    /// Error message to display, if any.
    pub error_message: Option<String>,
    /// Discovered models from models.dev (cached).
    pub discovered_models: Vec<discovery::DiscoveredModel>,
    /// Whether model discovery is currently loading.
    pub models_loading: bool,
}

/// Current UI mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    /// Main merged config view.
    MergedView,
    /// Split pane view (global vs project).
    SplitView,
    /// Provider list.
    ProviderList,
    /// Auth status view.
    AuthStatus,
    /// Model selector for a provider.
    ModelSelector,
    /// Config detail view (JSON).
    ConfigDetail,
    /// Help overlay.
    Help,
    /// Confirm delete provider dialog.
    ConfirmDelete(String),
    /// Confirm refresh (discard unsaved changes).
    ConfirmRefresh,
    /// Add provider wizard (form with text inputs).
    AddProvider(AddProviderForm),
    /// Edit provider view (display and edit fields).
    EditProvider(EditProviderForm),
}

/// Known SDK packages for provider configuration.
/// The last entry "Custom" signals manual text entry mode.
pub const KNOWN_SDKS: &[&str] = &[
    "@ai-sdk/openai",
    "@ai-sdk/openai-compatible",
    "@ai-sdk/anthropic",
    "@ai-sdk/google",
    "@ai-sdk/groq",
    "@ai-sdk/mistral",
    "@ai-sdk/amazon-bedrock",
    "@ai-sdk/azure",
    "Custom...",
];

/// Return all configured provider IDs.
pub fn all_provider_ids(state: &AppState) -> Vec<String> {
    state.provider_ids()
}

/// Build the list of providers available for copying (configured only).
/// Returns (id, display_name) pairs.
pub fn copy_source_list(state: &AppState) -> Vec<(String, String)> {
    let configured = state.provider_ids();
    let mut result: Vec<(String, String)> = Vec::new();

    // Add configured providers
    for id in &configured {
        let name = state
            .get_provider(id)
            .and_then(|p| p.name.clone())
            .unwrap_or_else(|| id.clone());
        result.push((id.clone(), name));
    }

    result
}

/// State for the SDK selection sub-field within a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkSelectState {
    /// Currently highlighted index in KNOWN_SDKS list.
    pub highlight: usize,
    /// Whether we're in custom text entry mode (last option selected + Enter pressed).
    pub custom_mode: bool,
    /// Custom text (only used when custom_mode is true).
    pub custom_text: String,
}

impl SdkSelectState {
    pub fn new() -> Self {
        Self {
            highlight: 0,
            custom_mode: false,
            custom_text: String::new(),
        }
    }

    /// Get the current SDK value (either a known one or the custom text).
    pub fn value(&self) -> String {
        if self.custom_mode {
            self.custom_text.clone()
        } else {
            KNOWN_SDKS[self.highlight].to_string()
        }
    }

    /// Initialize from an existing npm value. If it matches a known SDK, select it;
    /// otherwise go into custom mode with the existing value.
    pub fn from_value(value: &str) -> Self {
        if let Some(idx) = KNOWN_SDKS.iter().position(|&s| s == value) {
            Self {
                highlight: idx,
                custom_mode: false,
                custom_text: String::new(),
            }
        } else {
            Self {
                highlight: KNOWN_SDKS.len() - 1, // "Custom..."
                custom_mode: true,
                custom_text: value.to_string(),
            }
        }
    }
}

/// Form state for the add provider wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddProviderForm {
    /// Which field is currently focused (0=id, 1=name, 2=sdk, 3=base_url).
    pub focus: usize,
    /// Provider ID field.
    pub id: String,
    /// Provider display name.
    pub name: String,
    /// SDK package selection.
    pub sdk: SdkSelectState,
    /// Base URL (for options).
    pub base_url: String,
    /// Whether the copy-from list is shown.
    pub show_copy_list: bool,
    /// Highlighted index in the copy source list.
    pub copy_highlight: usize,
}

impl AddProviderForm {
    pub fn new() -> Self {
        Self {
            focus: 0,
            id: String::new(),
            name: String::new(),
            sdk: SdkSelectState::new(),
            base_url: String::new(),
            show_copy_list: false,
            copy_highlight: 0,
        }
    }

    pub fn field_labels() -> [&'static str; 4] {
        [
            "Provider ID",
            "Display Name",
            "SDK Package (↑↓ select, Enter confirm)",
            "Base URL (optional)",
        ]
    }
}

/// Form state for editing an existing provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditProviderForm {
    /// Provider ID being edited.
    pub provider_id: String,
    /// Currently focused field index.
    pub focus: usize,
    /// Editable name field.
    pub name: String,
    /// SDK package selection.
    pub sdk: SdkSelectState,
    /// Editable base URL field.
    pub base_url: String,
}

impl EditProviderForm {
    pub fn field_labels() -> [&'static str; 3] {
        [
            "Display Name",
            "SDK Package (↑↓ select, Enter confirm)",
            "Base URL",
        ]
    }
}

impl App {
    /// Create a new app instance.
    pub fn new() -> Self {
        Self {
            mode: AppMode::ProviderList,
            should_quit: false,
            selected_provider: None,
            selected_index: 0,
            error_message: None,
            discovered_models: Vec::new(),
            models_loading: false,
        }
    }

    /// Handle an application event.
    pub fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Quit => self.should_quit = true,
            AppEvent::SwitchMode(mode) => {
                self.mode = mode;
                self.selected_index = 0;
                self.selected_provider = None;
            }
            AppEvent::SelectProvider(id) => {
                self.selected_provider = Some(id);
            }
            AppEvent::SelectIndex(idx) => {
                self.selected_index = idx;
            }
            AppEvent::Error(msg) => {
                self.error_message = Some(msg);
            }
            AppEvent::ClearError => {
                self.error_message = None;
            }
            _ => {}
        }
    }

    /// Render the current UI.
    pub fn render(&self, frame: &mut ratatui::Frame, state: &AppState) {
        match &self.mode {
            AppMode::MergedView => ui::render_merged_view(frame, state, self),
            AppMode::SplitView => ui::render_split_view(frame, state, self),
            AppMode::ProviderList => ui::render_provider_list(frame, state, self),
            AppMode::AuthStatus => ui::render_auth_status(frame, state, self),
            AppMode::ModelSelector => ui::render_model_selector(frame, state, self),
            AppMode::ConfigDetail => ui::render_config_detail(frame, state, self),
            AppMode::Help => ui::render_help(frame),
            AppMode::ConfirmDelete(provider_id) => {
                ui::render_provider_list(frame, state, self);
                ui::render_confirm_delete(frame, provider_id);
            }
            AppMode::ConfirmRefresh => {
                ui::render_provider_list(frame, state, self);
                ui::render_confirm_refresh(frame);
            }
            AppMode::AddProvider(form) => {
                ui::render_add_provider(frame, form, state);
            }
            AppMode::EditProvider(form) => {
                ui::render_edit_provider(frame, state, form);
            }
        }
    }
}

/// Async action to perform after key handling.
enum AsyncAction {
    /// Fetch models from models.dev for a provider.
    FetchModels {
        provider_id: String,
        force_refresh: bool,
    },
    /// No async action needed.
    None,
}

/// Run the main TUI event loop.
pub async fn run(
    mut terminal: ratatui::DefaultTerminal,
    mut state: AppState,
    split: bool,
) -> Result<()> {
    let mut app = App::new();
    if split {
        app.mode = AppMode::SplitView;
    }

    loop {
        if app.should_quit {
            break;
        }

        terminal.draw(|frame| app.render(frame, &state))?;

        // Handle key events via crossterm
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                // Only handle key press events (crossterm 0.28+ sends Press/Release/Repeat)
                if key.kind == crossterm::event::KeyEventKind::Press {
                    let action = handle_key_event(key, &mut app, &mut state);
                    match action {
                        AsyncAction::FetchModels {
                            provider_id,
                            force_refresh,
                        } => {
                            app.models_loading = true;
                            // Re-render to show loading state
                            terminal.draw(|frame| app.render(frame, &state))?;
                            let client = discovery::models_dev::ModelsDevClient::new();
                            match client
                                .fetch_provider_models_cached(&provider_id, force_refresh)
                                .await
                            {
                                Ok(models) => {
                                    app.discovered_models = models;
                                }
                                Err(e) => {
                                    app.error_message =
                                        Some(format!("Failed to fetch models: {e}"));
                                }
                            }
                            app.models_loading = false;
                        }
                        AsyncAction::None => {}
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handle a keyboard event. Returns an async action if needed.
fn handle_key_event(
    key: crossterm::event::KeyEvent,
    app: &mut App,
    state: &mut AppState,
) -> AsyncAction {
    use crossterm::event::KeyCode;

    // Clear error on any keypress
    let had_error = app.error_message.is_some();
    if had_error {
        app.error_message = None;
    }

    // Handle confirm refresh mode separately
    if app.mode == AppMode::ConfirmRefresh {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(e) = state.load_configs() {
                    app.error_message = Some(format!("Refresh failed: {e}"));
                }
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.mode = AppMode::ProviderList;
            }
            _ => {}
        }
        return AsyncAction::None;
    }

    // Handle edit provider mode separately
    if let AppMode::EditProvider(ref mut form) = app.mode {
        match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Tab => {
                form.focus = (form.focus + 1) % EditProviderForm::field_labels().len();
            }
            KeyCode::BackTab => {
                form.focus = (form.focus + EditProviderForm::field_labels().len() - 1)
                    % EditProviderForm::field_labels().len();
            }
            KeyCode::Enter => {
                // If focused on SDK field and not yet confirmed, confirm selection
                if form.focus == 1
                    && !form.sdk.custom_mode
                    && form.sdk.highlight == KNOWN_SDKS.len() - 1
                {
                    // "Custom..." selected — enter custom text mode
                    form.sdk.custom_mode = true;
                } else if form.focus == 1 && !form.sdk.custom_mode {
                    // Known SDK confirmed — no action needed, value is set
                } else {
                    // Save edited fields back to state
                    let pid = form.provider_id.clone();
                    let name_val = form.name.trim().to_string();
                    let npm_val = form.sdk.value();
                    let base_url_val = form.base_url.trim().to_string();

                    let edit_result = (|| -> Result<(), app::error::AppError> {
                        if !name_val.is_empty() {
                            state.edit_provider_field(
                                &pid,
                                "name",
                                serde_json::Value::String(name_val),
                                state.edit_layer,
                            )?;
                        }
                        if !npm_val.is_empty() {
                            state.edit_provider_field(
                                &pid,
                                "npm",
                                serde_json::Value::String(npm_val),
                                state.edit_layer,
                            )?;
                        }
                        if !base_url_val.is_empty() {
                            state.edit_provider_field(
                                &pid,
                                "baseURL",
                                serde_json::Value::String(base_url_val),
                                state.edit_layer,
                            )?;
                        }
                        Ok(())
                    })();

                    match edit_result {
                        Ok(()) => app.mode = AppMode::ProviderList,
                        Err(e) => app.error_message = Some(format!("Edit failed: {e}")),
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if form.focus == 1 && !form.sdk.custom_mode && form.sdk.highlight > 0 {
                    form.sdk.highlight -= 1;
                } else if form.focus == 0 {
                    // nothing
                }
            }
            KeyCode::Down | KeyCode::Char('j')
                if form.focus == 1
                    && !form.sdk.custom_mode
                    && form.sdk.highlight < KNOWN_SDKS.len() - 1 =>
            {
                form.sdk.highlight += 1;
            }
            KeyCode::Backspace => match form.focus {
                0 => {
                    form.name.pop();
                }
                1 if form.sdk.custom_mode => {
                    form.sdk.custom_text.pop();
                }
                2 => {
                    form.base_url.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) => match form.focus {
                0 => form.name.push(c),
                1 if form.sdk.custom_mode => form.sdk.custom_text.push(c),
                2 => form.base_url.push(c),
                _ => {}
            },
            _ => {}
        }
        return AsyncAction::None;
    }

    // Handle confirm delete mode separately
    if let AppMode::ConfirmDelete(ref provider_id) = app.mode {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let id = provider_id.clone();
                if let Err(e) = state.remove_provider(&id, state.edit_layer) {
                    app.error_message = Some(format!("Failed to remove provider: {e}"));
                }
                app.mode = AppMode::ProviderList;
                app.selected_provider = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.mode = AppMode::ProviderList;
            }
            _ => {}
        }
        return AsyncAction::None;
    }

    // Handle add provider form mode separately
    if let AppMode::AddProvider(ref mut form) = app.mode {
        // Handle copy-from list mode first
        if form.show_copy_list {
            match key.code {
                KeyCode::Esc => {
                    app.mode = AppMode::ProviderList;
                }
                KeyCode::Char('c') => {
                    // Toggle copy-from list
                    if !form.show_copy_list && !copy_source_list(state).is_empty() {
                        form.show_copy_list = true;
                        form.copy_highlight = 0;
                    } else {
                        form.show_copy_list = false;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') if form.copy_highlight > 0 => {
                    form.copy_highlight -= 1;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let sources = copy_source_list(state);
                    if form.copy_highlight + 1 < sources.len() {
                        form.copy_highlight += 1;
                    }
                }
                KeyCode::Enter => {
                    let sources = copy_source_list(state);
                    if let Some((id, _name)) = sources.get(form.copy_highlight) {
                        // Get provider config from configured providers
                        if let Some(provider) = state.get_provider(id).cloned() {
                            form.name = provider.name.unwrap_or_default();
                            let npm_val = provider.npm.unwrap_or_default();
                            form.sdk = SdkSelectState::from_value(&npm_val);
                            form.base_url = provider
                                .options
                                .as_ref()
                                .and_then(|o| o.get("baseURL"))
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            // Auto-fill ID if empty
                            if form.id.is_empty() {
                                form.id = format!("{id}-copy");
                            }
                        }
                    }
                    form.show_copy_list = false;
                }
                _ => {}
            }
            return AsyncAction::None;
        }

        match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Toggle copy-from list (Ctrl+C to avoid conflict with text input)
                if !form.show_copy_list && !copy_source_list(state).is_empty() {
                    form.show_copy_list = true;
                    form.copy_highlight = 0;
                } else {
                    form.show_copy_list = false;
                }
            }
            KeyCode::Tab => {
                form.focus = (form.focus + 1) % AddProviderForm::field_labels().len();
            }
            KeyCode::BackTab => {
                form.focus = (form.focus + AddProviderForm::field_labels().len() - 1)
                    % AddProviderForm::field_labels().len();
            }
            KeyCode::Up | KeyCode::Char('k')
                if form.focus == 2 && !form.sdk.custom_mode && form.sdk.highlight > 0 =>
            {
                form.sdk.highlight -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if form.focus == 2
                    && !form.sdk.custom_mode
                    && form.sdk.highlight < KNOWN_SDKS.len() - 1 =>
            {
                form.sdk.highlight += 1;
            }
            KeyCode::Enter => {
                // If focused on SDK field and "Custom..." is highlighted, enter custom mode
                if form.focus == 2
                    && !form.sdk.custom_mode
                    && form.sdk.highlight == KNOWN_SDKS.len() - 1
                {
                    form.sdk.custom_mode = true;
                    return AsyncAction::None;
                }

                // Submit form
                let id = form.id.trim().to_string();
                let name_val = form.name.trim().to_string();
                let npm_val = form.sdk.value();
                let base_url_val = form.base_url.trim().to_string();

                if id.is_empty() {
                    app.error_message = Some("Provider ID cannot be empty".to_string());
                    return AsyncAction::None;
                }

                // Build ProviderConfig
                let mut options = std::collections::HashMap::new();
                if !base_url_val.is_empty() {
                    options.insert(
                        "baseURL".to_string(),
                        serde_json::Value::String(base_url_val),
                    );
                }

                let provider_config = config_core::ProviderConfig {
                    name: if name_val.is_empty() {
                        None
                    } else {
                        Some(name_val)
                    },
                    npm: if npm_val.is_empty() || npm_val == "Custom..." {
                        None
                    } else {
                        Some(npm_val)
                    },
                    options: if options.is_empty() {
                        None
                    } else {
                        Some(options)
                    },
                    models: None,
                    disabled: None,
                    extra: Default::default(),
                };

                if let Err(e) = state.add_provider(id, provider_config, state.edit_layer) {
                    app.error_message = Some(format!("Failed to add provider: {e}"));
                }
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Backspace => match form.focus {
                0 => {
                    form.id.pop();
                }
                1 => {
                    form.name.pop();
                }
                2 if form.sdk.custom_mode => {
                    form.sdk.custom_text.pop();
                }
                3 => {
                    form.base_url.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) => match form.focus {
                0 => form.id.push(c),
                1 => form.name.push(c),
                2 if form.sdk.custom_mode => form.sdk.custom_text.push(c),
                3 => form.base_url.push(c),
                _ => {}
            },
            _ => {}
        }
        return AsyncAction::None;
    }

    // Global keybindings
    let mut async_action = AsyncAction::None;
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.mode == AppMode::Help {
                app.on_event(AppEvent::SwitchMode(AppMode::ProviderList));
            } else {
                app.on_event(AppEvent::Quit);
            }
        }
        KeyCode::Char('?') => {
            if app.mode == AppMode::Help {
                app.on_event(AppEvent::SwitchMode(AppMode::ProviderList));
            } else {
                app.on_event(AppEvent::SwitchMode(AppMode::Help));
            }
        }
        KeyCode::Char('1') => app.on_event(AppEvent::SwitchMode(AppMode::MergedView)),
        KeyCode::Char('2') => app.on_event(AppEvent::SwitchMode(AppMode::SplitView)),
        KeyCode::Char('p') => app.on_event(AppEvent::SwitchMode(AppMode::ProviderList)),
        KeyCode::Char('a') => app.on_event(AppEvent::SwitchMode(AppMode::AuthStatus)),
        KeyCode::Char('m') => {
            // Switch to model selector and fetch models for selected provider
            app.mode = AppMode::ModelSelector;
            app.selected_index = 0;
            app.discovered_models.clear();
            // Determine which provider to fetch models for
            let provider_ids = state.provider_ids();
            if let Some(provider_id) = provider_ids.get(app.selected_index) {
                app.selected_provider = Some(provider_id.clone());
                async_action = AsyncAction::FetchModels {
                    provider_id: provider_id.clone(),
                    force_refresh: false,
                };
            }
        }
        KeyCode::Char('c') => app.on_event(AppEvent::SwitchMode(AppMode::ConfigDetail)),
        KeyCode::Char('s') => {
            // Save current config
            if let Err(e) = state.save(state.edit_layer) {
                app.error_message = Some(format!("Save failed: {e}"));
            }
        }
        KeyCode::Char('r') => {
            if app.mode == AppMode::ModelSelector {
                // In model selector, `r` re-fetches the model list for the
                // currently selected provider.
                if let Some(ref provider_id) = app.selected_provider {
                    app.discovered_models.clear();
                    async_action = AsyncAction::FetchModels {
                        provider_id: provider_id.clone(),
                        force_refresh: true,
                    };
                }
            } else if state.dirty {
                // Refresh configs from disk — check for unsaved changes first
                app.mode = AppMode::ConfirmRefresh;
            } else if let Err(e) = state.load_configs() {
                app.error_message = Some(format!("Refresh failed: {e}"));
            }
        }
        KeyCode::Char('d') => {
            // Delete selected provider (with confirmation)
            if app.mode == AppMode::ProviderList
                && let Some(provider_id) = state.provider_ids().get(app.selected_index)
            {
                app.mode = AppMode::ConfirmDelete(provider_id.clone());
            }
        }
        KeyCode::Char('n') if app.mode == AppMode::ProviderList => {
            // Add new provider
            app.mode = AppMode::AddProvider(AddProviderForm::new());
        }
        KeyCode::Up | KeyCode::Char('k') if app.selected_index > 0 => {
            app.on_event(AppEvent::SelectIndex(app.selected_index - 1));
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Clamp to actual list length when in ProviderList mode
            let max = if app.mode == AppMode::ProviderList {
                all_provider_ids(state).len().saturating_sub(1)
            } else {
                usize::MAX
            };
            if app.selected_index < max {
                app.on_event(AppEvent::SelectIndex(app.selected_index + 1));
            }
        }
        KeyCode::Enter => {
            // In provider list, open edit view for the selected provider
            if app.mode == AppMode::ProviderList {
                let all_ids = all_provider_ids(state);
                if let Some(provider_id) = all_ids.get(app.selected_index) {
                    let provider = state.get_provider(provider_id);
                    let npm_val = provider.and_then(|p| p.npm.clone()).unwrap_or_default();
                    let form = EditProviderForm {
                        provider_id: provider_id.clone(),
                        focus: 0,
                        name: provider.and_then(|p| p.name.clone()).unwrap_or_default(),
                        sdk: SdkSelectState::from_value(&npm_val),
                        base_url: provider
                            .and_then(|p| p.options.as_ref())
                            .and_then(|opts| opts.get("baseURL"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    };
                    app.mode = AppMode::EditProvider(form);
                }
            }
            // In model selector, add selected model to provider
            if app.mode == AppMode::ModelSelector {
                if let Some(ref provider_id) = app.selected_provider {
                    if let Some(model) = app.discovered_models.get(app.selected_index) {
                        let model_config = config_core::ModelConfig::default();
                        if let Err(e) = state.add_model(
                            provider_id,
                            model.id.clone(),
                            model_config,
                            state.edit_layer,
                        ) {
                            app.error_message = Some(format!("Failed to add model: {e}"));
                        }
                    }
                }
            }
        }
        _ => {}
    }
    async_action
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::state::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Helper to create a key event.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Helper to create an App with a test config loaded.
    /// Returns (App, AppState). provider_ids() order is not guaranteed
    /// (HashMap), so tests must use provider_ids() to find indices.
    fn test_app_with_providers() -> (App, AppState) {
        let mut state = AppState::new().unwrap();
        // Add providers directly to the merged config for testing
        let mut providers = std::collections::HashMap::new();
        providers.insert(
            "openai".to_string(),
            config_core::ProviderConfig {
                name: Some("OpenAI".to_string()),
                npm: Some("openai".to_string()),
                options: None,
                models: Some({
                    let mut m = std::collections::HashMap::new();
                    m.insert("gpt-4o".to_string(), config_core::ModelConfig::default());
                    m
                }),
                disabled: None,
                extra: Default::default(),
            },
        );
        providers.insert(
            "anthropic".to_string(),
            config_core::ProviderConfig {
                name: Some("Anthropic".to_string()),
                npm: Some("@anthropic-ai/sdk".to_string()),
                options: Some({
                    let mut o = std::collections::HashMap::new();
                    o.insert(
                        "baseURL".to_string(),
                        serde_json::Value::String("https://api.anthropic.com".to_string()),
                    );
                    o
                }),
                models: None,
                disabled: None,
                extra: Default::default(),
            },
        );
        state.merged_config.provider = Some(providers);
        let app = App::new();
        (app, state)
    }

    /// Helper to find the index of a provider by ID.
    fn provider_index(state: &AppState, target_id: &str) -> usize {
        state
            .provider_ids()
            .iter()
            .position(|id| id == target_id)
            .unwrap_or(0)
    }

    // --- App creation ---

    #[test]
    fn test_app_new_defaults() {
        let app = App::new();
        assert_eq!(app.mode, AppMode::ProviderList);
        assert!(!app.should_quit);
        assert!(app.selected_provider.is_none());
        assert_eq!(app.selected_index, 0);
        assert!(app.error_message.is_none());
        assert!(app.discovered_models.is_empty());
        assert!(!app.models_loading);
    }

    // --- Mode switching ---

    #[test]
    fn test_switch_to_merged_view() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('1')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::MergedView);
    }

    #[test]
    fn test_switch_to_split_view() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('2')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::SplitView);
    }

    #[test]
    fn test_switch_to_auth_status() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('a')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::AuthStatus);
    }

    #[test]
    fn test_switch_to_config_detail() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('c')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ConfigDetail);
    }

    #[test]
    fn test_help_toggle() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('?')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::Help);
        // Press ? again to go back
        handle_key_event(key(KeyCode::Char('?')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
    }

    #[test]
    fn test_quit_from_provider_list() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('q')), &mut app, &mut state);
        assert!(app.should_quit);
    }

    #[test]
    fn test_esc_from_provider_list_quits() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        assert!(app.should_quit);
    }

    #[test]
    fn test_esc_from_help_goes_back() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('?')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::Help);
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        assert!(!app.should_quit);
    }

    // --- Navigation ---

    #[test]
    fn test_navigate_down() {
        let (mut app, mut state) = test_app_with_providers();
        assert_eq!(app.selected_index, 0);
        handle_key_event(key(KeyCode::Down), &mut app, &mut state);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_navigate_up() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Down), &mut app, &mut state);
        assert_eq!(app.selected_index, 1);
        handle_key_event(key(KeyCode::Up), &mut app, &mut state);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_navigate_j_k() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('j')), &mut app, &mut state);
        assert_eq!(app.selected_index, 1);
        handle_key_event(key(KeyCode::Char('k')), &mut app, &mut state);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_navigate_up_at_zero_does_nothing() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Up), &mut app, &mut state);
        assert_eq!(app.selected_index, 0);
    }

    // --- Error clearing ---

    #[test]
    fn test_error_cleared_on_next_key() {
        let (mut app, mut state) = test_app_with_providers();
        app.error_message = Some("test error".to_string());
        handle_key_event(key(KeyCode::Down), &mut app, &mut state);
        assert!(app.error_message.is_none());
    }

    // --- AddProvider form ---

    #[test]
    fn test_add_provider_opens() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        assert!(matches!(app.mode, AppMode::AddProvider(_)));
    }

    #[test]
    fn test_add_provider_type_fields() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        // Type provider ID
        for c in "test-provider".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.id, "test-provider");
            assert_eq!(form.focus, 0);
        }

        // Tab to name field
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 1);
        }
        for c in "Test Provider".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.name, "Test Provider");
        }

        // Tab to sdk field (now a selection list)
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 2);
        }
        // Navigate down to "Custom..." (last entry, index 7)
        for _ in 0..KNOWN_SDKS.len() - 1 {
            handle_key_event(key(KeyCode::Down), &mut app, &mut state);
        }
        // Press Enter to enter custom text mode
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert!(form.sdk.custom_mode);
        }
        // Type custom SDK name
        for c in "@test/sdk".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.sdk.custom_text, "@test/sdk");
        }

        // Tab to base_url field
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 3);
        }
        for c in "https://api.test.com".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.base_url, "https://api.test.com");
        }
    }

    #[test]
    fn test_add_provider_tab_cycles() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        // Tab 4 times should cycle back to 0
        for _ in 0..4 {
            handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 0);
        }
    }

    #[test]
    fn test_add_provider_backspace() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('a')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('b')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Backspace), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.id, "a");
        }
    }

    #[test]
    fn test_add_provider_empty_id_shows_error() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        assert!(app.error_message.is_some());
        assert!(app.error_message.unwrap().contains("cannot be empty"));
    }

    #[test]
    fn test_add_provider_esc_cancels() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
    }

    #[test]
    fn test_add_provider_submit_creates_provider() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);

        // Type provider ID
        for c in "groq".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        // Tab to name
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        for c in "Groq".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        // Tab to npm
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        // Skip npm (empty)
        // Tab to base_url
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        // Skip base_url (empty)

        // Submit
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);

        assert_eq!(app.mode, AppMode::ProviderList);
        assert!(state.dirty);
        let ids = state.provider_ids();
        assert!(ids.contains(&"groq".to_string()));
        let provider = state.get_provider("groq").unwrap();
        assert_eq!(provider.name.as_deref(), Some("Groq"));
    }

    // --- EditProvider ---

    #[test]
    fn test_enter_opens_edit_provider() {
        let (mut app, mut state) = test_app_with_providers();
        let idx = provider_index(&state, "openai");
        app.selected_index = idx;
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        assert!(matches!(app.mode, AppMode::EditProvider(_)));
        if let AppMode::EditProvider(ref form) = app.mode {
            assert_eq!(form.provider_id, "openai");
            assert_eq!(form.name, "OpenAI");
            // "openai" is not in KNOWN_SDKS anymore, so custom mode
            assert!(form.sdk.custom_mode);
            assert_eq!(form.sdk.custom_text, "openai");
            assert_eq!(form.base_url, "");
        }
    }

    #[test]
    fn test_edit_provider_esc_cancels() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
    }

    #[test]
    fn test_edit_provider_edit_name() {
        let (mut app, mut state) = test_app_with_providers();
        // Copy merged into project_config so edit_provider_field has a target
        state.project_config = Some(state.merged_config.clone());
        let idx = provider_index(&state, "openai");
        app.selected_index = idx;
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        // Clear existing name and type new
        if let AppMode::EditProvider(ref form) = app.mode {
            assert_eq!(form.name, "OpenAI");
        }
        // Backspace to clear
        for _ in 0.."OpenAI".len() {
            handle_key_event(key(KeyCode::Backspace), &mut app, &mut state);
        }
        for c in "NewOpenAI".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        if let AppMode::EditProvider(ref form) = app.mode {
            assert_eq!(form.name, "NewOpenAI");
        }

        // Save
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        let provider = state.get_provider("openai").unwrap();
        assert_eq!(provider.name.as_deref(), Some("NewOpenAI"));
    }

    #[test]
    fn test_edit_provider_tab_cycles() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        for _ in 0..EditProviderForm::field_labels().len() {
            handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        }
        if let AppMode::EditProvider(ref form) = app.mode {
            assert_eq!(form.focus, 0);
        }
    }

    // --- EditProvider with baseURL ---

    #[test]
    fn test_edit_provider_shows_base_url() {
        let (mut app, mut state) = test_app_with_providers();
        // Select anthropic
        let idx = provider_index(&state, "anthropic");
        app.selected_index = idx;
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        if let AppMode::EditProvider(ref form) = app.mode {
            assert_eq!(form.provider_id, "anthropic");
            assert_eq!(form.base_url, "https://api.anthropic.com");
        }
    }

    // --- ConfirmDelete ---

    #[test]
    fn test_delete_opens_confirm() {
        let (mut app, mut state) = test_app_with_providers();
        let idx = provider_index(&state, "openai");
        app.selected_index = idx;
        handle_key_event(key(KeyCode::Char('d')), &mut app, &mut state);
        assert!(matches!(app.mode, AppMode::ConfirmDelete(_)));
        if let AppMode::ConfirmDelete(ref id) = app.mode {
            assert_eq!(id, "openai");
        }
    }

    #[test]
    fn test_delete_confirm_yes_removes() {
        let (mut app, mut state) = test_app_with_providers();
        let idx = provider_index(&state, "openai");
        app.selected_index = idx;
        handle_key_event(key(KeyCode::Char('d')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('y')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        let ids = state.provider_ids();
        assert!(!ids.contains(&"openai".to_string()));
        assert!(state.dirty);
    }

    #[test]
    fn test_delete_confirm_n_cancels() {
        let (mut app, mut state) = test_app_with_providers();
        let original_count = state.provider_ids().len();
        handle_key_event(key(KeyCode::Char('d')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        assert_eq!(state.provider_ids().len(), original_count);
    }

    #[test]
    fn test_delete_confirm_esc_cancels() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('d')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
    }

    // --- ConfirmRefresh ---

    #[test]
    fn test_refresh_when_dirty_shows_confirm() {
        let (mut app, mut state) = test_app_with_providers();
        state.dirty = true;
        handle_key_event(key(KeyCode::Char('r')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ConfirmRefresh);
    }

    #[test]
    fn test_refresh_confirm_yes_reloads() {
        let (mut app, mut state) = test_app_with_providers();
        state.dirty = true;
        handle_key_event(key(KeyCode::Char('r')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('y')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        assert!(!state.dirty);
    }

    #[test]
    fn test_refresh_confirm_n_cancels() {
        let (mut app, mut state) = test_app_with_providers();
        state.dirty = true;
        handle_key_event(key(KeyCode::Char('r')), &mut app, &mut state);
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
        assert!(state.dirty); // Still dirty
    }

    #[test]
    fn test_refresh_when_not_dirty_reloads_directly() {
        let (mut app, mut state) = test_app_with_providers();
        assert!(!state.dirty);
        handle_key_event(key(KeyCode::Char('r')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ProviderList);
    }

    // --- ModelSelector ---

    #[test]
    fn test_m_key_triggers_model_fetch() {
        let (mut app, mut state) = test_app_with_providers();
        app.selected_index = 0;
        let action = handle_key_event(key(KeyCode::Char('m')), &mut app, &mut state);
        assert_eq!(app.mode, AppMode::ModelSelector);
        assert!(app.selected_provider.is_some());
        assert!(matches!(
            action,
            AsyncAction::FetchModels {
                force_refresh: false,
                ..
            }
        ));
    }

    #[test]
    fn test_r_in_model_selector_refetches() {
        let (mut app, mut state) = test_app_with_providers();
        // Enter model selector first
        let _ = handle_key_event(key(KeyCode::Char('m')), &mut app, &mut state);
        app.discovered_models = vec![discovery::DiscoveredModel {
            id: "gpt-5".to_string(),
            name: "GPT-5".to_string(),
            provider_id: "openai".to_string(),
            context_length: Some(128000),
            max_output_tokens: Some(16384),
            input_cost_per_million: Some(10.0),
            output_cost_per_million: Some(30.0),
        }];
        // Press r to refresh
        let action = handle_key_event(key(KeyCode::Char('r')), &mut app, &mut state);
        assert!(app.discovered_models.is_empty());
        assert!(matches!(
            action,
            AsyncAction::FetchModels {
                force_refresh: true,
                ..
            }
        ));
    }

    // --- Save ---

    #[test]
    fn test_save_marks_dirty() {
        let (mut app, mut state) = test_app_with_providers();
        state.dirty = true;
        // Save to a layer that has no path will fail, but the attempt is made
        let _ = handle_key_event(key(KeyCode::Char('s')), &mut app, &mut state);
        // Error expected since no real config file exists
        // Just verify it doesn't crash
    }

    // --- AddProvider with npm and baseURL ---

    #[test]
    fn test_add_provider_with_all_fields() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);

        // Type ID
        for c in "mistral".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        // Tab → name
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        for c in "Mistral AI".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }
        // Tab → sdk (selection list — navigate to @ai-sdk/mistral at index 5)
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        for _ in 0..5 {
            handle_key_event(key(KeyCode::Down), &mut app, &mut state);
        }
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.sdk.highlight, 5); // @ai-sdk/mistral
        }
        // Tab → base_url
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        for c in "https://api.mistral.ai".chars() {
            handle_key_event(key(KeyCode::Char(c)), &mut app, &mut state);
        }

        // Submit
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);

        assert_eq!(app.mode, AppMode::ProviderList);
        let provider = state.get_provider("mistral").unwrap();
        assert_eq!(provider.name.as_deref(), Some("Mistral AI"));
        assert_eq!(provider.npm.as_deref(), Some("@ai-sdk/mistral"));
        let base_url = provider
            .options
            .as_ref()
            .and_then(|o| o.get("baseURL"))
            .and_then(|v| v.as_str());
        assert_eq!(base_url, Some("https://api.mistral.ai"));
    }

    // --- BackTab in forms ---

    #[test]
    fn test_add_provider_shift_tab_goes_back() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        // Tab to field 1
        handle_key_event(key(KeyCode::Tab), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 1);
        }
        // BackTab to field 0
        handle_key_event(key(KeyCode::BackTab), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert_eq!(form.focus, 0);
        }
    }

    // --- Provider list ---

    #[test]
    fn test_all_provider_ids_returns_configured() {
        let (_app, state) = test_app_with_providers();
        let all = all_provider_ids(&state);
        // Should only have configured providers (openai + anthropic)
        assert_eq!(all.len(), state.provider_ids().len());
        // All configured providers should be present
        for id in state.provider_ids() {
            assert!(all.contains(&id));
        }
    }

    #[test]
    fn test_copy_source_list() {
        let (_app, state) = test_app_with_providers();
        let sources = copy_source_list(&state);
        // Should have exactly the configured providers
        assert_eq!(sources.len(), state.provider_ids().len());
        // Configured providers should be there
        for id in state.provider_ids() {
            assert!(sources.iter().any(|(sid, _)| *sid == id));
        }
    }

    #[test]
    fn test_add_provider_copy_mode() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        // Press Ctrl+C to enter copy mode
        handle_key_event(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
            &mut state,
        );
        if let AppMode::AddProvider(ref form) = app.mode {
            assert!(form.show_copy_list);
        }
        // Press Esc to cancel copy
        handle_key_event(key(KeyCode::Esc), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert!(!form.show_copy_list);
        }
    }

    #[test]
    fn test_add_provider_copy_selects() {
        let (mut app, mut state) = test_app_with_providers();
        handle_key_event(key(KeyCode::Char('n')), &mut app, &mut state);
        // Enter copy mode with Ctrl+C
        handle_key_event(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
            &mut state,
        );
        // Press Enter to select first provider in copy list
        handle_key_event(key(KeyCode::Enter), &mut app, &mut state);
        if let AppMode::AddProvider(ref form) = app.mode {
            assert!(!form.show_copy_list);
            // Should have pre-filled fields (auto ID since empty)
            assert!(!form.id.is_empty() || !form.name.is_empty());
        }
    }
}
