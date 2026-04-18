use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Config error: {0}")]
    Config(#[from] config_core::ConfigError),

    #[error("Discovery error: {0}")]
    Discovery(#[from] discovery::DiscoveryError),

    #[error("Auth error: {0}")]
    Auth(#[from] auth::AuthError),

    #[error("State error: {0}")]
    State(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
