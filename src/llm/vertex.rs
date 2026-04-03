// Fin + Google Vertex AI Provider (Claude via Vertex)
//
// Uses Google Application Default Credentials (ADC) for auth.
// SSE streaming with same event format as the Anthropic API.
// Set: GOOGLE_CLOUD_PROJECT, CLOUD_ML_REGION (default: us-east5)

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

const ANTHROPIC_VERSION: &str = "vertex-2023-10-16";

pub struct VertexProvider {
    client: reqwest::Client,
}

impl VertexProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Resolve the Vertex endpoint URL from env vars and model config.
    fn endpoint_url(model_id: &str) -> anyhow::Result<String> {
        let project = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("CLOUDSDK_CORE_PROJECT"))
            .map_err(|_| {
                anyhow::anyhow!("GOOGLE_CLOUD_PROJECT not set. Required for Vertex AI.")
            })?;

        let region = std::env::var("CLOUD_ML_REGION").unwrap_or_else(|_| "us-east5".to_string());

        Ok(format!(
            "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/anthropic/models/{model_id}:streamRawPredict"
        ))
    }

    /// Get a Bearer token via gcloud CLI (Application Default Credentials).
    async fn get_access_token() -> anyhow::Result<String> {
        // First check for explicit env var
        if let Ok(token) = std::env::var("VERTEX_ACCESS_TOKEN") {
            return Ok(token);
        }

        // Try gcloud CLI
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .await
            .map_err(|_| anyhow::anyhow!(
                "Failed to run `gcloud auth print-access-token`. Install gcloud CLI or set VERTEX_ACCESS_TOKEN."
            ))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gcloud auth failed: {stderr}");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[async_trait]
impl LlmProvider for VertexProvider {
    fn name(&self) -> &str {
        "vertex"
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>> {
        let token = Self::get_access_token().await?;
        let url = Self::endpoint_url(&model.id)?;

        let messages =
            crate::llm::anthropic::AnthropicProvider::convert_messages(&context.messages);

        let mut body = serde_json::json!({
            "anthropic_version": ANTHROPIC_VERSION,
            "max_tokens": options.max_tokens,
            "stream": true,
            "messages": messages,
        });

        if !context.system_prompt.is_empty() {
            body["system"] = serde_json::json!(context.system_prompt);
        }

        if !context.tools.is_empty() {
            body["tools"] = serde_json::json!(
                crate::llm::anthropic::AnthropicProvider::convert_tools(&context.tools)
            );
        }

        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Thinking support
        let use_thinking =
            model.capabilities.thinking && !matches!(options.thinking_level, ThinkingLevel::Off);
        if use_thinking {
            let budget = match options.thinking_level {
                ThinkingLevel::Minimal => 1024,
                ThinkingLevel::Low => 2048,
                ThinkingLevel::Medium => 8192,
                ThinkingLevel::High => 16384,
                ThinkingLevel::XHigh => 32768,
                ThinkingLevel::Off => 0,
            };
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget
            });
            if let Some(max) = body["max_tokens"].as_u64() {
                if max < (budget as u64 + 1024) {
                    body["max_tokens"] = serde_json::json!(budget + 4096);
                }
            }
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::LlmError::ApiError {
                provider: "vertex".into(),
                status,
                body,
            }
            .into());
        }

        // Vertex uses the same SSE format as Anthropic — reuse the parser
        let byte_stream = response.bytes_stream();
        let event_stream = crate::llm::anthropic::parse_anthropic_sse(byte_stream);

        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, model: &str) -> bool {
        model.contains("opus") || model.contains("sonnet")
    }

    fn supports_images(&self, _model: &str) -> bool {
        true
    }
}
