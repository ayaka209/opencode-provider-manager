//! config-core: Core configuration file read/write/validate/merge for OpenCode.
//!
//! This crate handles:
//! - Parsing and serializing `opencode.json` / `opencode.jsonc` files
//! - Deep merging global and project-level configs
//! - Validating config against the OpenCode JSON schema
//! - JSONC comment preservation
//! - Platform-aware config path resolution
//! - Environment variable substitution `{env:VAR}` and file substitution `{file:path}`

pub mod error;
pub mod jsonc;
pub mod merge;
pub mod paths;
pub mod schema;
pub mod validate;

pub use error::{ConfigError, Result};
pub use jsonc::JsoncHandler;
pub use merge::{MergeStrategy, merge_configs, merge_two};
pub use paths::{ConfigLayer, ConfigPaths};
pub use schema::*;
pub use validate::validate_config;
