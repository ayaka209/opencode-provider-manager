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
    /// Whether config has unsaved changes.
    pub dirty: bool,
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
            dirty: false,
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
        match self.mode {
            AppMode::MergedView => ui::render_merged_view(frame, state, self),
            AppMode::SplitView => ui::render_split_view(frame, state, self),
            AppMode::ProviderList => ui::render_provider_list(frame, state, self),
            AppMode::AuthStatus => ui::render_auth_status(frame, state, self),
            AppMode::ModelSelector => ui::render_model_selector(frame, state, self),
            AppMode::ConfigDetail => ui::render_config_detail(frame, state, self),
            AppMode::Help => ui::render_help(frame),
        }
    }
}

/// Run the main TUI event loop.
pub async fn run(
    mut terminal: ratatui::DefaultTerminal,
    state: AppState,
) -> Result<()> {
    let mut app = App::new();

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
                    handle_key_event(key, &mut app, &state);
                }
            }
        }
    }

    Ok(())
}

/// Handle a keyboard event.
fn handle_key_event(key: crossterm::event::KeyEvent, app: &mut App, _state: &AppState) {
    use crossterm::event::KeyCode;

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
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_index > 0 {
                app.on_event(AppEvent::SelectIndex(app.selected_index - 1));
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Don't go below the list - bounds check handled in on_event
            app.on_event(AppEvent::SelectIndex(app.selected_index.saturating_add(1)));
        }
        KeyCode::Enter => {
            // In provider list, select the provider at current index
            if app.mode == AppMode::ProviderList {
                let provider_ids = _state.provider_ids();
                if let Some(provider_id) = provider_ids.get(app.selected_index) {
                    app.on_event(AppEvent::SelectProvider(provider_id.clone()));
                }
            }
        }
        _ => {}
    }
}