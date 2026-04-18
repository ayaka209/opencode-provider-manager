//! GUI (Graphical User Interface) for OpenCode Provider Manager using egui.
//!
//! This is a feature-gated GUI that shares the same app logic as the TUI.

use anyhow::Result;

/// GUI application state.
pub struct GuiApp {
    // TODO: Share app state with TUI via the `app` crate
}

impl GuiApp {
    /// Create a new GUI app instance.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("OpenCode Provider Manager");
            ui.label("GUI - Work in Progress");
            ui.add_space(10.0);
            ui.label("Use the TUI (opm) for full functionality.");
        });
    }
}

fn main() -> Result<()> {
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
