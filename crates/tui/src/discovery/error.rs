use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Provider API error: {0}")]
    ProviderApi(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),
}

pub type Result<T> = std::result::Result<T, DiscoveryError>;
