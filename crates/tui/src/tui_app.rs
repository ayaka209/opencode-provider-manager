//! TUI application state and event loop.

use anyhow::Result;

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
    /// Add provider wizard (form with text inputs).
    AddProvider(AddProviderForm),
}

/// Form state for the add provider wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddProviderForm {
    /// Which field is currently focused (0=id, 1=name, 2=base_url).
    pub focus: usize,
    /// Provider ID field.
    pub id: String,
    /// Provider display name.
    pub name: String,
    /// Base URL (for options).
    pub base_url: String,
}

impl AddProviderForm {
    pub fn new() -> Self {
        Self {
            focus: 0,
            id: String::new(),
            name: String::new(),
            base_url: String::new(),
        }
    }

    pub fn field_labels() -> [&'static str; 3] {
        ["Provider ID", "Display Name", "Base URL (optional)"]
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
            AppMode::AddProvider(form) => {
                ui::render_add_provider(frame, form);
            }
        }
    }
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
                    handle_key_event(key, &mut app, &mut state);
                }
            }
        }
    }

    Ok(())
}

/// Handle a keyboard event.
fn handle_key_event(key: crossterm::event::KeyEvent, app: &mut App, state: &mut AppState) {
    use crossterm::event::KeyCode;

    // Clear error on any keypress
    let had_error = app.error_message.is_some();
    if had_error {
        app.error_message = None;
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
        return;
    }

    // Handle add provider form mode separately
    if let AppMode::AddProvider(ref mut form) = app.mode {
        match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Tab | KeyCode::Down => {
                form.focus = (form.focus + 1) % AddProviderForm::field_labels().len();
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = (form.focus + AddProviderForm::field_labels().len() - 1)
                    % AddProviderForm::field_labels().len();
            }
            KeyCode::Enter => {
                // Submit form
                let id = form.id.trim().to_string();
                let name_val = form.name.trim().to_string();
                let base_url_val = form.base_url.trim().to_string();

                if id.is_empty() {
                    app.error_message = Some("Provider ID cannot be empty".to_string());
                    return;
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
                    name: if name_val.is_empty() { None } else { Some(name_val) },
                    npm: None,
                    options: if options.is_empty() { None } else { Some(options) },
                    models: None,
                    disabled: None,
                };

                if let Err(e) = state.add_provider(id, provider_config, state.edit_layer) {
                    app.error_message = Some(format!("Failed to add provider: {e}"));
                }
                app.mode = AppMode::ProviderList;
            }
            KeyCode::Backspace => match form.focus {
                0 => { form.id.pop(); }
                1 => { form.name.pop(); }
                2 => { form.base_url.pop(); }
                _ => {}
            },
            KeyCode::Char(c) => match form.focus {
                0 => form.id.push(c),
                1 => form.name.push(c),
                2 => form.base_url.push(c),
                _ => {}
            },
            _ => {}
        }
        return;
    }

    // Global keybindings
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
        KeyCode::Char('m') => app.on_event(AppEvent::SwitchMode(AppMode::ModelSelector)),
        KeyCode::Char('c') => app.on_event(AppEvent::SwitchMode(AppMode::ConfigDetail)),
        KeyCode::Char('s') => {
            // Save current config
            if let Err(e) = state.save(state.edit_layer) {
                app.error_message = Some(format!("Save failed: {e}"));
            }
        }
        KeyCode::Char('r') => {
            // Refresh configs from disk
            if let Err(e) = state.load_configs() {
                app.error_message = Some(format!("Refresh failed: {e}"));
            }
        }
        KeyCode::Char('d') => {
            // Delete selected provider (with confirmation)
            if app.mode == AppMode::ProviderList {
                let provider_ids = state.provider_ids();
                if let Some(provider_id) = provider_ids.get(app.selected_index) {
                    app.mode = AppMode::ConfirmDelete(provider_id.clone());
                }
            }
        }
        KeyCode::Char('n') => {
            // Add new provider
            if app.mode == AppMode::ProviderList {
                app.mode = AppMode::AddProvider(AddProviderForm::new());
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_index > 0 {
                app.on_event(AppEvent::SelectIndex(app.selected_index - 1));
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.on_event(AppEvent::SelectIndex(app.selected_index.saturating_add(1)));
        }
        KeyCode::Enter => {
            // In provider list, select the provider at current index
            if app.mode == AppMode::ProviderList {
                let provider_ids = state.provider_ids();
                if let Some(provider_id) = provider_ids.get(app.selected_index) {
                    app.on_event(AppEvent::SelectProvider(provider_id.clone()));
                }
            }
        }
        _ => {}
    }
}