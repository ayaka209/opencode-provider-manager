//! API key status detection for providers.

use crate::parser::AuthEntry;

/// The authentication status of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAuthStatus {
    /// API key is configured and has a recognized format.
    Configured {
        /// Whether the key format looks valid.
        format_valid: bool,
    },
    /// Auth is configured via environment variable reference.
    EnvVar {
        /// The environment variable name (e.g., "ANTHROPIC_API_KEY").
        var_name: String,
    },
    /// OAuth token is configured.
    OAuth,
    /// No authentication is configured for this provider.
    Missing,
}

impl ProviderAuthStatus {
    /// Check the auth status for a given provider based on its auth entry
    /// and provider configuration.
    pub fn from_provider(provider_id: &str, auth_entry: Option<&AuthEntry>) -> Self {
        match auth_entry {
            Some(entry) => {
                match entry.auth_type.as_str() {
                    "api" => {
                        if let Some(key) = &entry.key {
                            ProviderAuthStatus::Configured {
                                format_valid: is_valid_key_format(provider_id, key),
                            }
                        } else {
                            ProviderAuthStatus::Missing
                        }
                    }
                    "oauth" => ProviderAuthStatus::OAuth,
                    _other => {
                        // Unknown auth type, but something is configured
                        ProviderAuthStatus::Configured {
                            format_valid: false,
                        }
                    }
                }
            }
            None => {
                // Check if there's a well-known env var for this provider
                if let Some(env_var) = provider_env_var(provider_id) {
                    if std::env::var(env_var).is_ok() {
                        ProviderAuthStatus::EnvVar {
                            var_name: env_var.to_string(),
                        }
                    } else {
                        ProviderAuthStatus::Missing
                    }
                } else {
                    ProviderAuthStatus::Missing
                }
            }
        }
    }

    /// Check auth status using env var fallback.
    pub fn from_env_var(provider_id: &str) -> Self {
        if let Some(env_var) = provider_env_var(provider_id) {
            if std::env::var(env_var).is_ok() {
                return ProviderAuthStatus::EnvVar {
                    var_name: env_var.to_string(),
                };
            }
        }
        ProviderAuthStatus::Missing
    }
}

/// Check if an API key matches expected format patterns for a provider.
fn is_valid_key_format(provider_id: &str, key: &str) -> bool {
    match provider_id {
        "openai" => key.starts_with("sk-") && key.len() > 10,
        "anthropic" => key.starts_with("sk-ant-") && key.len() > 10,
        "google" | "gemini" => key.len() > 10,
        "deepseek" => key.len() > 10,
        "groq" => key.starts_with("gsk_") && key.len() > 10,
        "openrouter" => key.starts_with("sk-or-") && key.len() > 10,
        "xai" => key.len() > 10,
        // For custom/unknown providers, just check it's not empty
        _ => !key.is_empty(),
    }
}

/// Get the environment variable name for a provider's API key.
pub fn provider_env_var(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "google" | "gemini" => Some("GOOGLE_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        "groq" => Some("GROQ_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "xai" => Some("XAI_API_KEY"),
        "together" | "together-ai" => Some("TOGETHER_API_KEY"),
        "fireworks" | "fireworks-ai" => Some("FIREWORKS_API_KEY"),
        "cerebras" => Some("CEREBRAS_API_KEY"),
        "mistral" => Some("MISTRAL_API_KEY"),
        "perplexity" => Some("PERPLEXITY_API_KEY"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::AuthEntry;
    use std::collections::HashMap;

    #[test]
    fn test_configured_valid_key() {
        let entry = AuthEntry {
            auth_type: "api".to_string(),
            key: Some("sk-ant-api03-longkey".to_string()),
            token: None,
            extra: HashMap::new(),
        };
        let status = ProviderAuthStatus::from_provider("anthropic", Some(&entry));
        assert!(matches!(
            status,
            ProviderAuthStatus::Configured { format_valid: true }
        ));
    }

    #[test]
    fn test_configured_invalid_key_format() {
        let entry = AuthEntry {
            auth_type: "api".to_string(),
            key: Some("short".to_string()),
            token: None,
            extra: HashMap::new(),
        };
        let status = ProviderAuthStatus::from_provider("anthropic", Some(&entry));
        assert!(matches!(
            status,
            ProviderAuthStatus::Configured {
                format_valid: false
            }
        ));
    }

    #[test]
    fn test_oauth_status() {
        let entry = AuthEntry {
            auth_type: "oauth".to_string(),
            key: None,
            token: Some("gho_token".to_string()),
            extra: HashMap::new(),
        };
        let status = ProviderAuthStatus::from_provider("github-copilot", Some(&entry));
        assert_eq!(status, ProviderAuthStatus::OAuth);
    }

    #[test]
    fn test_missing_status() {
        let status = ProviderAuthStatus::from_provider("unknown-provider", None);
        assert_eq!(status, ProviderAuthStatus::Missing);
    }

    #[test]
    fn test_key_format_openai() {
        assert!(is_valid_key_format("openai", "sk-proj-abc123def456"));
        assert!(!is_valid_key_format("openai", "short"));
    }

    #[test]
    fn test_key_format_anthropic() {
        assert!(is_valid_key_format("anthropic", "sk-ant-api03-longkey"));
        assert!(!is_valid_key_format("anthropic", "sk-wrong-prefix"));
    }

    #[test]
    fn test_key_format_groq() {
        assert!(is_valid_key_format("groq", "gsk_abc123longkey"));
        assert!(!is_valid_key_format("groq", "short"));
    }
}
