use thiserror::Error;

/// Errors that can occur during configuration operations.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("JSONC parse error: {0}")]
    JsoncParse(String),

    #[error("Config file not found: {path}")]
    FileNotFound { path: String },

    #[error("Config validation error: {0}")]
    Validation(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),

    #[error("File not found for substitution: {0}")]
    FileSubstitutionNotFound(String),

    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },

    #[error("Invalid config layer: {0}")]
    InvalidLayer(String),

    #[error("Schema error: {0}")]
    Schema(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
