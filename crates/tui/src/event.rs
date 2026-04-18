//! TUI event types.

#[allow(dead_code)]
/// Events that the TUI application can handle.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Quit the application.
    Quit,

    /// Switch to a different UI mode.
    SwitchMode(crate::tui_app::AppMode),

    /// Select a provider.
    SelectProvider(String),

    /// Select a list item by index.
    SelectIndex(usize),

    /// Edit a provider field.
    EditProviderField {
        provider_id: String,
        field: String,
        value: String,
    },

    /// Add a new provider (wizard).
    AddProvider,

    /// Remove a provider.
    RemoveProvider(String),

    /// Open model selector for a provider.
    OpenModelSelector(String),

    /// Toggle a model on/off.
    ToggleModel {
        provider_id: String,
        model_id: String,
    },

    /// Save changes.
    Save,

    /// Load/refresh configs.
    Refresh,

    /// Show help.
    ShowHelp,

    /// Import config.
    ImportConfig,

    /// Export config.
    ExportConfig,

    /// Display an error message.
    Error(String),

    /// Clear the current error message.
    ClearError,
}
