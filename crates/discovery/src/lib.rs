//! discovery: Model discovery from models.dev API and provider APIs.
//!
//! Supports two channels for discovering available models:
//! 1. models.dev API - fetch the full provider/model catalog
//! 2. Provider APIs - direct queries to OpenAI, Ollama, LM Studio, etc.

pub mod cache;
pub mod error;
pub mod models_dev;
pub mod provider_api;

pub use error::{DiscoveryError, Result};

/// A discovered model with metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredModel {
    /// Model ID (e.g., "gpt-4o").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Provider ID (e.g., "openai").
    pub provider_id: String,
    /// Context window size.
    pub context_length: Option<u64>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<u64>,
    /// Input cost per million tokens (USD).
    pub input_cost_per_million: Option<f64>,
    /// Output cost per million tokens (USD).
    pub output_cost_per_million: Option<f64>,
}

/// A discovered provider with its available models.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredProvider {
    /// Provider ID (e.g., "anthropic").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Available models.
    pub models: Vec<DiscoveredModel>,
}
