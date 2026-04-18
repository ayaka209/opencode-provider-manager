//! TUI (Terminal User Interface) for OpenCode Provider Manager.

use anyhow::{Context, Result};
use app::state::AppState;
use clap::{Parser, Subcommand};
use config_core::OpenCodeConfig;
use serde::Serialize;
use std::process;

mod tui_app;
mod event;
mod ui;

/// Command-line arguments.
#[derive(Parser, Debug)]
#[command(name = "opm", about = "OpenCode Provider Manager", version)]
struct Args {
    /// Subcommand to run (defaults to TUI if not specified)
    #[command(subcommand)]
    command: Option<Commands>,

    /// Start with a specific config layer view (for TUI mode).
    #[arg(long, value_name = "LAYER", global = true)]
    layer: Option<String>,

    /// Path to a custom opencode.json config file (for TUI mode).
    #[arg(long, value_name = "PATH", global = true)]
    config: Option<String>,

    /// Start in split view mode (for TUI mode).
    #[arg(long, global = true)]
    split: bool,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch the TUI (default behavior).
    Tui {
        /// Start with a specific config layer view.
        #[arg(long, value_name = "LAYER")]
        layer: Option<String>,

        /// Path to a custom opencode.json config file.
        #[arg(long, value_name = "PATH")]
        config: Option<String>,

        /// Start in split view mode.
        #[arg(long)]
    split: bool,
    },

    /// List configured providers as JSON.
    ListProviders {
        /// Which config layer to read from (merged, global, project).
        #[arg(long, value_name = "LAYER", default_value = "merged")]
        layer: String,
    },

    /// Show config as JSON.
    ShowConfig {
        /// Which config layer to show (merged, global, project).
        #[arg(long, value_name = "LAYER", default_value = "merged")]
        layer: String,
    },

    /// Validate config files.
    Validate,
}

/// Provider info for JSON output.
#[derive(Serialize)]
struct ProviderInfo {
    id: String,
    name: Option<String>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Determine which command to run
    let result = match args.command {
        None => {
            // No subcommand - run TUI with top-level args
            run_tui(args.layer, args.config, args.split).await
        }
        Some(Commands::Tui { layer, config, split }) => {
            // Explicit TUI subcommand
            run_tui(layer, config, split).await
        }
        Some(Commands::ListProviders { layer }) => {
            // CLI: list providers
            run_list_providers(&layer)
        }
        Some(Commands::ShowConfig { layer }) => {
            // CLI: show config
            run_show_config(&layer)
        }
        Some(Commands::Validate) => {
            // CLI: validate configs
            run_validate()
        }
    };

    // Handle errors
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Run the TUI application.
async fn run_tui(
    layer: Option<String>,
    _config: Option<String>,
    split: bool,
) -> Result<()> {
    // Initialize app state
    let mut state = AppState::new().context("Failed to initialize app state")?;

    // Apply layer selection
    if let Some(ref layer_str) = layer {
        match layer_str.to_lowercase().as_str() {
            "global" => state.edit_layer = config_core::ConfigLayer::Global,
            "project" => state.edit_layer = config_core::ConfigLayer::Project,
            _ => {} // ignore invalid layer
        }
    }

    // Load configs
    state.load_configs().context("Failed to load configs")?;

    // Run TUI
    let terminal = ratatui::init();
    let result = tui_app::run(terminal, state, split).await;
    ratatui::restore();

    result
}

/// List providers as JSON.
fn run_list_providers(layer_str: &str) -> Result<()> {
    // Initialize app state
    let mut state = AppState::new().context("Failed to initialize app state")?;

    // Load configs
    state.load_configs().context("Failed to load configs")?;

    // Get the appropriate config based on layer
    let config = get_config_for_layer(&state, layer_str)
        .with_context(|| format!("Invalid layer: {}", layer_str))?;

    // Build provider info list
    let providers: Vec<ProviderInfo> = config
        .provider
        .as_ref()
        .map(|providers| {
            providers
                .iter()
                .map(|(id, provider)| ProviderInfo {
                    id: id.clone(),
                    name: provider.name.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Output as JSON
    let json = serde_json::to_string_pretty(&providers)
        .context("Failed to serialize providers to JSON")?;
    println!("{}", json);

    Ok(())
}

/// Show config as JSON.
fn run_show_config(layer_str: &str) -> Result<()> {
    // Initialize app state
    let mut state = AppState::new().context("Failed to initialize app state")?;

    // Load configs
    state.load_configs().context("Failed to load configs")?;

    // Get the appropriate config based on layer
    let config = get_config_for_layer(&state, layer_str)
        .with_context(|| format!("Invalid layer: {}", layer_str))?;

    // Output as pretty JSON
    let json = serde_json::to_string_pretty(config)
        .context("Failed to serialize config to JSON")?;
    println!("{}", json);

    Ok(())
}

/// Validate config files.
fn run_validate() -> Result<()> {
    // Initialize app state
    let mut state = AppState::new().context("Failed to initialize app state")?;

    // Load configs
    state.load_configs().context("Failed to load configs")?;

    let mut has_errors = false;

    // Validate global config if present
    if let Some(ref global) = state.global_config {
        if let Err(e) = config_core::validate_config(global) {
            eprintln!("Global config error: {}", e);
            has_errors = true;
        } else {
            println!("Global config: OK");
        }
    } else {
        println!("Global config: not found");
    }

    // Validate project config if present
    if let Some(ref project) = state.project_config {
        if let Err(e) = config_core::validate_config(project) {
            eprintln!("Project config error: {}", e);
            has_errors = true;
        } else {
            println!("Project config: OK");
        }
    } else {
        println!("Project config: not found");
    }

    // Validate merged config
    if let Err(e) = config_core::validate_config(&state.merged_config) {
        eprintln!("Merged config error: {}", e);
        has_errors = true;
    } else {
        println!("Merged config: OK");
    }

    if has_errors {
        process::exit(1);
    }

    Ok(())
}

/// Get config for a specific layer.
fn get_config_for_layer<'a>(state: &'a AppState, layer: &str) -> Result<&'a OpenCodeConfig> {
    match layer.to_lowercase().as_str() {
        "merged" => Ok(&state.merged_config),
        "global" => state
            .global_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Global config not found")),
        "project" => state
            .project_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Project config not found")),
        _ => Err(anyhow::anyhow!(
            "Invalid layer '{}'. Must be one of: merged, global, project",
            layer
        )),
    }
}
