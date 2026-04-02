// Fin — Ollama / OpenAI-Compatible Local LLM Provider
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use futures::stream::Stream;
use serde::Deserialize;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::openai::{OpenAIProvider, parse_openai_sse};
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

const DEFAULT_HOST: &str = "http://localhost:11434";

pub struct OllamaProvider {
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Resolve the Ollama base URL from env or default.
    fn base_url() -> String {
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string())
    }

    /// Check if Ollama is reachable.
    pub async fn is_available(client: &reqwest::Client) -> bool {
        let url = Self::base_url();
        client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .is_ok()
    }

    /// Discover locally available models from Ollama's /api/tags endpoint.
    pub async fn discover_models(client: &reqwest::Client) -> Vec<OllamaModel> {
        let url = format!("{}/api/tags", Self::base_url());
        let resp = match client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => return Vec::new(),
        };

        let body: OllamaTagsResponse = match resp.json().await {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };

        body.models
    }

    /// Strip the ollama/ prefix if present, returning the raw model name.
    pub fn strip_prefix(model_id: &str) -> &str {
        model_id.strip_prefix("ollama/").unwrap_or(model_id)
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>> {
        let base = Self::base_url();
        let model_id = Self::strip_prefix(&model.id);

        let messages = OpenAIProvider::convert_messages(&context.system_prompt, &context.messages);

        let mut body = serde_json::json!({
            "model": model_id,
            "max_tokens": options.max_tokens,
            "stream": true,
            "messages": messages,
        });

        if !context.tools.is_empty() {
            body["tools"] = serde_json::json!(OpenAIProvider::convert_tools(&context.tools));
        }

        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Optional API key for remote OpenAI-compatible endpoints
        let auth = crate::config::auth::AuthStore::default();
        let mut request = self
            .client
            .post(format!("{base}/v1/chat/completions"))
            .header("content-type", "application/json");

        if let Some(api_key) = auth.get_api_key("ollama") {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let response = request.json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::LlmError::ApiError {
                provider: "ollama".into(),
                status,
                body,
            }
            .into());
        }

        let byte_stream = response.bytes_stream();
        let event_stream = parse_openai_sse(byte_stream, "ollama");

        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, _model: &str) -> bool {
        // Some local models support thinking (e.g. qwen3, deepseek-r1)
        // but there's no reliable way to detect this, so default to false
        false
    }

    fn supports_images(&self, _model: &str) -> bool {
        false
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub size: u64,
    #[serde(default)]
    pub details: OllamaModelDetails,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OllamaModelDetails {
    #[serde(default)]
    pub parameter_size: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub quantization_level: String,
    #[serde(default)]
    pub family: String,
}

impl OllamaModel {
    /// Convert to a ModelConfig for the registry.
    pub fn to_model_config(&self) -> ModelConfig {
        ModelConfig {
            id: format!("ollama/{}", self.name),
            provider: "ollama".into(),
            display_name: self.name.clone(),
            max_tokens: 4096,
            context_window: self.guess_context_window(),
            cost: crate::llm::models::ModelCost {
                input_per_million: 0.0,
                output_per_million: 0.0,
                cache_read_per_million: 0.0,
                cache_write_per_million: 0.0,
            },
            capabilities: crate::llm::models::ModelCapabilities {
                thinking: false,
                images: false,
                tool_use: true,
            },
        }
    }

    fn guess_context_window(&self) -> u64 {
        // Most Ollama models default to 2048-8192, but many support 128k+
        // Conservative default; users can override via model config
        match self.details.family.as_str() {
            "qwen3" | "qwen2.5" => 32_768,
            "llama4" => 131_072,
            "gemma3" => 131_072,
            "deepseek" => 65_536,
            _ => 8_192,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_prefix_with_prefix() {
        assert_eq!(OllamaProvider::strip_prefix("ollama/qwen3:8b"), "qwen3:8b");
    }

    #[test]
    fn strip_prefix_without_prefix() {
        assert_eq!(OllamaProvider::strip_prefix("qwen3:8b"), "qwen3:8b");
    }

    #[test]
    fn default_base_url() {
        // Only test when OLLAMA_HOST is not set
        if std::env::var("OLLAMA_HOST").is_err() {
            assert_eq!(OllamaProvider::base_url(), "http://localhost:11434");
        }
    }

    #[test]
    fn ollama_model_to_config() {
        let model = OllamaModel {
            name: "qwen3:8b".into(),
            size: 4_000_000_000,
            details: OllamaModelDetails {
                parameter_size: "8B".into(),
                quantization_level: "Q4_K_M".into(),
                family: "qwen3".into(),
            },
        };
        let config = model.to_model_config();
        assert_eq!(config.id, "ollama/qwen3:8b");
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.cost.input_per_million, 0.0);
        assert_eq!(config.context_window, 32_768);
    }
}
