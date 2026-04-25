//! Direct Provider API queries for discovering available models.
//!
//! Supports querying provider APIs directly to list available models:
//! - OpenAI: GET /v1/models
//! - Ollama: GET /api/tags
//! - LM Studio: GET /v1/models
//! - Extensible via the `ModelDiscovery` trait

use super::error::{DiscoveryError, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

/// Trait for provider-specific model discovery.
#[async_trait]
pub trait ModelDiscovery: Send + Sync {
    /// The provider ID this discovery implementation handles.
    fn provider_id(&self) -> &str;

    /// Discover available models from the provider.
    async fn discover_models(&self, api_key: Option<&str>) -> Result<Vec<DiscoveredModel>>;
}

/// OpenAI-compatible provider model discovery.
pub struct OpenAICompatibleDiscovery {
    provider_id: String,
    base_url: String,
    client: Client,
}

impl OpenAICompatibleDiscovery {
    pub fn new(provider_id: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            base_url: base_url.into(),
            client: Client::new(),
        }
    }

    /// Create a discovery for OpenAI.
    pub fn openai() -> Self {
        Self::new("openai", "https://api.openai.com/v1")
    }

    /// Create a discovery for LM Studio.
    pub fn lmstudio() -> Self {
        Self::new("lmstudio", "http://127.0.0.1:1234/v1")
    }
}

#[async_trait]
impl ModelDiscovery for OpenAICompatibleDiscovery {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    async fn discover_models(&self, api_key: Option<&str>) -> Result<Vec<DiscoveredModel>> {
        let mut request = self
            .client
            .get(format!("{}/models", self.base_url.trim_end_matches('/')));

        if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let models_response: OpenAIModelsResponse = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        Ok(models_response
            .data
            .into_iter()
            .map(|model| {
                let name = model.id.clone();
                DiscoveredModel {
                    id: model.id,
                    name,
                    provider_id: self.provider_id.clone(),
                    context_length: None,
                    max_output_tokens: None,
                    input_cost_per_million: None,
                    output_cost_per_million: None,
                }
            })
            .collect())
    }
}

/// Ollama model discovery.
pub struct OllamaDiscovery {
    base_url: String,
    client: Client,
}

impl OllamaDiscovery {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(),
        }
    }

    pub fn default_instance() -> Self {
        Self::new("http://127.0.0.1:11434")
    }
}

#[async_trait]
impl ModelDiscovery for OllamaDiscovery {
    fn provider_id(&self) -> &str {
        "ollama"
    }

    async fn discover_models(&self, _api_key: Option<&str>) -> Result<Vec<DiscoveredModel>> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url.trim_end_matches('/')))
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let ollama_response: OllamaTagsResponse = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        Ok(ollama_response
            .models
            .into_iter()
            .map(|model| {
                let name = model.name.clone();
                DiscoveredModel {
                    id: model.name,
                    name,
                    provider_id: "ollama".to_string(),
                    context_length: None,
                    max_output_tokens: None,
                    input_cost_per_million: None,
                    output_cost_per_million: None,
                }
            })
            .collect())
    }
}

// Response type definitions

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

use super::DiscoveredModel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_discovery_creation() {
        let discovery = OpenAICompatibleDiscovery::openai();
        assert_eq!(discovery.provider_id(), "openai");
    }

    #[test]
    fn test_lmstudio_discovery_creation() {
        let discovery = OpenAICompatibleDiscovery::lmstudio();
        assert_eq!(discovery.provider_id(), "lmstudio");
    }

    #[test]
    fn test_ollama_discovery_creation() {
        let discovery = OllamaDiscovery::default_instance();
        assert_eq!(discovery.provider_id(), "ollama");
    }
}
