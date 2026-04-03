// Fin + Cross-Provider Model Tier Mapping

use crate::llm::models::{ModelConfig, resolve_model};

/// Abstract capability tier independent of provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
}

impl ModelTier {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "opus" => Some(Self::Opus),
            "sonnet" => Some(Self::Sonnet),
            "haiku" => Some(Self::Haiku),
            _ => None,
        }
    }
}

/// Cross-provider tier mapping.
///
/// | Tier   | Anthropic              | OpenAI  | Google            |
/// |--------|------------------------|---------|-------------------|
/// | Opus   | claude-opus-4-6        | o3      | gemini-2.5-pro    |
/// | Sonnet | claude-sonnet-4-6      | gpt-4.1 | gemini-2.5-pro    |
/// | Haiku  | claude-haiku-4-5-*     | gpt-4.1 | gemini-2.5-flash  |
struct TierEntry {
    anthropic: &'static str,
    openai: &'static str,
    google: &'static str,
}

const TIER_MAP: [TierEntry; 3] = [
    // Opus
    TierEntry {
        anthropic: "claude-opus-4-6",
        openai: "o3",
        google: "gemini-2.5-pro",
    },
    // Sonnet
    TierEntry {
        anthropic: "claude-sonnet-4-6",
        openai: "gpt-4.1",
        google: "gemini-2.5-pro",
    },
    // Haiku
    TierEntry {
        anthropic: "claude-haiku-4-5-20251001",
        openai: "gpt-4.1",
        google: "gemini-2.5-flash",
    },
];

/// Resolve a model tier to a concrete ModelConfig.
///
/// If `preferred_provider` is given, try that provider first.
/// Falls back to Anthropic as default.
pub fn resolve_model_tier(tier_str: &str, preferred_provider: Option<&str>) -> Option<ModelConfig> {
    let tier = ModelTier::from_str(tier_str)?;
    let idx = match tier {
        ModelTier::Opus => 0,
        ModelTier::Sonnet => 1,
        ModelTier::Haiku => 2,
    };
    let entry = &TIER_MAP[idx];

    // Try preferred provider first
    if let Some(provider) = preferred_provider {
        let model_id = match provider {
            "anthropic" => entry.anthropic,
            "openai" => entry.openai,
            "google" => entry.google,
            _ => entry.anthropic,
        };
        if let Some(config) = resolve_model(model_id) {
            return Some(config);
        }
    }

    // Fallback chain: anthropic → openai → google
    resolve_model(entry.anthropic)
        .or_else(|| resolve_model(entry.openai))
        .or_else(|| resolve_model(entry.google))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_parsing() {
        assert_eq!(ModelTier::from_str("opus"), Some(ModelTier::Opus));
        assert_eq!(ModelTier::from_str("SONNET"), Some(ModelTier::Sonnet));
        assert_eq!(ModelTier::from_str("Haiku"), Some(ModelTier::Haiku));
        assert_eq!(ModelTier::from_str("unknown"), None);
    }

    #[test]
    fn test_resolve_anthropic_tier() {
        let config = resolve_model_tier("opus", Some("anthropic")).unwrap();
        assert_eq!(config.id, "claude-opus-4-6");

        let config = resolve_model_tier("sonnet", Some("anthropic")).unwrap();
        assert_eq!(config.id, "claude-sonnet-4-6");
    }

    #[test]
    fn test_resolve_openai_tier() {
        let config = resolve_model_tier("opus", Some("openai")).unwrap();
        assert_eq!(config.id, "o3");

        let config = resolve_model_tier("sonnet", Some("openai")).unwrap();
        assert_eq!(config.id, "gpt-4.1");
    }

    #[test]
    fn test_resolve_google_tier() {
        let config = resolve_model_tier("opus", Some("google")).unwrap();
        assert_eq!(config.id, "gemini-2.5-pro");
    }

    #[test]
    fn test_resolve_fallback() {
        // No preferred provider — should fall back to anthropic
        let config = resolve_model_tier("sonnet", None).unwrap();
        assert_eq!(config.provider, "anthropic");
    }
}
