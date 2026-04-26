//! TUI UI rendering functions.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use app::state::AppState;
use auth::ProviderAuthStatus;
use auth::parser::{AuthEntries, parse_auth_file};
use config_core::{ConfigLayer, ProviderConfig};
use opencode_provider_manager::{app, auth, config_core};

use crate::tui_app::{
    AddProviderForm, App, EditProviderForm, KNOWN_SDKS, all_provider_ids,
    copy_source_list,
};

/// Helper for label styling based on focus state.
fn label_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(colors::PRIMARY)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(colors::DIM)
    }
}

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

/// Calculate scroll offset so the selected item stays visible in a list.
/// Returns (row_offset, 0) for use with `.scroll()`.
fn scroll_offset_for_list(selected: usize, total_items: usize, visible_height: u16) -> (u16, u16) {
    if visible_height == 0 || total_items == 0 {
        return (0, 0);
    }
    // Account for border (2 lines) in the widget area
    let usable = visible_height.saturating_sub(2) as usize;
    if usable == 0 {
        return (0, 0);
    }
    // Keep selected item within the visible window
    if selected >= usable {
        (selected.saturating_sub(usable.saturating_sub(1)) as u16, 0)
    } else {
        (0, 0)
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

    // Provider list — show configured providers
    let all_ids = all_provider_ids(state);
    let auth_entries = read_auth_entries(state).ok().flatten();
    let mut lines: Vec<Line> = Vec::new();

    for (i, id) in all_ids.iter().enumerate() {
        let provider = state.get_provider(id);
        let model_count = provider
            .and_then(|p| p.models.as_ref())
            .map(|m| m.len())
            .unwrap_or(0);

        let auth_span = match provider_auth_status(state, auth_entries.as_ref(), id, provider) {
            ProviderAuthStatus::Configured { format_valid: true } => {
                Span::styled(" [configured]", Style::default().fg(colors::SUCCESS))
            }
            ProviderAuthStatus::Configured {
                format_valid: false,
            } => Span::styled(" [configured?]", Style::default().fg(colors::WARNING)),
            ProviderAuthStatus::OAuth => {
                Span::styled(" [oauth]", Style::default().fg(colors::SUCCESS))
            }
            ProviderAuthStatus::EnvVar { var_name: var } => Span::styled(
                format!(" [env:{var}]"),
                Style::default().fg(colors::WARNING),
            ),
            ProviderAuthStatus::Missing => {
                Span::styled(" [no key]", Style::default().fg(colors::ERROR))
            }
        };

        let name = provider
            .and_then(|p| p.name.clone())
            .unwrap_or_else(|| id.clone());

        let style = if i == app.selected_index {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        let count_str = format!("{} models", model_count);

        lines.push(Line::from(vec![
            Span::styled(format!("  {:2} ", i + 1), Style::default().fg(colors::DIM)),
            Span::styled(name, style),
            auth_span,
            Span::styled(
                format!("  ({})", count_str),
                Style::default().fg(colors::DIM),
            ),
        ]));
    }

    if all_ids.is_empty() {
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
        .wrap(Wrap { trim: false })
        .scroll(scroll_offset_for_list(app.selected_index, all_ids.len(), main_area.height));
    frame.render_widget(provider_view, main_area);

    // Status bar
    let layer = match state.edit_layer {
        ConfigLayer::Global => "Global",
        ConfigLayer::Project => "Project",
        ConfigLayer::Custom => "Custom",
    };
    let dirty = if state.dirty { " ●" } else { "" };
    let status = format!(
        " Layer: {layer}{dirty} | n:New | s:Save | d:Delete | r:Refresh | ?:Help | q:Quit | j/k:Nav | Enter:Select "
    );
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(colors::DIM)),
        status_area,
    );

    // Error bar (if any)
    render_error_bar(frame, &app.error_message);
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

    let auth_entries_result = read_auth_entries(state);
    let auth_entries = auth_entries_result.as_ref().ok().and_then(|e| e.as_ref());
    let mut lines = vec![
        Line::from(format!("Auth file: {}", state.paths.auth.to_string_lossy())),
        Line::from(format!("Exists: {}", state.paths.auth.exists())),
    ];

    if let Err(e) = &auth_entries_result {
        lines.push(Line::from(Span::styled(
            format!("Read error: {e}"),
            Style::default().fg(colors::ERROR),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Provider authentication status:"));
    lines.push(Line::from(""));

    for provider_id in all_provider_ids(state) {
        let provider = state.get_provider(&provider_id);
        let (status_text, style) = match provider_auth_status(state, auth_entries, &provider_id, provider) {
            ProviderAuthStatus::Configured { format_valid: true } => (
                "configured in auth.json".to_string(),
                Style::default().fg(colors::SUCCESS),
            ),
            ProviderAuthStatus::Configured {
                format_valid: false,
            } => (
                "configured in auth.json (unrecognized key format)".to_string(),
                Style::default().fg(colors::WARNING),
            ),
            ProviderAuthStatus::OAuth => (
                "oauth token in auth.json".to_string(),
                Style::default().fg(colors::SUCCESS),
            ),
            ProviderAuthStatus::EnvVar { var_name } => (
                format!("environment variable: {var_name}"),
                Style::default().fg(colors::WARNING),
            ),
            ProviderAuthStatus::Missing => {
                ("missing".to_string(), Style::default().fg(colors::ERROR))
            }
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {provider_id:<18} "),
                Style::default().fg(colors::PRIMARY),
            ),
            Span::styled(status_text, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Secrets are never displayed. Use `/connect <provider-id>` in OpenCode to add credentials.",
        Style::default().fg(colors::DIM),
    )));

    let auth_view = Paragraph::new(lines)
        .block(Block::bordered().title("opm - Auth Status"))
        .wrap(Wrap { trim: false });
    frame.render_widget(auth_view, main_area);

    frame.render_widget(
        Paragraph::new(" p:Providers | a:Auth | ?:Help | q:Quit")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}

fn read_auth_entries(state: &AppState) -> Result<Option<AuthEntries>, auth::AuthError> {
    if state.paths.auth.exists() {
        parse_auth_file(&state.paths.auth).map(Some)
    } else {
        Ok(None)
    }
}

fn provider_auth_status(
    _state: &AppState,
    auth_entries: Option<&AuthEntries>,
    provider_id: &str,
    provider: Option<&ProviderConfig>,
) -> ProviderAuthStatus {
    if let Some(entries) = auth_entries {
        if let Some(entry) = entries.get(provider_id) {
            return ProviderAuthStatus::from_provider(provider_id, Some(entry));
        }
    }

    if let Some(var_name) = provider_env_reference(provider) {
        return ProviderAuthStatus::EnvVar { var_name };
    }

    ProviderAuthStatus::from_provider(provider_id, None)
}

fn provider_env_reference(provider: Option<&ProviderConfig>) -> Option<String> {
    let options = provider?.options.as_ref()?;
    for key in ["apiKey", "apikey", "key"] {
        if let Some(var_name) = options
            .get(key)
            .and_then(|value| value.as_str())
            .and_then(|value| value.strip_prefix("{env:"))
            .and_then(|rest| rest.strip_suffix('}'))
        {
            return Some(var_name.to_string());
        }
    }
    None
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

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" Provider: {provider_id}"),
        Style::default().fg(colors::PRIMARY),
    )));
    lines.push(Line::from(""));

    if app.models_loading {
        lines.push(Line::from(Span::styled(
            " Fetching models from models.dev...",
            Style::default().fg(colors::DIM),
        )));
    } else if app.discovered_models.is_empty() {
        lines.push(Line::from(Span::styled(
            " No models found. Press Esc to go back.",
            Style::default().fg(colors::DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                " Found {} models. Press Enter to add to provider.",
                app.discovered_models.len()
            ),
            Style::default().fg(colors::DIM),
        )));
        lines.push(Line::from(""));

        for (i, model) in app.discovered_models.iter().enumerate() {
            let style = if i == app.selected_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let ctx = model
                .context_length
                .map(|c| format!("{c}"))
                .unwrap_or_else(|| "-".to_string());

            let cost = match (model.input_cost_per_million, model.output_cost_per_million) {
                (Some(inp), Some(out)) => format!("${inp:.2}/${out:.2}/M"),
                _ => "-".to_string(),
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {:2} ", i + 1), Style::default().fg(colors::DIM)),
                Span::styled(model.id.clone(), style),
                Span::styled(
                    format!("  (ctx: {ctx}, cost: {cost})"),
                    Style::default().fg(colors::DIM),
                ),
            ]));
        }
    }

    let model_view = Paragraph::new(lines)
        .block(Block::bordered().title("opm - Model Selector"))
        .wrap(Wrap { trim: false })
        .scroll((app.selected_index.saturating_sub(5) as u16, 0));
    frame.render_widget(model_view, main_area);

    frame.render_widget(
        Paragraph::new(" Enter:Add model | j/k:Nav | r:Refresh models | p:Providers | q:Quit")
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
        state
            .paths
            .project
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "(not found)".to_string()),
        state
            .paths
            .project
            .as_ref()
            .map(|p| p.exists())
            .unwrap_or(false),
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
        Line::from(vec![
            Span::styled("  s        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Save config"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Delete selected provider"),
        ]),
        Line::from(vec![
            Span::styled("  n        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Add new provider"),
        ]),
        Line::from(vec![
            Span::styled("  r        ", Style::default().fg(colors::PRIMARY)),
            Span::raw("Refresh configs from disk"),
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

/// Render a confirmation dialog for provider deletion.
pub fn render_confirm_delete(frame: &mut ratatui::Frame, provider_id: &str) {
    let size = frame.area();
    let dialog_width = 50.min(size.width.saturating_sub(4));
    let dialog_height = 5;
    let x = (size.width.saturating_sub(dialog_width)) / 2;
    let y = (size.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = ratatui::layout::Rect::new(x, y, dialog_width, dialog_height);

    let dialog_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" Delete provider \"{}\"?", provider_id),
            Style::default().fg(colors::ERROR),
        )),
        Line::from(""),
        Line::from(Span::raw(" y: Confirm   n: Cancel")),
    ];

    let dialog = Paragraph::new(dialog_text)
        .block(
            Block::bordered()
                .title("Confirm Delete")
                .border_style(Style::default().fg(colors::ERROR)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(dialog, dialog_area);
}

/// Render a confirmation dialog for refreshing with unsaved changes.
pub fn render_confirm_refresh(frame: &mut ratatui::Frame) {
    let size = frame.area();
    let dialog_width = 54.min(size.width.saturating_sub(4));
    let dialog_height = 5;
    let x = (size.width.saturating_sub(dialog_width)) / 2;
    let y = (size.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = ratatui::layout::Rect::new(x, y, dialog_width, dialog_height);

    let dialog_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            " You have unsaved changes. Discard and refresh?",
            Style::default().fg(colors::WARNING),
        )),
        Line::from(""),
        Line::from(Span::raw(" y: Discard & Refresh   n: Cancel")),
    ];

    let dialog = Paragraph::new(dialog_text)
        .block(
            Block::bordered()
                .title("Confirm Refresh")
                .border_style(Style::default().fg(colors::WARNING)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(dialog, dialog_area);
}

/// Render an error bar at the bottom of the screen.
fn render_error_bar(frame: &mut ratatui::Frame, error_message: &Option<String>) {
    if let Some(msg) = error_message {
        let size = frame.area();
        let bar_height = 1.min(size.height);
        let bar_area = ratatui::layout::Rect::new(
            0,
            size.height.saturating_sub(bar_height + 1),
            size.width,
            bar_height,
        );

        let error_text = Paragraph::new(format!(" Error: {msg}")).style(
            Style::default()
                .fg(colors::ERROR)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(error_text, bar_area);
    }
}

/// Render the add provider form wizard.
pub fn render_add_provider(frame: &mut ratatui::Frame, form: &AddProviderForm, state: &AppState) {
    let size = frame.area();
    let dialog_width = 60.min(size.width.saturating_sub(4));
    let dialog_height = if form.show_copy_list {
        24.min(size.height.saturating_sub(2))
    } else {
        20.min(size.height.saturating_sub(2))
    };
    let x = (size.width.saturating_sub(dialog_width)) / 2;
    let y = (size.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = ratatui::layout::Rect::new(x, y, dialog_width, dialog_height);

    let mut lines: Vec<Line> = vec![Line::from("")];

    // Copy-from list overlay
    if form.show_copy_list {
        lines.push(Line::from(Span::styled(
            "  Copy from existing provider:",
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "  ↑↓:Navigate  Enter:Select  Esc:Cancel",
            Style::default().fg(colors::DIM),
        )));
        lines.push(Line::from(""));

        let sources = copy_source_list(state);
        for (i, (_id, name)) in sources.iter().enumerate() {
            let highlighted = form.copy_highlight == i;
            let marker = if highlighted { "  › " } else { "    " };
            let style = if highlighted {
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors::DIM)
            };
            lines.push(Line::from(vec![
                Span::styled(marker.to_string(), style),
                Span::styled(name.clone(), style),
            ]));
        }

        let dialog = Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(" Add Provider — Copy From ")
                    .border_style(Style::default().fg(colors::PRIMARY)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(dialog, dialog_area);
        return;
    }

    // Field 0: Provider ID (text input)
    let id_focused = form.focus == 0;
    let id_cursor = if id_focused { "▶ " } else { "  " };
    let id_cursor_char = if id_focused && form.id.is_empty() {
        "│"
    } else {
        ""
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{id_cursor}Provider ID: "), label_style(id_focused)),
        Span::styled(form.id.clone(), Style::default()),
        Span::styled(id_cursor_char, Style::default().fg(colors::PRIMARY)),
    ]));
    lines.push(Line::from(""));

    // Field 1: Display Name (text input)
    let name_focused = form.focus == 1;
    let name_cursor = if name_focused { "▶ " } else { "  " };
    let name_cursor_char = if name_focused && form.name.is_empty() {
        "│"
    } else {
        ""
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{name_cursor}Display Name: "),
            label_style(name_focused),
        ),
        Span::styled(form.name.clone(), Style::default()),
        Span::styled(name_cursor_char, Style::default().fg(colors::PRIMARY)),
    ]));
    lines.push(Line::from(""));

    // Field 2: SDK Package (selection list or custom text)
    let sdk_focused = form.focus == 2;
    let sdk_cursor = if sdk_focused { "▶ " } else { "  " };
    lines.push(Line::from(vec![Span::styled(
        format!("{sdk_cursor}SDK Package: "),
        label_style(sdk_focused),
    )]));

    if form.sdk.custom_mode {
        let cc = if sdk_focused && form.sdk.custom_text.is_empty() {
            "│"
        } else {
            ""
        };
        lines.push(Line::from(vec![
            Span::styled("    Custom: ", Style::default().fg(colors::DIM)),
            Span::styled(form.sdk.custom_text.clone(), Style::default()),
            Span::styled(cc, Style::default().fg(colors::PRIMARY)),
        ]));
    } else {
        for (i, sdk) in KNOWN_SDKS.iter().enumerate() {
            let highlighted = sdk_focused && form.sdk.highlight == i;
            let marker = if highlighted { "  › " } else { "    " };
            let style = if highlighted {
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors::DIM)
            };
            lines.push(Line::from(vec![Span::styled(
                format!("{marker}{sdk}"),
                style,
            )]));
        }
    }
    lines.push(Line::from(""));

    // Field 3: Base URL (text input)
    let url_focused = form.focus == 3;
    let url_cursor = if url_focused { "▶ " } else { "  " };
    let url_cursor_char = if url_focused && form.base_url.is_empty() {
        "│"
    } else {
        ""
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{url_cursor}Base URL (opt): "),
            label_style(url_focused),
        ),
        Span::styled(form.base_url.clone(), Style::default()),
        Span::styled(url_cursor_char, Style::default().fg(colors::PRIMARY)),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        " Tab:Next | ↑↓:Select SDK | Ctrl+C:Copy | Enter:Save | Esc:Cancel",
        Style::default().fg(colors::DIM),
    )));

    let dialog = Paragraph::new(lines)
        .block(
            Block::bordered()
                .title(" Add Provider ")
                .border_style(Style::default().fg(colors::PRIMARY)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(dialog, dialog_area);
}

/// Render the edit provider view.
pub fn render_edit_provider(frame: &mut ratatui::Frame, state: &AppState, form: &EditProviderForm) {
    let size = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [title_area, main_area, status_area] = vertical.areas(size);

    let title = Paragraph::new(format!("Edit Provider: {}", form.provider_id))
        .style(
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::bordered());
    frame.render_widget(title, title_area);

    // Show read-only info + editable fields
    let provider = state.get_provider(&form.provider_id);
    let model_count = provider
        .and_then(|p| p.models.as_ref())
        .map(|m| m.len())
        .unwrap_or(0);
    let is_disabled = provider.and_then(|p| p.disabled).unwrap_or(false);

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("  Provider ID: ", Style::default().fg(colors::DIM)),
            Span::styled(
                form.provider_id.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Models: ", Style::default().fg(colors::DIM)),
            Span::styled(format!("{model_count}"), Style::default()),
        ]),
        Line::from(vec![
            Span::styled("  Disabled: ", Style::default().fg(colors::DIM)),
            Span::styled(
                if is_disabled { "yes" } else { "no" }.to_string(),
                Style::default(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Editable fields:",
            Style::default().fg(colors::DIM),
        )),
        Line::from(""),
    ];

    // Field 0: Display Name (text input)
    {
        let name_focused = form.focus == 0;
        let name_cursor = if name_focused { "▶ " } else { "  " };
        let name_cursor_char = if name_focused && form.name.is_empty() {
            "│"
        } else {
            ""
        };
        let label_style = if name_focused {
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors::DIM)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{name_cursor}Display Name: "), label_style),
            Span::styled(form.name.clone(), Style::default()),
            Span::styled(name_cursor_char, Style::default().fg(colors::PRIMARY)),
        ]));
        lines.push(Line::from(""));
    }

    // Field 1: SDK Package (selection list or custom text)
    {
        let sdk_focused = form.focus == 1;
        let sdk_cursor = if sdk_focused { "▶ " } else { "  " };
        let label_style = if sdk_focused {
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors::DIM)
        };
        lines.push(Line::from(vec![Span::styled(
            format!("{sdk_cursor}SDK Package: "),
            label_style,
        )]));

        if form.sdk.custom_mode {
            let cc = if sdk_focused && form.sdk.custom_text.is_empty() {
                "│"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::styled("    Custom: ", Style::default().fg(colors::DIM)),
                Span::styled(form.sdk.custom_text.clone(), Style::default()),
                Span::styled(cc, Style::default().fg(colors::PRIMARY)),
            ]));
        } else {
            for (i, sdk) in KNOWN_SDKS.iter().enumerate() {
                let highlighted = sdk_focused && form.sdk.highlight == i;
                let marker = if highlighted { "  › " } else { "    " };
                let style = if highlighted {
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(colors::DIM)
                };
                lines.push(Line::from(vec![Span::styled(
                    format!("{marker}{sdk}"),
                    style,
                )]));
            }
        }
        lines.push(Line::from(""));
    }

    // Field 2: Base URL (text input)
    {
        let url_focused = form.focus == 2;
        let url_cursor = if url_focused { "▶ " } else { "  " };
        let url_cursor_char = if url_focused && form.base_url.is_empty() {
            "│"
        } else {
            ""
        };
        let label_style = if url_focused {
            Style::default()
                .fg(colors::PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors::DIM)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{url_cursor}Base URL: "), label_style),
            Span::styled(form.base_url.clone(), Style::default()),
            Span::styled(url_cursor_char, Style::default().fg(colors::PRIMARY)),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  Tab:Next | ↑↓:Select SDK | Enter:Confirm/Save | Esc:Cancel",
        Style::default().fg(colors::DIM),
    )));

    let edit_view = Paragraph::new(lines)
        .block(Block::bordered().title("opm - Edit Provider"))
        .wrap(Wrap { trim: false });
    frame.render_widget(edit_view, main_area);

    frame.render_widget(
        Paragraph::new(" Enter:Save | Tab:Next | Esc:Cancel | p:Back to list")
            .style(Style::default().fg(colors::DIM)),
        status_area,
    );
}
