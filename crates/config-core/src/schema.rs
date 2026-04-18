//! Config schema types matching the OpenCode JSON schema.
//!
//! Reference: https://opencode.ai/config.json
//! These types are designed for serde serialization/deserialization
//! and support partial configs (all fields optional) for merge operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration for `opencode.json`.
///
/// All fields are optional to support partial configs from different layers
/// (global, project, custom) that are merged together.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeConfig {
    /// JSON schema reference for validation.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// Log level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,

    /// Server configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,

    /// Custom commands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<HashMap<String, CommandConfig>>,

    /// Additional skill folder paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsConfig>,

    /// File watcher ignore patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watcher: Option<WatcherConfig>,

    /// Enable or disable snapshot tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Plugin list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<Vec<PluginEntry>>,

    /// Sharing behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share: Option<ShareMode>,

    /// Auto-update behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoupdate: Option<AutoupdateConfig>,

    /// Disabled provider IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_providers: Option<Vec<String>>,

    /// Enabled provider IDs (allowlist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_providers: Option<Vec<String>>,

    /// Default model in `provider/model` format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Small model for lightweight tasks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,

    /// Default agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,

    /// Custom username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Agent configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<HashMap<String, AgentConfig>>,

    /// Provider configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<HashMap<String, ProviderConfig>>,

    /// MCP server configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<HashMap<String, McpConfig>>,

    /// Tool permissions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionConfig>,

    /// Formatter configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatter: Option<HashMap<String, FormatterConfig>>,

    /// Instruction file paths/globs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Vec<String>>,

    /// Compaction settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionConfig>,

    /// Experimental features.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,

    /// Tool enable/disable overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, bool>>,
}

/// Log levels matching OpenCode config schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdnsDomain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors: Option<Vec<String>>,
}

/// Custom command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandConfig {
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask: Option<bool>,
}

/// Skills configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
}

/// Watcher configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
}

/// Provider configuration.
///
/// This is the core type for the provider manager, representing a single
/// provider entry in `opencode.json` under the `provider` key.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// NPM package for custom providers (e.g., "@ai-sdk/openai-compatible").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,

    /// Display name for the provider in the UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Provider-specific options (baseURL, apiKey, timeout, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,

    /// Models available under this provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<HashMap<String, ModelConfig>>,

    /// Provider-specific disabled flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Model configuration within a provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// Display name for the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Model ID override (for custom inference profiles, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Model-specific options (reasoningEffort, thinking, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,

    /// Model variants with different configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<HashMap<String, VariantConfig>>,

    /// Model limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<ModelLimit>,

    /// Whether this model/variant is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Model limits for context and output tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelLimit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
}

/// Variant configuration for a model.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct VariantConfig {
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topP: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<AgentMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionConfig>,

    /// Deprecated: use `permission` field instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, bool>>,
}

/// Agent mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Subagent,
    Primary,
    All,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mcp_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Permission configuration.
///
/// Supports both simple string values ("ask", "allow", "deny") and
/// nested object rules per tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PermissionConfig {
    /// Simple permission for all tools.
    Simple(PermissionAction),
    /// Per-tool permission rules.
    Detailed(HashMap<String, PermissionRule>),
}

/// Permission action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    Ask,
    Allow,
    Deny,
}

/// Permission rule for a specific tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PermissionRule {
    /// Simple action for all sub-operations.
    Simple(PermissionAction),
    /// Per-pattern rules (e.g., bash: { "rm -rf *": "deny", "*": "ask" }).
    Detailed(HashMap<String, PermissionAction>),
}

/// Formatter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FormatterConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
}

/// Compaction configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<u64>,
}

/// Share mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShareMode {
    Manual,
    Auto,
    Disabled,
}

/// Autoupdate configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AutoupdateConfig {
    Bool(bool),
    Notify(String),
}

/// Plugin entry - can be a string or [string, options] tuple.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PluginEntry {
    Name(String),
    WithOptions(Vec<serde_json::Value>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_deserialize() {
        let json = r#"{
            "npm": "@ai-sdk/openai-compatible",
            "name": "My Custom Provider",
            "options": {
                "baseURL": "http://127.0.0.1:1234/v1"
            },
            "models": {
                "gpt-4o": {
                    "name": "GPT-4o"
                }
            }
        }"#;

        let config: ProviderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.npm.as_deref(), Some("@ai-sdk/openai-compatible"));
        assert_eq!(config.name.as_deref(), Some("My Custom Provider"));
        assert!(config.models.is_some());
        assert!(config.options.is_some());
    }

    #[test]
    fn test_opencode_config_deserialize_minimal() {
        let json = r#"{
            "$schema": "https://opencode.ai/config.json",
            "model": "anthropic/claude-sonnet-4-5"
        }"#;

        let config: OpenCodeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.schema.as_deref(),
            Some("https://opencode.ai/config.json")
        );
        assert_eq!(config.model.as_deref(), Some("anthropic/claude-sonnet-4-5"));
    }

    #[test]
    fn test_opencode_config_serialize_roundtrip() {
        let json = r#"{
            "$schema": "https://opencode.ai/config.json",
            "provider": {
                "anthropic": {
                    "options": {
                        "apiKey": "{env:ANTHROPIC_API_KEY}"
                    }
                }
            },
            "model": "anthropic/claude-sonnet-4-5",
            "smallModel": "anthropic/claude-haiku-4-5",
            "autoupdate": true
        }"#;

        let config: OpenCodeConfig = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: OpenCodeConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_log_level_deserialize() {
        let json = r#""WARN""#;
        let level: LogLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level, LogLevel::Warn);
    }

    #[test]
    fn test_share_mode_deserialize() {
        assert_eq!(
            serde_json::from_str::<ShareMode>(r#""manual""#).unwrap(),
            ShareMode::Manual
        );
        assert_eq!(
            serde_json::from_str::<ShareMode>(r#""disabled""#).unwrap(),
            ShareMode::Disabled
        );
    }

    #[test]
    fn test_plugin_entry_variants() {
        let name: PluginEntry = serde_json::from_str(r#""my-plugin""#).unwrap();
        assert!(matches!(name, PluginEntry::Name(_)));

        let with_opts: PluginEntry =
            serde_json::from_str(r#"["my-plugin", {"key": "value"}]"#).unwrap();
        assert!(matches!(with_opts, PluginEntry::WithOptions(_)));
    }

    #[test]
    fn test_autoupdate_config_variants() {
        let bool_val: AutoupdateConfig = serde_json::from_str(r#"true"#).unwrap();
        assert!(matches!(bool_val, AutoupdateConfig::Bool(true)));

        let notify_val: AutoupdateConfig = serde_json::from_str(r#""notify""#).unwrap();
        assert!(matches!(notify_val, AutoupdateConfig::Notify(_)));
    }
}
