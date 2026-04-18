//! User actions that transform app state.

use crate::error::Result;
use crate::state::AppState;
use config_core::{ConfigLayer, OpenCodeConfig, ProviderConfig};
use std::collections::HashMap;

/// Actions the user can perform on the app state.
impl AppState {
    /// Add a new provider to the config at the specified layer.
    pub fn add_provider(
        &mut self,
        provider_id: String,
        config: ProviderConfig,
        layer: ConfigLayer,
    ) -> Result<()> {
        let target_config = self.config_for_layer_mut(layer)?;
        target_config
            .provider
            .get_or_insert_with(HashMap::new)
            .insert(provider_id.clone(), config);
        self.recompute_merged();
        self.mark_dirty();
        Ok(())
    }

    /// Remove a provider from the config at the specified layer.
    pub fn remove_provider(&mut self, provider_id: &str, layer: ConfigLayer) -> Result<()> {
        let target_config = self.config_for_layer_mut(layer)?;
        if let Some(ref mut providers) = target_config.provider {
            providers.remove(provider_id);
        }
        self.recompute_merged();
        self.mark_dirty();
        Ok(())
    }

    /// Edit a provider field in the config at the specified layer.
    pub fn edit_provider_field(
        &mut self,
        provider_id: &str,
        field: &str,
        value: serde_json::Value,
        layer: ConfigLayer,
    ) -> Result<()> {
        let target_config = self.config_for_layer_mut(layer)?;
        if let Some(ref mut providers) = target_config.provider {
            if let Some(provider) = providers.get_mut(provider_id) {
                match field {
                    "name" => {
                        if let serde_json::Value::String(s) = value {
                            provider.name = Some(s);
                        }
                    }
                    "npm" => {
                        if let serde_json::Value::String(s) = value {
                            provider.npm = Some(s);
                        }
                    }
                    _ => {
                        // Store as an option
                        provider
                            .options
                            .get_or_insert_with(HashMap::new)
                            .insert(field.to_string(), value);
                    }
                }
            }
        }
        self.recompute_merged();
        self.mark_dirty();
        Ok(())
    }

    /// Add a model to a provider.
    pub fn add_model(
        &mut self,
        provider_id: &str,
        model_id: String,
        model_config: config_core::ModelConfig,
        layer: ConfigLayer,
    ) -> Result<()> {
        let target_config = self.config_for_layer_mut(layer)?;
        if let Some(ref mut providers) = target_config.provider {
            if let Some(provider) = providers.get_mut(provider_id) {
                provider
                    .models
                    .get_or_insert_with(HashMap::new)
                    .insert(model_id, model_config);
            }
        }
        self.recompute_merged();
        self.mark_dirty();
        Ok(())
    }

    /// Remove a model from a provider.
    pub fn remove_model(
        &mut self,
        provider_id: &str,
        model_id: &str,
        layer: ConfigLayer,
    ) -> Result<()> {
        let target_config = self.config_for_layer_mut(layer)?;
        if let Some(ref mut providers) = target_config.provider {
            if let Some(provider) = providers.get_mut(provider_id) {
                if let Some(ref mut models) = provider.models {
                    models.remove(model_id);
                }
            }
        }
        self.recompute_merged();
        self.mark_dirty();
        Ok(())
    }

    /// Save the config at the specified layer to disk.
    pub fn save(&mut self, layer: ConfigLayer) -> Result<()> {
        let config = self.config_for_layer(layer)?;
        let path = self.paths.path_for_layer(layer).ok_or_else(|| {
            crate::error::AppError::State(format!("No config path for layer {:?}", layer))
        })?;

        config_core::jsonc::write_config(config, path)?;
        self.dirty = false;
        Ok(())
    }

    // --- Private helpers ---

    fn config_for_layer(&self, layer: ConfigLayer) -> Result<&OpenCodeConfig> {
        match layer {
            ConfigLayer::Global => self.global_config.as_ref().ok_or_else(|| {
                crate::error::AppError::State("No global config loaded".to_string())
            }),
            ConfigLayer::Project => self.project_config.as_ref().ok_or_else(|| {
                crate::error::AppError::State("No project config loaded".to_string())
            }),
            ConfigLayer::Custom => self.global_config.as_ref().ok_or_else(|| {
                crate::error::AppError::State("Custom config not available".to_string())
            }),
        }
    }

    fn config_for_layer_mut(&mut self, layer: ConfigLayer) -> Result<&mut OpenCodeConfig> {
        match layer {
            ConfigLayer::Global => {
                if self.global_config.is_none() {
                    self.global_config = Some(OpenCodeConfig::default());
                }
                Ok(self.global_config.as_mut().unwrap())
            }
            ConfigLayer::Project => {
                if self.project_config.is_none() {
                    self.project_config = Some(OpenCodeConfig::default());
                }
                Ok(self.project_config.as_mut().unwrap())
            }
            ConfigLayer::Custom => Err(crate::error::AppError::State(
                "Cannot edit custom config directly".to_string(),
            )),
        }
    }

    pub fn recompute_merged(&mut self) {
        let mut configs = Vec::new();
        if let Some(global) = &self.global_config {
            configs.push(global.clone());
        }
        if let Some(project) = &self.project_config {
            configs.push(project.clone());
        }
        self.merged_config = config_core::merge_configs(&configs);
    }
}
