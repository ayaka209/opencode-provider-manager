//! Application state management.

use config_core::{ConfigLayer, ConfigPaths, OpenCodeConfig, ProviderConfig};

/// The current UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    /// Viewing the merged config.
    MergedView,
    /// Viewing global and project side by side.
    SplitView,
    /// Adding a new provider (wizard).
    AddProvider,
    /// Editing an existing provider.
    EditProvider { provider_id: String },
    /// Selecting models for a provider.
    ModelSelector { provider_id: String },
    /// Viewing auth status.
    AuthStatus,
    /// Importing config.
    Import,
}

/// The application state.
#[derive(Debug)]
pub struct AppState {
    /// Loaded global config.
    pub global_config: Option<OpenCodeConfig>,
    /// Loaded project config.
    pub project_config: Option<OpenCodeConfig>,
    /// Merged config (global + project).
    pub merged_config: OpenCodeConfig,
    /// Resolved config paths.
    pub paths: ConfigPaths,
    /// Current UI state.
    pub mode: AppMode,
    /// Whether any config has unsaved changes.
    pub dirty: bool,
    /// Currently selected layer for edits.
    pub edit_layer: ConfigLayer,
}

impl AppState {
    /// Create a new app state by discovering config paths.
    pub fn new() -> config_core::Result<Self> {
        let paths = ConfigPaths::discover()?;
        Ok(Self {
            global_config: None,
            project_config: None,
            merged_config: OpenCodeConfig::default(),
            paths,
            mode: AppMode::MergedView,
            dirty: false,
            edit_layer: ConfigLayer::Project,
        })
    }

    /// Load all config layers and merge them.
    pub fn load_configs(&mut self) -> config_core::Result<()> {
        // Load global config
        self.global_config = if self.paths.global.exists() {
            Some(config_core::jsonc::read_config(&self.paths.global)?)
        } else {
            None
        };

        // Load project config
        self.project_config = if let Some(ref project_path) = self.paths.project {
            if project_path.exists() {
                Some(config_core::jsonc::read_config(project_path)?)
            } else {
                None
            }
        } else {
            None
        };

        // Merge
        let mut configs_to_merge = Vec::new();
        if let Some(global) = &self.global_config {
            configs_to_merge.push(global.clone());
        }
        if let Some(project) = &self.project_config {
            configs_to_merge.push(project.clone());
        }

        self.merged_config = config_core::merge_configs(&configs_to_merge);
        self.dirty = false;

        Ok(())
    }

    /// Get a provider from the merged config.
    pub fn get_provider(&self, provider_id: &str) -> Option<&ProviderConfig> {
        self.merged_config.provider.as_ref()?.get(provider_id)
    }

    /// Get the list of configured provider IDs.
    pub fn provider_ids(&self) -> Vec<String> {
        self.merged_config
            .provider
            .as_ref()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Mark state as dirty (unsaved changes).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert!(state.is_ok());
        let state = state.unwrap();
        assert_eq!(state.mode, AppMode::MergedView);
        assert!(!state.dirty);
    }

    #[test]
    fn test_app_state_default_mode() {
        let state = AppState::new().unwrap();
        assert!(matches!(state.mode, AppMode::MergedView));
    }

    #[test]
    fn test_provider_ids_empty() {
        let state = AppState::new().unwrap();
        assert!(state.provider_ids().is_empty());
    }
}
