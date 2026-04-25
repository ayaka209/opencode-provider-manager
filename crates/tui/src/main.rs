//! TUI (Terminal User Interface) for OpenCode Provider Manager.

use anyhow::{Context, Result};
use app::import::{ImportMergeMode, import_source};
use app::state::AppState;
use clap::{Parser, Subcommand};
use config_core::{ConfigLayer, OpenCodeConfig};
use opencode_provider_manager::{app, config_core};
use serde::Serialize;
use std::path::PathBuf;
use std::process;

mod event;
mod tui_app;
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

    /// Import JSON/JSONC/TOML/YAML config/provider/model snippets from a file, directory, URL, or inline text.
    Import {
        /// File path, directory path, GitHub URL, raw URL, or inline snippet to import.
        #[arg(long, value_name = "SOURCE")]
        input: String,

        /// Target config layer to modify.
        #[arg(long, value_name = "LAYER", default_value = "project")]
        layer: String,

        /// Merge imported config into the target layer, or replace that layer.
        #[arg(long, value_name = "MODE", default_value = "merge")]
        mode: String,

        /// Provider ID hint for provider/model fragments that do not contain an ID.
        #[arg(long, value_name = "ID")]
        provider_id: Option<String>,

        /// Preview import summary without saving.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Provider info for JSON output.
#[derive(Serialize)]
struct ProviderInfo {
    id: String,
    name: Option<String>,
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let global_config = args.config.clone();

    // Determine which command to run
    let result = match args.command {
        None => {
            // No subcommand - run TUI with top-level args
            run_tui_blocking(args.layer, args.config, args.split)
        }
        Some(Commands::Tui {
            layer,
            config,
            split,
        }) => {
            // Explicit TUI subcommand
            run_tui_blocking(layer, config.or(global_config), split)
        }
        Some(Commands::ListProviders { layer }) => {
            // CLI: list providers
            run_list_providers(&layer, args.config.as_deref())
        }
        Some(Commands::ShowConfig { layer }) => {
            // CLI: show config
            run_show_config(&layer, args.config.as_deref())
        }
        Some(Commands::Validate) => {
            // CLI: validate configs
            run_validate(args.config.as_deref())
        }
        Some(Commands::Import {
            input,
            layer,
            mode,
            provider_id,
            dry_run,
        }) => run_import(
            &input,
            &layer,
            &mode,
            provider_id.as_deref(),
            args.config.as_deref(),
            dry_run,
        ),
    };

    // Handle errors
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run_tui_blocking(layer: Option<String>, config: Option<String>, split: bool) -> Result<()> {
    tokio::runtime::Runtime::new()
        .context("Failed to initialize async runtime")?
        .block_on(run_tui(layer, config, split))
}

/// Run the TUI application.
async fn run_tui(layer: Option<String>, config: Option<String>, split: bool) -> Result<()> {
    // Initialize app state
    let mut state = AppState::new().context("Failed to initialize app state")?;

    apply_custom_config_path(&mut state, config.as_deref())?;

    // Apply explicit layer selection
    if let Some(ref layer_str) = layer {
        match layer_str.to_lowercase().as_str() {
            "global" => state.edit_layer = config_core::ConfigLayer::Global,
            "project" => state.edit_layer = config_core::ConfigLayer::Project,
            "custom" => state.edit_layer = config_core::ConfigLayer::Custom,
            other => {
                return Err(anyhow::anyhow!(
                    "Invalid --layer '{}'. Must be one of: global, project, custom",
                    other
                ));
            }
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

fn load_state(config: Option<&str>) -> Result<AppState> {
    let mut state = AppState::new().context("Failed to initialize app state")?;
    apply_custom_config_path(&mut state, config)?;
    state.load_configs().context("Failed to load configs")?;
    Ok(state)
}

fn parse_config_layer(layer: &str) -> Result<ConfigLayer> {
    match layer.to_lowercase().as_str() {
        "global" => Ok(ConfigLayer::Global),
        "project" => Ok(ConfigLayer::Project),
        "custom" => Ok(ConfigLayer::Custom),
        other => Err(anyhow::anyhow!(
            "Invalid layer '{}'. Must be one of: global, project, custom",
            other
        )),
    }
}

fn parse_import_mode(mode: &str) -> Result<ImportMergeMode> {
    match mode.to_lowercase().as_str() {
        "merge" => Ok(ImportMergeMode::Merge),
        "replace" => Ok(ImportMergeMode::Replace),
        other => Err(anyhow::anyhow!(
            "Invalid import mode '{}'. Must be one of: merge, replace",
            other
        )),
    }
}

fn apply_custom_config_path(state: &mut AppState, config: Option<&str>) -> Result<()> {
    let Some(path_str) = config else {
        return Ok(());
    };

    let config_path = PathBuf::from(path_str);
    let ext = config_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(ext, "json" | "jsonc") {
        return Err(anyhow::anyhow!(
            "Invalid --config path: file must have .json or .jsonc extension, got '{}'",
            path_str
        ));
    }

    let canonical = if config_path.exists() {
        config_path
            .canonicalize()
            .context("Failed to resolve config path")?
    } else if let Some(parent) = config_path.parent() {
        if parent.as_os_str().is_empty() {
            config_path.clone()
        } else if parent.exists() {
            let file_name = config_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid --config path: missing file name"))?;
            parent
                .canonicalize()
                .context("Failed to resolve config directory")?
                .join(file_name)
        } else {
            config_path.clone()
        }
    } else {
        config_path.clone()
    };

    state.paths.custom = Some(canonical);
    Ok(())
}

/// List providers as JSON.
fn run_list_providers(layer_str: &str, config: Option<&str>) -> Result<()> {
    let state = load_state(config)?;

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

/// Show config as JSON (with sensitive values redacted).
fn run_show_config(layer_str: &str, config: Option<&str>) -> Result<()> {
    let state = load_state(config)?;

    // Get the appropriate config based on layer
    let config = get_config_for_layer(&state, layer_str)
        .with_context(|| format!("Invalid layer: {}", layer_str))?;

    // Deep-clone and redact sensitive fields before serialization
    let mut redacted = config.clone();
    redact_sensitive_values(&mut redacted);

    // Output as pretty JSON
    let json =
        serde_json::to_string_pretty(&redacted).context("Failed to serialize config to JSON")?;
    println!("{}", json);

    Ok(())
}

/// Key names that should be redacted from JSON output.
const SENSITIVE_KEYS: &[&str] = &[
    "apiKey",
    "apikey",
    "key",
    "secret",
    "token",
    "password",
    "credential",
    "privateKey",
    "private_key",
    "accessToken",
    "access_token",
    "refreshToken",
    "refresh_token",
];

/// Recursively redact sensitive string values in a config.
fn redact_sensitive_values(config: &mut config_core::OpenCodeConfig) {
    if let Some(ref mut providers) = config.provider {
        for provider in providers.values_mut() {
            if let Some(ref mut options) = provider.options {
                for (key, value) in options.iter_mut() {
                    if SENSITIVE_KEYS.contains(&key.as_str()) && value.is_string() {
                        *value = serde_json::Value::String("***".to_string());
                    }
                }
            }
        }
    }
}

/// Validate config files.
fn run_validate(config: Option<&str>) -> Result<()> {
    let state = load_state(config)?;

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

    // Validate custom config if present
    if let Some(ref custom) = state.custom_config {
        if let Err(e) = config_core::validate_config(custom) {
            eprintln!("Custom config error: {}", e);
            has_errors = true;
        } else {
            println!("Custom config: OK");
        }
    } else if state.paths.custom.is_some() {
        println!("Custom config: not found");
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

fn run_import(
    input: &str,
    layer_str: &str,
    mode_str: &str,
    provider_id: Option<&str>,
    custom_config: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let mut state = load_state(custom_config)?;
    let layer = parse_config_layer(layer_str)?;
    let mode = parse_import_mode(mode_str)?;
    let summary = import_source(&mut state, input, provider_id, layer, mode)?;

    println!(
        "Imported {} provider(s), {} model(s): {}",
        summary.provider_count,
        summary.model_count,
        if summary.provider_ids.is_empty() {
            "(none)".to_string()
        } else {
            summary.provider_ids.join(", ")
        }
    );

    if dry_run {
        println!("Dry run: not saved");
        return Ok(());
    }

    state.save(layer)?;
    println!("Saved to {layer_str} layer");
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
        "custom" => state
            .custom_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Custom config not found")),
        _ => Err(anyhow::anyhow!(
            "Invalid layer '{}'. Must be one of: merged, global, project, custom",
            layer
        )),
    }
}
