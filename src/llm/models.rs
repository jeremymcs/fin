// Fin + Model Registry

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub max_tokens: u32,
    pub context_window: u64,
    pub cost: ModelCost,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_write_per_million: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    pub thinking: bool,
    pub images: bool,
    pub tool_use: bool,
}

/// Default model registry with current pricing.
pub fn default_models() -> Vec<ModelConfig> {
    vec![
        // Anthropic
        ModelConfig {
            id: "claude-opus-4-6".into(),
            provider: "anthropic".into(),
            display_name: "Claude Opus 4.6".into(),
            max_tokens: 16384,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 15.0,
                output_per_million: 75.0,
                cache_read_per_million: 1.5,
                cache_write_per_million: 18.75,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "claude-sonnet-4-6".into(),
            provider: "anthropic".into(),
            display_name: "Claude Sonnet 4.6".into(),
            max_tokens: 16384,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.3,
                cache_write_per_million: 3.75,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "claude-haiku-4-5-20251001".into(),
            provider: "anthropic".into(),
            display_name: "Claude Haiku 4.5".into(),
            max_tokens: 8192,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cache_read_per_million: 0.08,
                cache_write_per_million: 1.0,
            },
            capabilities: ModelCapabilities {
                thinking: false,
                images: true,
                tool_use: true,
            },
        },
        // OpenAI
        ModelConfig {
            id: "gpt-4.1".into(),
            provider: "openai".into(),
            display_name: "GPT-4.1".into(),
            max_tokens: 32768,
            context_window: 1_047_576,
            cost: ModelCost {
                input_per_million: 2.0,
                output_per_million: 8.0,
                cache_read_per_million: 0.5,
                cache_write_per_million: 0.0,
            },
            capabilities: ModelCapabilities {
                thinking: false,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "o3".into(),
            provider: "openai".into(),
            display_name: "o3".into(),
            max_tokens: 100_000,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 2.0,
                output_per_million: 8.0,
                cache_read_per_million: 0.5,
                cache_write_per_million: 0.0,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        // Google
        ModelConfig {
            id: "gemini-2.5-pro".into(),
            provider: "google".into(),
            display_name: "Gemini 2.5 Pro".into(),
            max_tokens: 65536,
            context_window: 1_048_576,
            cost: ModelCost {
                input_per_million: 1.25,
                output_per_million: 10.0,
                cache_read_per_million: 0.315,
                cache_write_per_million: 0.0,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "gemini-2.5-flash".into(),
            provider: "google".into(),
            display_name: "Gemini 2.5 Flash".into(),
            max_tokens: 65536,
            context_window: 1_048_576,
            cost: ModelCost {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_read_per_million: 0.0375,
                cache_write_per_million: 0.0,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        // Vertex AI (Claude via Google Cloud)
        ModelConfig {
            id: "claude-sonnet-4@20250514".into(),
            provider: "vertex".into(),
            display_name: "Claude Sonnet 4 (Vertex)".into(),
            max_tokens: 16384,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.3,
                cache_write_per_million: 3.75,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "claude-haiku-4@20250514".into(),
            provider: "vertex".into(),
            display_name: "Claude Haiku 4 (Vertex)".into(),
            max_tokens: 8192,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cache_read_per_million: 0.08,
                cache_write_per_million: 1.0,
            },
            capabilities: ModelCapabilities {
                thinking: false,
                images: true,
                tool_use: true,
            },
        },
        // AWS Bedrock (Claude via AWS)
        ModelConfig {
            id: "anthropic.claude-sonnet-4-20250514-v1:0".into(),
            provider: "bedrock".into(),
            display_name: "Claude Sonnet 4 (Bedrock)".into(),
            max_tokens: 16384,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.3,
                cache_write_per_million: 3.75,
            },
            capabilities: ModelCapabilities {
                thinking: true,
                images: true,
                tool_use: true,
            },
        },
        ModelConfig {
            id: "anthropic.claude-haiku-4-20250514-v1:0".into(),
            provider: "bedrock".into(),
            display_name: "Claude Haiku 4 (Bedrock)".into(),
            max_tokens: 8192,
            context_window: 200_000,
            cost: ModelCost {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cache_read_per_million: 0.08,
                cache_write_per_million: 1.0,
            },
            capabilities: ModelCapabilities {
                thinking: false,
                images: true,
                tool_use: true,
            },
        },
    ]
}

/// Short aliases for common models.
/// Checked before exact/partial matching in `resolve_model()`.
pub fn model_aliases() -> &'static [(&'static str, &'static str)] {
    &[
        // Anthropic
        ("sonnet", "claude-sonnet-4-6"),
        ("opus", "claude-opus-4-6"),
        ("haiku", "claude-haiku-4-5-20251001"),
        // OpenAI
        ("gpt", "gpt-4.1"),
        ("gpt4", "gpt-4.1"),
        ("o3", "o3"),
        ("codex", "o3"),
        // Google
        ("pro", "gemini-2.5-pro"),
        ("gemini", "gemini-2.5-pro"),
        ("flash", "gemini-2.5-flash"),
    ]
}

/// Resolve a model by ID, alias, or fuzzy match.
/// Resolution order: alias → exact → partial.
/// Supports `ollama/model-name` syntax for local models.
pub fn resolve_model(id: &str) -> Option<ModelConfig> {
    // Handle explicit ollama/ prefix — create a config directly
    if let Some(model_name) = id.strip_prefix("ollama/") {
        return Some(ModelConfig {
            id: id.to_string(),
            provider: "ollama".into(),
            display_name: model_name.to_string(),
            max_tokens: 4096,
            context_window: 8_192,
            cost: ModelCost {
                input_per_million: 0.0,
                output_per_million: 0.0,
                cache_read_per_million: 0.0,
                cache_write_per_million: 0.0,
            },
            capabilities: ModelCapabilities {
                thinking: false,
                images: false,
                tool_use: true,
            },
        });
    }

    let lower = id.to_lowercase();

    // Alias lookup (case-insensitive) — direct lookup to avoid recursion
    if let Some(&(_, full_id)) = model_aliases()
        .iter()
        .find(|(alias, _)| *alias == lower.as_str())
    {
        return default_models().into_iter().find(|m| m.id == full_id);
    }

    let models = default_models();

    // Exact match
    if let Some(m) = models.iter().find(|m| m.id == id) {
        return Some(m.clone());
    }

    // Partial match on id or display name
    if let Some(m) = models.into_iter().find(|m| {
        m.id.to_lowercase().contains(&lower) || m.display_name.to_lowercase().contains(&lower)
    }) {
        return Some(m);
    }

    // Ollama fallback — treat unrecognized model names as local Ollama models
    Some(ModelConfig {
        id: format!("ollama/{id}"),
        provider: "ollama".into(),
        display_name: id.to_string(),
        max_tokens: 4096,
        context_window: 8_192,
        cost: ModelCost {
            input_per_million: 0.0,
            output_per_million: 0.0,
            cache_read_per_million: 0.0,
            cache_write_per_million: 0.0,
        },
        capabilities: ModelCapabilities {
            thinking: false,
            images: false,
            tool_use: true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_models_has_all_providers() {
        let models = default_models();
        let providers: Vec<&str> = models.iter().map(|m| m.provider.as_str()).collect();
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"google"));
        assert!(providers.contains(&"vertex"));
        assert!(providers.contains(&"bedrock"));
    }

    #[test]
    fn default_models_count() {
        // 3 anthropic + 2 openai + 2 google + 2 vertex + 2 bedrock = 11
        assert!(default_models().len() >= 11);
    }

    #[test]
    fn resolve_exact_match() {
        let m = resolve_model("claude-opus-4-6").unwrap();
        assert_eq!(m.id, "claude-opus-4-6");
        assert_eq!(m.provider, "anthropic");
    }

    #[test]
    fn resolve_partial_match() {
        let m = resolve_model("opus").unwrap();
        assert_eq!(m.provider, "anthropic");
        assert!(m.id.contains("opus"));
    }

    #[test]
    fn resolve_case_insensitive() {
        let m = resolve_model("GEMINI-2.5-PRO").unwrap();
        assert_eq!(m.provider, "google");
    }

    #[test]
    fn resolve_display_name() {
        let m = resolve_model("GPT-4.1").unwrap();
        assert_eq!(m.provider, "openai");
    }

    #[test]
    fn resolve_unknown_falls_back_to_ollama() {
        let m = resolve_model("llama3.2").unwrap();
        assert_eq!(m.provider, "ollama");
        assert_eq!(m.id, "ollama/llama3.2");

        let m = resolve_model("mistral").unwrap();
        assert_eq!(m.provider, "ollama");
        assert_eq!(m.id, "ollama/mistral");
    }

    #[test]
    fn resolve_aliases() {
        assert_eq!(resolve_model("sonnet").unwrap().id, "claude-sonnet-4-6");
        assert_eq!(resolve_model("opus").unwrap().id, "claude-opus-4-6");
        assert_eq!(
            resolve_model("haiku").unwrap().id,
            "claude-haiku-4-5-20251001"
        );
        assert_eq!(resolve_model("gpt").unwrap().id, "gpt-4.1");
        assert_eq!(resolve_model("gpt4").unwrap().id, "gpt-4.1");
        assert_eq!(resolve_model("o3").unwrap().id, "o3");
        assert_eq!(resolve_model("codex").unwrap().id, "o3");
        assert_eq!(resolve_model("pro").unwrap().id, "gemini-2.5-pro");
        assert_eq!(resolve_model("gemini").unwrap().id, "gemini-2.5-pro");
        assert_eq!(resolve_model("flash").unwrap().id, "gemini-2.5-flash");
    }

    #[test]
    fn resolve_aliases_case_insensitive() {
        assert_eq!(resolve_model("SONNET").unwrap().id, "claude-sonnet-4-6");
        assert_eq!(resolve_model("Opus").unwrap().id, "claude-opus-4-6");
        assert_eq!(resolve_model("FLASH").unwrap().id, "gemini-2.5-flash");
    }

    #[test]
    fn opus_has_thinking() {
        let m = resolve_model("claude-opus-4-6").unwrap();
        assert!(m.capabilities.thinking);
        assert!(m.capabilities.tool_use);
        assert!(m.capabilities.images);
    }

    #[test]
    fn costs_are_positive() {
        for m in default_models() {
            assert!(m.cost.input_per_million > 0.0, "{} input cost", m.id);
            assert!(m.cost.output_per_million > 0.0, "{} output cost", m.id);
        }
    }

    #[test]
    fn context_windows_are_reasonable() {
        for m in default_models() {
            assert!(
                m.context_window >= 200_000,
                "{} context window too small",
                m.id
            );
        }
    }
}
