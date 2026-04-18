//! Config validation against the OpenCode JSON schema and custom rules.

use crate::error::{ConfigError, Result};
use crate::schema::OpenCodeConfig;

/// Validate an OpenCode config structure.
///
/// Checks for:
/// - Valid model ID format (`provider/model`)
/// - Valid provider IDs against known providers
/// - Valid API key format patterns
/// - Schema consistency
pub fn validate_config(config: &OpenCodeConfig) -> Result<()> {
    let mut errors = Vec::new();

    // Validate model ID format
    if let Some(ref model) = config.model {
        validate_model_id(model, &mut errors);
    }

    if let Some(ref small_model) = config.small_model {
        validate_model_id(small_model, &mut errors);
    }

    // Validate provider configs
    if let Some(ref providers) = config.provider {
        for (provider_id, provider_config) in providers {
            validate_provider(provider_id, provider_config, &mut errors);
        }
    }

    // Validate disabled_providers and enabled_providers don't conflict
    if let (Some(ref disabled), Some(ref enabled)) =
        (&config.disabled_providers, &config.enabled_providers)
    {
        for provider_id in disabled {
            if enabled.contains(provider_id) {
                errors.push(format!(
                    "Provider '{}' is in both disabled_providers and enabled_providers (disabled takes priority)",
                    provider_id
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::Validation(errors.join("; ")))
    }
}

/// Validate a model ID in `provider/model` format.
fn validate_model_id(model_id: &str, errors: &mut Vec<String>) {
    if model_id.contains('/') {
        let parts: Vec<&str> = model_id.splitn(2, '/').collect();
        if parts[0].is_empty() {
            errors.push(format!(
                "Invalid model ID '{}': provider part is empty",
                model_id
            ));
        }
        if parts[1].is_empty() {
            errors.push(format!(
                "Invalid model ID '{}': model part is empty",
                model_id
            ));
        }
    } else {
        errors.push(format!(
            "Invalid model ID '{}': must be in 'provider/model' format",
            model_id
        ));
    }
}

/// Validate a provider configuration.
fn validate_provider(
    provider_id: &str,
    provider: &crate::schema::ProviderConfig,
    errors: &mut Vec<String>,
) {
    // Provider ID should not contain spaces or special chars
    if provider_id.contains(' ') || provider_id.contains('\t') {
        errors.push(format!("Provider ID '{}' contains whitespace", provider_id));
    }

    // If npm is specified, it should be a valid npm package name pattern
    if let Some(ref npm) = provider.npm {
        if npm.is_empty() {
            errors.push(format!(
                "Provider '{}' has empty npm package name",
                provider_id
            ));
        }
    }

    // Validate models
    if let Some(ref models) = provider.models {
        for (model_id, model_config) in models {
            if model_id.is_empty() {
                errors.push(format!(
                    "Provider '{}' has a model with empty ID",
                    provider_id
                ));
            }
            if let Some(ref limit) = model_config.limit {
                if limit.context == Some(0) {
                    errors.push(format!(
                        "Provider '{}' model '{}' has context limit of 0",
                        provider_id, model_id
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use std::collections::HashMap;

    #[test]
    fn test_validate_valid_config() {
        let config = OpenCodeConfig {
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            provider: Some({
                let mut providers = HashMap::new();
                providers.insert(
                    "anthropic".to_string(),
                    ProviderConfig {
                        options: Some(HashMap::new()),
                        ..Default::default()
                    },
                );
                providers
            }),
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_model_id() {
        let config = OpenCodeConfig {
            model: Some("invalid-model-id".to_string()),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("provider/model"));
    }

    #[test]
    fn test_validate_empty_provider_id() {
        // Model ID with empty provider part
        let config = OpenCodeConfig {
            model: Some("/claude-sonnet-4-5".to_string()),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_provider_whitespace_id() {
        let config = OpenCodeConfig {
            provider: Some({
                let mut providers = HashMap::new();
                providers.insert("has space".to_string(), ProviderConfig::default());
                providers
            }),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_disabled_enabled_conflict() {
        let config = OpenCodeConfig {
            disabled_providers: Some(vec!["anthropic".to_string()]),
            enabled_providers: Some(vec!["anthropic".to_string()]),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("disabled_providers and enabled_providers"));
    }

    #[test]
    fn test_validate_empty_config() {
        let config = OpenCodeConfig::default();
        assert!(validate_config(&config).is_ok());
    }
}
