//! Parser for OpenCode's auth.json file.
//!
//! Format: { "provider_id": { "type": "api", "key": "sk-..." } }
//! Also supports: { "provider_id": { "type": "oauth", "token": "..." } }

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::error::{AuthError, Result};

/// Auth entries parsed from auth.json.
pub type AuthEntries = HashMap<String, AuthEntry>;

/// A single provider's auth entry.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthEntry {
    /// Auth type (e.g., "api", "oauth").
    #[serde(rename = "type")]
    pub auth_type: String,

    /// API key or token value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// OAuth token (for OAuth-based auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Additional fields we don't explicitly model.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// Manual Debug impl that redacts sensitive fields to prevent accidental
// secret exposure in logs or error output.
impl std::fmt::Debug for AuthEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthEntry")
            .field("auth_type", &self.auth_type)
            .field("key", &self.key.as_ref().map(|_| "***"))
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("extra", &self.extra)
            .finish()
    }
}

/// Parse auth.json from a file path.
pub fn parse_auth_file(path: &Path) -> Result<AuthEntries> {
    if !path.exists() {
        return Err(AuthError::FileNotFound {
            path: path.display().to_string(),
        });
    }

    let content = std::fs::read_to_string(path)?;
    parse_auth_json(&content)
}

/// Parse auth.json from a string.
/// Supports JSONC (JSON with comments and trailing commas).
pub fn parse_auth_json(content: &str) -> Result<AuthEntries> {
    // Try direct JSON first (fast path for well-formed files)
    if let Ok(entries) = serde_json::from_str::<AuthEntries>(content) {
        return Ok(entries);
    }

    // Fall back to JSONC parsing (strips comments and trailing commas)
    let value = jsonc_parser::parse_to_value(content, &Default::default())
        .map_err(|e| AuthError::InvalidFormat(format!("JSONC parse error: {e:?}")))?;

    match value {
        Some(v) => {
            let serde_value = json_value_to_serde(&v);
            serde_json::from_value::<AuthEntries>(serde_value)
                .map_err(|e| AuthError::InvalidFormat(format!("{e}")))
        }
        None => Err(AuthError::InvalidFormat("Empty auth document".to_string())),
    }
}

/// Convert a jsonc_parser::JsonValue to a serde_json::Value.
fn json_value_to_serde(value: &jsonc_parser::JsonValue<'_>) -> serde_json::Value {
    match value {
        jsonc_parser::JsonValue::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (key, val) in obj.clone().into_iter() {
                map.insert(key, json_value_to_serde(&val));
            }
            serde_json::Value::Object(map)
        }
        jsonc_parser::JsonValue::Array(arr) => {
            let values: Vec<serde_json::Value> =
                arr.iter().map(|v| json_value_to_serde(v)).collect();
            serde_json::Value::Array(values)
        }
        jsonc_parser::JsonValue::Boolean(b) => serde_json::Value::Bool(*b),
        jsonc_parser::JsonValue::Number(n) => {
            if let Ok(i) = n.parse::<i64>() {
                serde_json::Value::Number(i.into())
            } else if let Ok(f) = n.parse::<f64>() {
                serde_json::Value::Number(
                    serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
                )
            } else {
                serde_json::Value::Number(0.into())
            }
        }
        jsonc_parser::JsonValue::String(s) => serde_json::Value::String(s.to_string()),
        jsonc_parser::JsonValue::Null => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_json_api_key() {
        let json = r#"{
            "anthropic": {
                "type": "api",
                "key": "sk-ant-api03-xxxxx"
            }
        }"#;

        let entries = parse_auth_json(json).unwrap();
        assert!(entries.contains_key("anthropic"));
        let entry = &entries["anthropic"];
        assert_eq!(entry.auth_type, "api");
        assert_eq!(entry.key.as_deref(), Some("sk-ant-api03-xxxxx"));
    }

    #[test]
    fn test_parse_auth_json_oauth() {
        let json = r#"{
            "github-copilot": {
                "type": "oauth",
                "token": "gho_xxxxx"
            }
        }"#;

        let entries = parse_auth_json(json).unwrap();
        assert!(entries.contains_key("github-copilot"));
        let entry = &entries["github-copilot"];
        assert_eq!(entry.auth_type, "oauth");
        assert_eq!(entry.token.as_deref(), Some("gho_xxxxx"));
    }

    #[test]
    fn test_parse_auth_json_multiple_providers() {
        let json = r#"{
            "anthropic": {
                "type": "api",
                "key": "sk-ant-xxx"
            },
            "openai": {
                "type": "api",
                "key": "sk-xxx"
            }
        }"#;

        let entries = parse_auth_json(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains_key("anthropic"));
        assert!(entries.contains_key("openai"));
    }

    #[test]
    fn test_parse_auth_json_empty() {
        let json = "{}";
        let entries = parse_auth_json(json).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_auth_json_with_comments() {
        let jsonc = r#"{
            // This is my Anthropic key
            "anthropic": {
                "type": "api",
                "key": "sk-ant-xxx"
            }
        }"#;

        let entries = parse_auth_json(jsonc).unwrap();
        assert!(entries.contains_key("anthropic"));
        assert_eq!(entries["anthropic"].auth_type, "api");
    }
}
