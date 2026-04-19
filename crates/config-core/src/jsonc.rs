//! JSONC (JSON with Comments) parser and serializer.
//!
//! Handles reading and writing JSONC files while preserving comments,
//! trailing commas, and formatting, using `jsonc-parser`'s CST API.
//!
//! Strategy:
//! - Read: parse JSONC to clean JSON for serde deserialization.
//! - Write: if the destination file already has JSONC source, reconcile the
//!   new value against the existing CST node-by-node so comments and
//!   structural formatting around unchanged keys are preserved. When a key's
//!   value changes shape (scalar → object, array, etc.), the whole subtree is
//!   replaced. New destinations fall back to `serde_json::to_string_pretty`.

use crate::error::{ConfigError, Result};
use jsonc_parser::cst::{
    CstContainerNode, CstInputValue, CstNode, CstObject, CstObjectProp, CstRootNode,
};
use jsonc_parser::ParseOptions;
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

/// Serialize a config value and write it to a file.
///
/// If the destination already contains valid JSONC, the existing CST is
/// reconciled with the new value so that comments and formatting around
/// unchanged keys are preserved. Otherwise, a freshly formatted JSON
/// document is written.
pub fn write_config<T: serde::Serialize>(value: &T, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let new_value = serde_json::to_value(value)?;

    // Try comment-preserving round-trip when an existing source is present.
    if path.exists() {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if let Ok(root) = CstRootNode::parse(&existing, &ParseOptions::default()) {
                reconcile_root(&root, &new_value);
                std::fs::write(path, root.to_string())?;
                return Ok(());
            }
        }
    }

    let json_str = serde_json::to_string_pretty(&new_value)?;
    std::fs::write(path, json_str)?;
    Ok(())
}

/// Reconcile the root CST with the new serde value, preserving structural
/// formatting and comments wherever the shape still matches.
fn reconcile_root(root: &CstRootNode, new_value: &serde_json::Value) {
    match (root.value(), new_value) {
        (
            Some(CstNode::Container(CstContainerNode::Object(obj))),
            serde_json::Value::Object(map),
        ) => reconcile_object(&obj, map),
        _ => root.set_value(json_to_cst_input(new_value)),
    }
}

fn reconcile_object(obj: &CstObject, new: &serde_json::Map<String, serde_json::Value>) {
    // Snapshot existing properties (Rc clones) so we can iterate while mutating.
    let existing: Vec<(String, CstObjectProp)> = obj
        .properties()
        .into_iter()
        .filter_map(|prop| {
            let name = prop.name()?.decoded_value().ok()?;
            Some((name, prop))
        })
        .collect();

    // Update or add keys present in the new map.
    for (key, new_val) in new.iter() {
        if let Some(prop) = obj.get(key) {
            reconcile_prop(&prop, new_val);
        } else {
            obj.append(key, json_to_cst_input(new_val));
        }
    }

    // Remove keys that no longer exist in the new map.
    for (key, prop) in existing {
        if !new.contains_key(&key) {
            prop.remove();
        }
    }
}

fn reconcile_prop(prop: &CstObjectProp, new: &serde_json::Value) {
    match (prop.value(), new) {
        (
            Some(CstNode::Container(CstContainerNode::Object(obj))),
            serde_json::Value::Object(map),
        ) => reconcile_object(&obj, map),
        _ => prop.set_value(json_to_cst_input(new)),
    }
}

fn json_to_cst_input(v: &serde_json::Value) -> CstInputValue {
    match v {
        serde_json::Value::Null => CstInputValue::Null,
        serde_json::Value::Bool(b) => CstInputValue::Bool(*b),
        serde_json::Value::Number(n) => CstInputValue::Number(n.to_string()),
        serde_json::Value::String(s) => CstInputValue::String(s.clone()),
        serde_json::Value::Array(a) => {
            CstInputValue::Array(a.iter().map(json_to_cst_input).collect())
        }
        serde_json::Value::Object(o) => CstInputValue::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), json_to_cst_input(v)))
                .collect(),
        ),
    }
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

    #[test]
    fn test_write_preserves_comments_on_edit() {
        use std::io::Write;

        // Start with a JSONC file that has comments around keys that will stay
        // intact as well as around a key whose value we will change.
        let mut temp_file = NamedTempFile::new().unwrap();
        let original = "{\n  \
            // keep this comment\n  \
            \"$schema\": \"https://opencode.ai/config.json\",\n  \
            // this comment sits next to a value that changes\n  \
            \"model\": \"anthropic/claude-haiku-4-5\",\n  \
            /* trailing block */\n  \
            \"autoupdate\": true\n\
            }\n";
        temp_file.write_all(original.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        // Read, mutate a single scalar, write back.
        let mut config: crate::schema::OpenCodeConfig = read_config(temp_file.path()).unwrap();
        config.model = Some("anthropic/claude-sonnet-4-5".to_string());
        write_config(&config, temp_file.path()).unwrap();

        let after = std::fs::read_to_string(temp_file.path()).unwrap();

        // All original comments survive.
        assert!(after.contains("// keep this comment"));
        assert!(after.contains("// this comment sits next to a value that changes"));
        assert!(after.contains("/* trailing block */"));
        // The new value landed.
        assert!(after.contains("anthropic/claude-sonnet-4-5"));
        // Old value is gone.
        assert!(!after.contains("anthropic/claude-haiku-4-5"));
    }

    #[test]
    fn test_write_preserves_comments_on_added_key() {
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();
        let original = "{\n  \
            // annotation\n  \
            \"model\": \"anthropic/claude-haiku-4-5\"\n\
            }\n";
        temp_file.write_all(original.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let mut config: crate::schema::OpenCodeConfig = read_config(temp_file.path()).unwrap();
        config.small_model = Some("anthropic/claude-haiku-4-5".to_string());
        write_config(&config, temp_file.path()).unwrap();

        let after = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(after.contains("// annotation"));
        assert!(after.contains("\"smallModel\""));
    }
}
