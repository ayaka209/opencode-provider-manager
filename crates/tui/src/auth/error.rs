use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Auth file not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid auth format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, AuthError>;
