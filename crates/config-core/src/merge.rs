//! Deep merge logic for OpenCode configuration files.
//!
//! Follows OpenCode's documented precedence:
//! 1. Remote config (.well-known/opencode) — organizational defaults
//! 2. Global config (~/.config/opencode/opencode.json) — user preferences
//! 3. Custom config (OPENCODE_CONFIG env var) — custom overrides
//! 4. Project config (./opencode.json) — project-specific settings
//! 5. .opencode directories — agents, commands, plugins
//! 6. Inline config (OPENCODE_CONFIG_CONTENT env var) — runtime overrides
//! 7. Managed config files — highest priority, not user-overridable
//!
//! Merge rules (replicating DOCUMENTED behavior):
//! - For objects: deep merge (project keys override global, global keys preserved)
//! - For arrays: project replaces global
//! - For scalars: project overrides global
//! - Special handling for `provider` field: deep merge provider entries

use crate::schema::OpenCodeConfig;

/// Strategy for resolving merge conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Later sources override earlier ones (documented OpenCode behavior).
    Override,
    /// Only merge if the key doesn't already exist.
    FillMissing,
}

/// Merge multiple configs in priority order (lowest priority first).
///
/// The configs are merged in order, with later configs overriding earlier ones.
/// This follows the documented precedence order:
/// remote < global < custom < project < inline < managed
pub fn merge_configs(configs: &[OpenCodeConfig]) -> OpenCodeConfig {
    configs
        .iter()
        .fold(OpenCodeConfig::default(), |acc, config| {
            merge_two(acc, config.clone())
        })
}

/// Merge two configs with the second taking priority.
///
/// Deep merge for objects, replace for arrays and scalars.
pub fn merge_two(lower: OpenCodeConfig, higher: OpenCodeConfig) -> OpenCodeConfig {
    let mut result = lower;

    // Simple fields: higher priority overrides if set
    if higher.schema.is_some() {
        result.schema = higher.schema;
    }
    if higher.log_level.is_some() {
        result.log_level = higher.log_level;
    }
    if higher.model.is_some() {
        result.model = higher.model;
    }
    if higher.small_model.is_some() {
        result.small_model = higher.small_model;
    }
    if higher.default_agent.is_some() {
        result.default_agent = higher.default_agent;
    }
    if higher.username.is_some() {
        result.username = higher.username;
    }
    if higher.snapshot.is_some() {
        result.snapshot = higher.snapshot;
    }
    if higher.share.is_some() {
        result.share = higher.share;
    }
    if higher.autoupdate.is_some() {
        result.autoupdate = higher.autoupdate;
    }
    if higher.experimental.is_some() {
        result.experimental = higher.experimental;
    }

    // Deep merge for optional objects
    result.server = merge_option_struct(result.server, higher.server);
    result.skills = merge_option_struct(result.skills, higher.skills);
    result.watcher = merge_option_struct(result.watcher, higher.watcher);
    result.compaction = merge_option_struct(result.compaction, higher.compaction);

    // Deep merge for HashMap fields
    result.provider = merge_option_hashmap(result.provider, higher.provider);
    result.agent = merge_option_hashmap(result.agent, higher.agent);
    result.command = merge_option_hashmap(result.command, higher.command);
    result.mcp = merge_option_hashmap(result.mcp, higher.mcp);
    // For formatter, tools, and provider options - use replace semantics for values
    result.formatter = merge_option_hashmap_replace(result.formatter, higher.formatter);
    result.tools = merge_option_hashmap_replace(result.tools, higher.tools);
    result.permission = higher.permission.or(result.permission);

    // Arrays: higher priority replaces
    result.disabled_providers = higher.disabled_providers.or(result.disabled_providers);
    result.enabled_providers = higher.enabled_providers.or(result.enabled_providers);
    result.instructions = higher.instructions.or(result.instructions);
    result.plugin = higher.plugin.or(result.plugin);

    result
}

/// Deep merge two Option<T> structs. If both exist, merge fields.
fn merge_option_struct<T: Mergeable>(lower: Option<T>, higher: Option<T>) -> Option<T> {
    match (lower, higher) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(h)) => Some(h),
        (Some(l), Some(h)) => Some(l.merge(h)),
    }
}

/// Deep merge two Option<HashMap> fields. If both exist, merge entries.
fn merge_option_hashmap<K, V>(
    lower: Option<std::collections::HashMap<K, V>>,
    higher: Option<std::collections::HashMap<K, V>>,
) -> Option<std::collections::HashMap<K, V>>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone + Mergeable + std::fmt::Debug,
{
    match (lower, higher) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(h)) => Some(h),
        (Some(mut l), Some(h)) => {
            for (key, higher_val) in h {
                match l.remove(&key) {
                    Some(lower_val) => {
                        // Deep merge existing entries
                        l.insert(key, lower_val.merge(higher_val));
                    }
                    None => {
                        // New entry from higher priority
                        l.insert(key, higher_val);
                    }
                }
            }
            Some(l)
        }
    }
}

/// Replace-merge two Option<HashMap> fields. Higher priority entries replace lower ones.
/// Used for HashMaps where values don't support deep merge (FormatterConfig, bool, serde_json::Value).
fn merge_option_hashmap_replace<K, V>(
    lower: Option<std::collections::HashMap<K, V>>,
    higher: Option<std::collections::HashMap<K, V>>,
) -> Option<std::collections::HashMap<K, V>>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone + std::fmt::Debug,
{
    match (lower, higher) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(h)) => Some(h),
        (Some(mut l), Some(h)) => {
            // Higher priority entries override lower ones
            for (key, val) in h {
                l.insert(key, val);
            }
            Some(l)
        }
    }
}

/// Trait for types that support deep merge operations.
pub trait Mergeable: Sized {
    /// Merge another value into this one, with `other` taking priority on conflicts.
    fn merge(self, other: Self) -> Self;
}

// Import schema types for Mergeable impls
use crate::schema::*;

impl Mergeable for ServerConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            port: other.port.or(self.port),
            hostname: other.hostname.or(self.hostname),
            mdns: other.mdns.or(self.mdns),
            mdns_domain: other.mdns_domain.or(self.mdns_domain),
            cors: other.cors.or(self.cors),
        }
    }
}

impl Mergeable for SkillsConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            paths: other.paths.or(self.paths),
            urls: other.urls.or(self.urls),
        }
    }
}

impl Mergeable for WatcherConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            ignore: other.ignore.or(self.ignore),
        }
    }
}

impl Mergeable for CompactionConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            auto: other.auto.or(self.auto),
            prune: other.prune.or(self.prune),
            reserved: other.reserved.or(self.reserved),
        }
    }
}

impl Mergeable for ProviderConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            npm: other.npm.or(self.npm),
            name: other.name.or(self.name),
            options: merge_option_hashmap_replace(self.options, other.options),
            models: merge_option_hashmap(self.models, other.models),
            disabled: other.disabled.or(self.disabled),
        }
    }
}

impl Mergeable for ModelConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            name: other.name.or(self.name),
            id: other.id.or(self.id),
            options: merge_option_hashmap_replace(self.options, other.options),
            variants: merge_option_hashmap(self.variants, other.variants),
            limit: other.limit.or(self.limit),
            disabled: other.disabled.or(self.disabled),
        }
    }
}

impl Mergeable for AgentConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            model: other.model.or(self.model),
            variant: other.variant.or(self.variant),
            temperature: other.temperature.or(self.temperature),
            top_p: other.top_p.or(self.top_p),
            prompt: other.prompt.or(self.prompt),
            description: other.description.or(self.description),
            disable: other.disable.or(self.disable),
            mode: other.mode.or(self.mode),
            hidden: other.hidden.or(self.hidden),
            steps: other.steps.or(self.steps),
            color: other.color.or(self.color),
            options: merge_option_hashmap_replace(self.options, other.options),
            permission: other.permission.or(self.permission),
            tools: other.tools.or(self.tools),
        }
    }
}

impl Mergeable for CommandConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            template: other.template,
            description: other.description.or(self.description),
            agent: other.agent.or(self.agent),
            model: other.model.or(self.model),
            subtask: other.subtask.or(self.subtask),
        }
    }
}

impl Mergeable for McpConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            mcp_type: other.mcp_type.or(self.mcp_type),
            command: other.command.or(self.command),
            args: other.args.or(self.args),
            url: other.url.or(self.url),
            env: other.env.or(self.env),
            enabled: other.enabled.or(self.enabled),
        }
    }
}

impl Mergeable for VariantConfig {
    fn merge(self, other: Self) -> Self {
        Self {
            options: {
                let mut merged = self.options;
                for (k, v) in other.options {
                    merged.insert(k, v);
                }
                merged
            },
            disabled: other.disabled.or(self.disabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use std::collections::HashMap;

    #[test]
    fn test_merge_non_conflicting_keys() {
        let global = OpenCodeConfig {
            autoupdate: Some(AutoupdateConfig::Bool(true)),
            ..Default::default()
        };
        let project = OpenCodeConfig {
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            ..Default::default()
        };

        let merged = merge_two(global, project);
        assert!(matches!(
            merged.autoupdate,
            Some(AutoupdateConfig::Bool(true))
        ));
        assert_eq!(
            merged.model,
            Some("anthropic/claude-sonnet-4-5".to_string())
        );
    }

    #[test]
    fn test_merge_conflicting_scalar_project_overrides() {
        let global = OpenCodeConfig {
            model: Some("anthropic/claude-haiku-4-5".to_string()),
            ..Default::default()
        };
        let project = OpenCodeConfig {
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            ..Default::default()
        };

        let merged = merge_two(global, project);
        assert_eq!(
            merged.model,
            Some("anthropic/claude-sonnet-4-5".to_string())
        );
    }

    #[test]
    fn test_merge_provider_deep_merge() {
        let mut global_models = HashMap::new();
        global_models.insert(
            "claude-haiku-4-5".to_string(),
            ModelConfig {
                name: Some("Claude Haiku 4.5".to_string()),
                ..Default::default()
            },
        );

        let global = OpenCodeConfig {
            provider: Some({
                let mut providers = HashMap::new();
                providers.insert(
                    "anthropic".to_string(),
                    ProviderConfig {
                        options: Some({
                            let mut opts = HashMap::new();
                            opts.insert(
                                "apiKey".to_string(),
                                serde_json::Value::String("{env:ANTHROPIC_API_KEY}".to_string()),
                            );
                            opts
                        }),
                        models: Some(global_models),
                        ..Default::default()
                    },
                );
                providers
            }),
            ..Default::default()
        };

        let mut project_models = HashMap::new();
        project_models.insert(
            "claude-sonnet-4-5".to_string(),
            ModelConfig {
                name: Some("Claude Sonnet 4.5".to_string()),
                ..Default::default()
            },
        );

        let project = OpenCodeConfig {
            provider: Some({
                let mut providers = HashMap::new();
                providers.insert(
                    "anthropic".to_string(),
                    ProviderConfig {
                        models: Some(project_models),
                        ..Default::default()
                    },
                );
                providers
            }),
            ..Default::default()
        };

        let merged = merge_two(global, project);
        let providers = merged.provider.unwrap();
        let anthropic = providers.get("anthropic").unwrap();
        // Should have both models (deep merge)
        assert!(
            anthropic
                .models
                .as_ref()
                .unwrap()
                .contains_key("claude-haiku-4-5")
        );
        assert!(
            anthropic
                .models
                .as_ref()
                .unwrap()
                .contains_key("claude-sonnet-4-5")
        );
        // Should have options from global
        assert!(anthropic.options.is_some());
    }

    #[test]
    fn test_merge_configs_priority_order() {
        let global = OpenCodeConfig {
            model: Some("global/model".to_string()),
            ..Default::default()
        };
        let project = OpenCodeConfig {
            model: Some("project/model".to_string()),
            ..Default::default()
        };

        let merged = merge_configs(&[global, project]);
        assert_eq!(merged.model, Some("project/model".to_string()));
    }

    #[test]
    fn test_merge_empty_configs() {
        let merged = merge_configs(&[]);
        assert_eq!(merged.model, None);
    }
}
