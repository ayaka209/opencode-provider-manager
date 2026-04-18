//! JSONC (JSON with Comments) parser and serializer.
//!
//! Handles reading and writing JSONC files while preserving comments,
//! trailing commas, and formatting.
//!
//! Note: `jsonc-parser` v0.26 does not have CST (Concrete Syntax Tree) support.
//! CST-based comment preservation will require upgrading to a newer version
//! or implementing a custom approach. For now, we parse JSONC to clean JSON
//! for deserialization, and write back as formatted JSON.

use crate::error::{ConfigError, Result};
use std::path::Path;

/// Handler for JSONC file operations.
///
/// Currently parses JSONC and strips comments for serde deserialization.
/// Full comment-preserving round-trip editing will require upgrading
/// the jsonc-parser version or implementing custom CST handling.
pub struct JsoncHandler {
    /// The original source text (for preservation when possible).
    source: String,
}

impl JsoncHandler {
    /// Parse a JSONC string.
    pub fn parse(source: &str) -> Result<Self> {
        // Validate that it's parseable JSONC
        let _ = jsonc_parser::parse_to_value(source, &Default::default())
            .map_err(|e| ConfigError::JsoncParse(format!("{e:?}")))?;

        Ok(Self {
            source: source.to_string(),
        })
    }

    /// Read and parse a JSONC file.
    pub fn read_file(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound {
            path: path.display().to_string(),
        })?;
        Self::parse(&source)
    }

    /// Get the original source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Extract clean JSON from the JSONC source (strips comments and trailing commas).
    pub fn to_json_string(&self) -> Result<String> {
        let value = jsonc_parser::parse_to_value(&self.source, &Default::default())
            .map_err(|e| ConfigError::JsoncParse(format!("{e:?}")))?;

        match value {
            Some(v) => {
                let sv = json_value_to_serde(&v)?;
                serde_json::to_string_pretty(&sv)
                    .map_err(|e| ConfigError::JsoncParse(format!("{e}")))
            }
            None => Err(ConfigError::JsoncParse("Empty JSONC document".to_string())),
        }
    }

    /// Write the JSONC content back to a file.
    /// For now, this writes the original source (preserving comments in existing files).
    pub fn write_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &self.source)?;
        Ok(())
    }
}

/// Convert jsonc_parser's JsonValue to serde_json::Value.
fn json_value_to_serde(value: &jsonc_parser::JsonValue) -> Result<serde_json::Value> {
    match value {
        jsonc_parser::JsonValue::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (key, val) in obj.clone().into_iter() {
                map.insert(key, json_value_to_serde(&val)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        jsonc_parser::JsonValue::Array(arr) => {
            let mut vec = Vec::new();
            for item in arr.iter() {
                vec.push(json_value_to_serde(item)?);
            }
            Ok(serde_json::Value::Array(vec))
        }
        jsonc_parser::JsonValue::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        jsonc_parser::JsonValue::Number(n) => {
            // Parse the number string
            if let Ok(i) = n.parse::<i64>() {
                Ok(serde_json::Value::Number(i.into()))
            } else if let Ok(f) = n.parse::<f64>() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| ConfigError::JsoncParse(format!("Invalid number: {n}")))
            } else {
                Err(ConfigError::JsoncParse(format!("Invalid number: {n}")))
            }
        }
        jsonc_parser::JsonValue::String(s) => Ok(serde_json::Value::String(s.to_string())),
        jsonc_parser::JsonValue::Null => Ok(serde_json::Value::Null),
    }
}

/// Read a config file (JSONC or JSON) and return clean JSON for deserialization.
pub fn read_config_to_json(path: &Path) -> Result<String> {
    let source = std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound {
        path: path.display().to_string(),
    })?;

    let handler = JsoncHandler::parse(&source)?;
    handler.to_json_string()
}

/// Read a config file and deserialize into the target type.
///
/// Supports both JSON and JSONC files.
pub fn read_config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let json_str = read_config_to_json(path)?;
    let value: T = serde_json::from_str(&json_str)?;
    Ok(value)
}

/// Serialize a config value and write it to a file as formatted JSON.
///
/// Note: This currently writes formatted JSON, not JSONC. Full JSONC
/// comment preservation in write operations will be enhanced later.
pub fn write_config<T: serde::Serialize>(value: &T, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json_str = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json_str)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_json() {
        let source = r#"{"model": "anthropic/claude-sonnet-4-5"}"#;
        let handler = JsoncHandler::parse(source).unwrap();
        let json = handler.to_json_string().unwrap();
        assert!(json.contains("anthropic/claude-sonnet-4-5"));
    }

    #[test]
    fn test_parse_jsonc_with_comments() {
        let source = r#"{
            // This is a comment
            "model": "anthropic/claude-sonnet-4-5",
            /* Multi-line
               comment */
            "autoupdate": true
        }"#;
        let handler = JsoncHandler::parse(source).unwrap();
        let json = handler.to_json_string().unwrap();
        // Comments should be stripped in JSON output
        assert!(!json.contains("//"));
        assert!(!json.contains("/*"));
        assert!(json.contains("anthropic/claude-sonnet-4-5"));
        assert!(json.contains("autoupdate"));
    }

    #[test]
    fn test_parse_trailing_commas() {
        let source = r#"{
            "model": "anthropic/claude-sonnet-4-5",
        }"#;
        let handler = JsoncHandler::parse(source).unwrap();
        let json = handler.to_json_string().unwrap();
        assert!(json.contains("anthropic/claude-sonnet-4-5"));
    }

    #[test]
    fn test_read_write_roundtrip() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = crate::schema::OpenCodeConfig {
            schema: Some("https://opencode.ai/config.json".to_string()),
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            autoupdate: Some(crate::schema::AutoupdateConfig::Bool(true)),
            ..Default::default()
        };

        let path = temp_file.path().to_path_buf();
        write_config(&config, &path).unwrap();

        let read_back: crate::schema::OpenCodeConfig = read_config(&path).unwrap();
        assert_eq!(
            read_back.model,
            Some("anthropic/claude-sonnet-4-5".to_string())
        );
        assert!(matches!(
            read_back.autoupdate,
            Some(crate::schema::AutoupdateConfig::Bool(true))
        ));
    }

    #[test]
    fn test_source_preservation() {
        let source = r#"{ "model": "anthropic/claude-sonnet-4-5" }"#;
        let handler = JsoncHandler::parse(source).unwrap();
        assert_eq!(handler.source(), source);
    }
}
