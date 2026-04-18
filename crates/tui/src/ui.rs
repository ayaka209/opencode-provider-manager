//! TUI UI rendering functions.

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use app::state::AppState;
use auth::provider_env_var;
use config_core::ConfigLayer;
use serde_json;

use crate::tui_app::App;

/// Color scheme for the TUI.
mod colors {
    use ratatui::style::Color;
    pub const PRIMARY: Color = Color::Cyan;
    pub const SUCCESS: Color = Color::Green;
    pub const WARNING: Color = Color::Yellow;
    pub const ERROR: Color = Color::Red;
    pub const DIM: Color = Color::DarkGray;
    #[allow(dead_code)]
    pub const HIGHLIGHT: Color = Color::White;
}

/// Auth status for a provider.
enum AuthStatus {
    Configured,
    EnvVar(String),
    Missing,
}

/// Check auth status for a provider (reuses auth crate logic).
fn check_provider_auth(provider_id: &str) -> AuthStatus {
    match provider_env_var(provider_id) {
        Some(var) if std::env::var(var).is_ok() => AuthStatus::EnvVar(var.to_string()),
        Some(_) => AuthStatus::Missing,
        None => AuthStatus::Configured,
    }
}

/// Render the provider list view.
pub fn render_provider_list(frame: &mut Frame, state: &AppState, app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    // Title
    let title = Paragraph::new("OpenCode Provider Manager")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered().title("opm"));
    frame.render_widget(title, title_area);

    // Provider list
    let provider_ids = state.provider_ids();
    let mut lines: Vec<Line> = Vec::new();

    for (i, id) in provider_ids.iter().enumerate() {
        let provider = state.get_provider(id);
        let model_count = provider
            .and_then(|p| p.models.as_ref())
            .map(|m| m.len())
            .unwrap_or(0);

        let auth_span = match check_provider_auth(id) {
            AuthStatus::Configured => {
                Span::styled(" [configured]", Style::default().fg(colors::SUCCESS))
            }
            AuthStatus::EnvVar(var) => Span::styled(
                format!(" [env:{var}]"),
                Style::default().fg(colors::WARNING),
            ),
            AuthStatus::Missing => Span::styled(" [no key]", Style::default().fg(colors::ERROR)),
        };

        let name = provider
            .and_then(|p| p.name.clone())
            .unwrap_or_else(|| id.clone());

        let style = if i == app.selected_index {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:2} ", i + 1), Style::default().fg(colors::DIM)),
            Span::styled(name.clone(), style),
            auth_span,
            Span::styled(
                format!("  ({} models)", model_count),
                Style::default().fg(colors::DIM),
            ),
        ]));
    }

    if provider_ids.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  No providers configured.",
            Style::default().fg(colors::DIM),
        )));
        lines.push(Line::from(Span::styled(
            "  Add providers to your opencode.json to get started.",
            Style::default().fg(colors::DIM),
        )));
    }

    let provider_view = Paragraph::new(lines)
        .block(Block::bordered().title("Providers"))
        .wrap(Wrap { trim: false });
    frame.render_widget(provider_view, main_area);

    // Status bar
    let layer = match state.edit_layer {
        ConfigLayer::Global => "Global",
        ConfigLayer::Project => "Project",
        ConfigLayer::Custom => "Custom",
    };
    let dirty = if app.dirty { " [unsaved]" } else { "" };
    let status =
        format!(" Layer: {layer}{dirty} | ?: Help | q: Quit | j/k: Navigate | Enter: Select ");
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the merged config view.
pub fn render_merged_view(frame: &mut Frame, state: &AppState, app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new("Merged Configuration")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    let config_json = serde_json::to_string_pretty(&state.merged_config)
        .unwrap_or_else(|_| "Error serializing config".to_string());

    let config_view = Paragraph::new(config_json)
        .block(Block::bordered().title("opm - Merged Config"))
        .wrap(Wrap { trim: false })
        .scroll((app.selected_index as u16, 0));
    frame.render_widget(config_view, main_area);

    frame.render_widget(
        Paragraph::new(" 1:Merged | 2:Split | p:Providers | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the split pane view (global vs project).
pub fn render_split_view(frame: &mut Frame, state: &AppState, _app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new("Split View: Global | Project")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    let horizontal = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
    let [left_area, right_area] = horizontal.areas(main_area);

    let global_json = state
        .global_config
        .as_ref()
        .map(|c| serde_json::to_string_pretty(c).unwrap_or_else(|_| "Error".to_string()))
        .unwrap_or_else(|| "No global config found".to_string());

    let global_view = Paragraph::new(global_json)
        .block(Block::bordered().title("Global (~/.config/opencode/opencode.json)"))
        .wrap(Wrap { trim: false });
    frame.render_widget(global_view, left_area);

    let project_json = state
        .project_config
        .as_ref()
        .map(|c| serde_json::to_string_pretty(c).unwrap_or_else(|_| "Error".to_string()))
        .unwrap_or_else(|| "No project config found".to_string());

    let project_view = Paragraph::new(project_json)
        .block(Block::bordered().title("Project (./opencode.json)"))
        .wrap(Wrap { trim: false });
    frame.render_widget(project_view, right_area);

    frame.render_widget(
        Paragraph::new(" 1:Merged | 2:Split | p:Providers | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the auth status view.
pub fn render_auth_status(frame: &mut Frame, state: &AppState, _app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new("Authentication Status")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    let auth_path = state.paths.auth.to_string_lossy().to_string();
    let auth_exists = state.paths.auth.exists();

    let content = format!(
        "Auth file: {auth_path}\nExists: {auth_exists}\n\nProvider authentication status:\n(Run 'opencode auth list' for full details)"
    );

    let auth_view = Paragraph::new(content)
        .block(Block::bordered().title("opm - Auth Status"))
        .wrap(Wrap { trim: false });
    frame.render_widget(auth_view, main_area);

    frame.render_widget(
        Paragraph::new(" p:Providers | a:Auth | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the model selector view.
pub fn render_model_selector(frame: &mut Frame, _state: &AppState, app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new("Model Discovery")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    let provider_id = app
        .selected_provider
        .as_deref()
        .unwrap_or("(none selected)");

    let content = format!(
        "Model discovery for provider: {provider_id}\n\nUse 'opm --refresh-models' to fetch available models\nfrom models.dev and provider APIs.\n\nThis feature will be available in a future version."
    );

    let model_view = Paragraph::new(content)
        .block(Block::bordered().title("opm - Model Selector"))
        .wrap(Wrap { trim: false });
    frame.render_widget(model_view, main_area);

    frame.render_widget(
        Paragraph::new(" p:Providers | m:Models | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the config detail view.
pub fn render_config_detail(frame: &mut Frame, state: &AppState, _app: &App) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new("Configuration Paths")
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    let paths_info = format!(
        "Global config: {}\n  Exists: {}\n\nProject config: {}\n  Exists: {}\n\nAuth file: {}\n  Exists: {}\n\nCache dir: {}",
        state.paths.global.to_string_lossy(),
        state.paths.global.exists(),
        state.paths.project.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "(not found)".to_string()),
        state.paths.project.as_ref().map(|p| p.exists()).unwrap_or(false),
        state.paths.auth.to_string_lossy(),
        state.paths.auth.exists(),
        state.paths.cache_dir.to_string_lossy(),
    );

    let detail_view = Paragraph::new(paths_info)
        .block(Block::bordered().title("opm - Config Paths"))
        .wrap(Wrap { trim: false });
    frame.render_widget(detail_view, main_area);

    frame.render_widget(
        Paragraph::new(" 1:Merged | 2:Split | p:Providers | c:Config | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

/// Render the help overlay.
pub fn render_help(frame: &mut Frame) {
    let help_text = vec![
        Line::from(Span::styled(
            "OpenCode Provider Manager - Help",
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q / Esc  ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Quit / Cancel"),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  1        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Merged config view"),
        ]),
        Line::from(vec![
            Span::styled("  2        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Split view (global vs project)"),
        ]),
        Line::from(vec![
            Span::styled("  p        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Provider list"),
        ]),
        Line::from(vec![
            Span::styled("  a        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Auth status"),
        ]),
        Line::from(vec![
            Span::styled("  m        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Model discovery"),
        ]),
        Line::from(vec![
            Span::styled("  c        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Config paths detail"),
        ]),
        Line::from(vec![
            Span::styled("  j / Down  ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Move down"),
        ]),
        Line::from(vec![
            Span::styled("  k / Up    ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Move up"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Select item"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::bordered()
                .title("opm - Help")
                .border_style(Style::default().fg(colors::PRIMARY)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(help, frame.area());
}
