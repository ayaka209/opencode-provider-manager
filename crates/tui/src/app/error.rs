use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config_core::ConfigError),

    #[error("Discovery error: {0}")]
    Discovery(#[from] crate::discovery::DiscoveryError),

    #[error("Auth error: {0}")]
    Auth(#[from] crate::auth::AuthError),

    #[error("State error: {0}")]
    State(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Import error: {0}")]
    Import(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
