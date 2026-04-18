//! models.dev API client for fetching provider and model catalogs.
//!
//! Endpoint: https://models.dev/api.json

use crate::error::Result;
use crate::{DiscoveredModel, DiscoveredProvider};
use reqwest::Client;
use serde::Deserialize;

const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

/// Client for the models.dev API.
pub struct ModelsDevClient {
    client: Client,
    api_url: String,
}

impl ModelsDevClient {
    /// Create a new client with the default API URL.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_url: MODELS_DEV_API_URL.to_string(),
        }
    }

    /// Create a new client with a custom API URL (for testing).
    pub fn with_url(api_url: String) -> Self {
        Self {
            client: Client::new(),
            api_url,
        }
    }

    /// Fetch all providers and their models from models.dev.
    pub async fn fetch_providers(&self) -> Result<Vec<DiscoveredProvider>> {
        let response = self
            .client
            .get(&self.api_url)
            .send()
            .await
            .map_err(|e| crate::error::DiscoveryError::Network(e.to_string()))?;

        let providers: ModelsDevResponse = response
            .json()
            .await
            .map_err(|e| crate::error::DiscoveryError::Parse(e.to_string()))?;

        Ok(providers.into_providers())
    }

    /// Fetch a specific provider's models.
    pub async fn fetch_provider_models(&self, provider_id: &str) -> Result<Vec<DiscoveredModel>> {
        let providers = self.fetch_providers().await?;
        Ok(providers
            .into_iter()
            .find(|p| p.id == provider_id)
            .map(|p| p.models)
            .unwrap_or_default())
    }
}

/// Internal representation of the models.dev API response.
#[derive(Debug, Deserialize)]
struct ModelsDevResponse {
    #[serde(flatten)]
    providers: HashMap<String, ModelsDevProvider>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevProvider {
    name: String,
    #[serde(default)]
    models: HashMap<String, ModelsDevModel>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevModel {
    name: Option<String>,
    context_length: Option<u64>,
    max_output_tokens: Option<u64>,
    pricing: Option<ModelsDevPricing>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevPricing {
    prompt: Option<String>,
    completion: Option<String>,
}

use std::collections::HashMap;

impl ModelsDevResponse {
    fn into_providers(self) -> Vec<DiscoveredProvider> {
        self.providers
            .into_iter()
            .map(|(id, provider)| DiscoveredProvider {
                id: id.clone(),
                name: provider.name.clone(),
                models: provider
                    .models
                    .into_iter()
                    .map(|(model_id, model)| DiscoveredModel {
                        id: model_id,
                        name: model.name.unwrap_or_default(),
                        provider_id: id.clone(),
                        context_length: model.context_length,
                        max_output_tokens: model.max_output_tokens,
                        input_cost_per_million: model
                            .pricing
                            .as_ref()
                            .and_then(|p| p.prompt.as_ref()?.parse::<f64>().ok()),
                        output_cost_per_million: model
                            .pricing
                            .as_ref()
                            .and_then(|p| p.completion.as_ref()?.parse::<f64>().ok()),
                    })
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = ModelsDevClient::new();
        assert_eq!(client.api_url, MODELS_DEV_API_URL);
    }

    #[test]
    fn test_client_custom_url() {
        let client = ModelsDevClient::with_url("http://localhost:8080/api.json".to_string());
        assert_eq!(client.api_url, "http://localhost:8080/api.json");
    }
}