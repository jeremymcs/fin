// Fin + LLM Provider Trait

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::types::{LlmContext, StreamEvent, StreamOptions};

/// Transport-agnostic LLM provider interface.
///
/// Each provider (Anthropic, OpenAI, Google, etc.) implements this trait
/// using raw HTTP calls — no SDKs required.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider identifier (e.g., "anthropic", "openai", "google").
    fn name(&self) -> &str;

    /// Stream a response from the LLM.
    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>>;

    /// Whether this provider supports extended thinking for the given model.
    #[allow(dead_code)]
    fn supports_thinking(&self, model: &str) -> bool;

    /// Whether this provider supports image inputs for the given model.
    #[allow(dead_code)]
    fn supports_images(&self, model: &str) -> bool;
}

/// Provider registry — resolves provider by name.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn LlmProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn LlmProvider>) {
        self.providers.push(provider);
    }

    pub fn get(&self, name: &str) -> Option<&dyn LlmProvider> {
        self.providers
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Create registry with all built-in providers.
    pub fn with_defaults(client: reqwest::Client) -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(crate::llm::anthropic::AnthropicProvider::new(
            client.clone(),
        )));
        registry.register(Box::new(crate::llm::openai::OpenAIProvider::new(
            client.clone(),
        )));
        registry.register(Box::new(crate::llm::google::GoogleProvider::new(
            client.clone(),
        )));
        registry.register(Box::new(crate::llm::vertex::VertexProvider::new(
            client.clone(),
        )));
        registry.register(Box::new(crate::llm::bedrock::BedrockProvider::new(
            client.clone(),
        )));
        registry.register(Box::new(crate::llm::ollama::OllamaProvider::new(client)));
        registry
    }
}
