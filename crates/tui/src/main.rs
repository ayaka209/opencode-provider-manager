//! TUI (Terminal User Interface) for OpenCode Provider Manager.

use anyhow::Result;
use app::state::AppState;
use clap::Parser;

mod tui_app;
mod event;
mod ui;

/// Command-line arguments.
#[derive(Parser, Debug)]
#[command(name = "opm", about = "OpenCode Provider Manager", version)]
struct Args {
    /// Start with a specific config layer view.
    #[arg(long, value_name = "LAYER")]
    layer: Option<String>,

    /// Path to a custom opencode.json config file.
    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Start in split view mode.
    #[arg(long)]
    split: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let _args = Args::parse();

    // Initialize app state
    let mut state = AppState::new()?;

    // Load configs
    state.load_configs()?;

    // Run TUI
    let terminal = ratatui::init();
    let result = tui_app::run(terminal, state).await;
    ratatui::restore();

    result
}