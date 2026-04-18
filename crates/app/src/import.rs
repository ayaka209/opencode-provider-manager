//! Config import and export functionality.

use crate::error::Result;
use crate::state::AppState;
use config_core::OpenCodeConfig;
use std::path::Path;

/// Import configuration from an external opencode.json file.
pub fn import_config(
    state: &mut AppState,
    path: &Path,
    layer: config_core::ConfigLayer,
    merge_mode: ImportMergeMode,
) -> Result<()> {
    let external_config: OpenCodeConfig = config_core::jsonc::read_config(path)?;

    match merge_mode {
        ImportMergeMode::Replace => {
            // Replace the entire config at this layer
            match layer {
                config_core::ConfigLayer::Global => {
                    state.global_config = Some(external_config);
                }
                config_core::ConfigLayer::Project => {
                    state.project_config = Some(external_config);
                }
                config_core::ConfigLayer::Custom => {
                    return Err(crate::error::AppError::State(
                        "Cannot import into custom config layer".to_string(),
                    ));
                }
            }
        }
        ImportMergeMode::Merge => {
            // Merge the external config into the existing config at this layer
            let target = match layer {
                config_core::ConfigLayer::Global => &mut state.global_config,
                config_core::ConfigLayer::Project => &mut state.project_config,
                config_core::ConfigLayer::Custom => {
                    return Err(crate::error::AppError::State(
                        "Cannot import into custom config layer".to_string(),
                    ));
                }
            };

            match target {
                Some(existing) => {
                    *target = Some(config_core::merge_two(existing.clone(), external_config));
                }
                None => {
                    *target = Some(external_config);
                }
            }
        }
    }

    // Recompute merged config
    state.recompute_merged();
    state.mark_dirty();
    Ok(())
}

/// Export current merged config to a file.
pub fn export_config(state: &AppState, path: &Path, export_scope: ExportScope) -> Result<()> {
    let config = match export_scope {
        ExportScope::Merged => &state.merged_config,
        ExportScope::Global => state
            .global_config
            .as_ref()
            .ok_or_else(|| crate::error::AppError::State("No global config".to_string()))?,
        ExportScope::Project => state
            .project_config
            .as_ref()
            .ok_or_else(|| crate::error::AppError::State("No project config".to_string()))?,
    };

    config_core::jsonc::write_config(config, path)?;
    Ok(())
}

/// How to handle conflicts during import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMergeMode {
    /// Replace the entire config at the target layer.
    Replace,
    /// Deep merge the imported config into the existing config.
    Merge,
}

/// What scope to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    /// Export the merged result.
    Merged,
    /// Export only the global config.
    Global,
    /// Export only the project config.
    Project,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_merged_config() {
        let state = AppState::new().unwrap();
        let temp_file = NamedTempFile::new().unwrap();
        export_config(&state, temp_file.path(), ExportScope::Merged).unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("{"));
    }
}
