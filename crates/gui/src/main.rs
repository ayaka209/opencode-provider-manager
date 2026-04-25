//! Standalone GUI for OpenCode Provider Manager using egui.
//!
//! This binary intentionally lives in the `gui` crate (`opm-gui`) so the TUI
//! binary does not link egui/eframe.

use anyhow::Result;
use app::import::{ImportMergeMode, ImportSummary, import_source, parse_import_source};
use app::state::AppState;
use config_core::ConfigLayer;
use eframe::egui;
use opencode_provider_manager::{app, config_core};

/// GUI application state.
pub struct GuiApp {
    state: Result<AppState, String>,
    selected_layer: ConfigLayer,
    import_mode: ImportMergeMode,
    import_input: String,
    provider_id_hint: String,
    status: String,
    preview: Option<ImportSummary>,
}

impl GuiApp {
    /// Create a new GUI app instance.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut state = AppState::new().map_err(|e| e.to_string());
        if let Ok(state) = &mut state {
            if let Err(e) = state.load_configs() {
                return Self {
                    state: Err(e.to_string()),
                    selected_layer: ConfigLayer::Project,
                    import_mode: ImportMergeMode::Merge,
                    import_input: String::new(),
                    provider_id_hint: String::new(),
                    status: "Failed to load configs".to_string(),
                    preview: None,
                };
            }
        }

        Self {
            state,
            selected_layer: ConfigLayer::Project,
            import_mode: ImportMergeMode::Merge,
            import_input: String::new(),
            provider_id_hint: String::new(),
            status: "Ready".to_string(),
            preview: None,
        }
    }

    fn reload(&mut self) {
        match &mut self.state {
            Ok(state) => match state.load_configs() {
                Ok(()) => {
                    self.status = "Reloaded configs from disk".to_string();
                    self.preview = None;
                }
                Err(e) => self.status = format!("Reload failed: {e}"),
            },
            Err(e) => self.status = format!("App state unavailable: {e}"),
        }
    }

    fn save(&mut self) {
        match &mut self.state {
            Ok(state) => match state.save(self.selected_layer) {
                Ok(()) => self.status = format!("Saved {} layer", layer_name(self.selected_layer)),
                Err(e) => self.status = format!("Save failed: {e}"),
            },
            Err(e) => self.status = format!("App state unavailable: {e}"),
        }
    }

    fn preview_import(&mut self) {
        let provider_hint = non_empty(&self.provider_id_hint);
        match parse_import_source(&self.import_input, provider_hint) {
            Ok(config) => {
                let summary = ImportSummary::from_config(&config);
                self.status = import_summary_text("Preview", &summary);
                self.preview = Some(summary);
            }
            Err(e) => {
                self.status = format!("Preview failed: {e}");
                self.preview = None;
            }
        }
    }

    fn import_now(&mut self) {
        let provider_hint = non_empty(&self.provider_id_hint);
        match &mut self.state {
            Ok(state) => match import_source(
                state,
                &self.import_input,
                provider_hint,
                self.selected_layer,
                self.import_mode,
            ) {
                Ok(summary) => {
                    self.status = import_summary_text("Imported", &summary);
                    self.preview = Some(summary);
                }
                Err(e) => {
                    self.status = format!("Import failed: {e}");
                    self.preview = None;
                }
            },
            Err(e) => self.status = format!("App state unavailable: {e}"),
        }
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("OpenCode Provider Manager");
            ui.separator();
            ui.label("Standalone GUI binary: opm-gui");
            if ui.button("Refresh").clicked() {
                self.reload();
            }
            if ui.button("Save selected layer").clicked() {
                self.save();
            }
        });
    }

    fn render_layer_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Target layer:");
            ui.radio_value(&mut self.selected_layer, ConfigLayer::Global, "Global");
            ui.radio_value(&mut self.selected_layer, ConfigLayer::Project, "Project");
            ui.radio_value(&mut self.selected_layer, ConfigLayer::Custom, "Custom");
            ui.separator();
            ui.label("Import mode:");
            ui.radio_value(&mut self.import_mode, ImportMergeMode::Merge, "Merge");
            ui.radio_value(&mut self.import_mode, ImportMergeMode::Replace, "Replace");
        });
    }

    fn render_provider_list(&self, ui: &mut egui::Ui) {
        ui.heading("Merged providers");
        match &self.state {
            Ok(state) => {
                let mut provider_ids = state.provider_ids();
                provider_ids.sort();
                if provider_ids.is_empty() {
                    ui.label("No providers configured.");
                    return;
                }

                egui::ScrollArea::vertical()
                    .id_salt("provider-list")
                    .max_height(280.0)
                    .show(ui, |ui| {
                        egui::Grid::new("providers-grid")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("ID");
                                ui.strong("Name");
                                ui.strong("Models");
                                ui.end_row();

                                for provider_id in provider_ids {
                                    let provider = state.get_provider(&provider_id);
                                    let name = provider
                                        .and_then(|p| p.name.as_deref())
                                        .unwrap_or("(unnamed)");
                                    let model_count = provider
                                        .and_then(|p| p.models.as_ref())
                                        .map(|m| m.len())
                                        .unwrap_or(0);

                                    ui.monospace(provider_id);
                                    ui.label(name);
                                    ui.label(model_count.to_string());
                                    ui.end_row();
                                }
                            });
                    });
            }
            Err(e) => {
                ui.colored_label(egui::Color32::RED, e);
            }
        }
    }

    fn render_import_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import JSON / TOML / YAML");
        ui.label(
            "Paste a full opencode config, a provider map, a provider fragment, a local path, \
             a directory, a raw URL, or a GitHub provider directory URL.",
        );
        ui.horizontal(|ui| {
            ui.label("Provider ID hint:");
            ui.text_edit_singleline(&mut self.provider_id_hint);
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.import_input)
                .desired_rows(14)
                .code_editor()
                .hint_text("Paste JSON/TOML/YAML or enter a file/directory/URL..."),
        );
        ui.horizontal(|ui| {
            if ui.button("Preview").clicked() {
                self.preview_import();
            }
            if ui.button("Import into selected layer").clicked() {
                self.import_now();
            }
            if ui.button("Import and save").clicked() {
                self.import_now();
                self.save();
            }
        });

        if let Some(summary) = &self.preview {
            ui.label(import_summary_text("Current preview", summary));
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.render_top_bar(ui);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let color = if self.status.contains("failed") || self.status.contains("unavailable") {
                egui::Color32::RED
            } else {
                egui::Color32::GRAY
            };
            ui.colored_label(color, &self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_layer_selector(ui);
            ui.separator();
            ui.columns(2, |columns| {
                self.render_provider_list(&mut columns[0]);
                self.render_import_panel(&mut columns[1]);
            });
        });
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn layer_name(layer: ConfigLayer) -> &'static str {
    match layer {
        ConfigLayer::Global => "global",
        ConfigLayer::Project => "project",
        ConfigLayer::Custom => "custom",
    }
}

fn import_summary_text(prefix: &str, summary: &ImportSummary) -> String {
    let providers = if summary.provider_ids.is_empty() {
        "(none)".to_string()
    } else {
        summary.provider_ids.join(", ")
    };
    format!(
        "{prefix}: {} provider(s), {} model(s): {providers}",
        summary.provider_count, summary.model_count
    )
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("OpenCode Provider Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "OpenCode Provider Manager",
        options,
        Box::new(|cc| Ok(Box::new(GuiApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {e}"))?;

    Ok(())
}
