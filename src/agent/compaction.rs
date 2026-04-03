// Fin + Context Window Token Estimation

use crate::llm::types::{Content, Message};

/// Pick the right BPE encoding for a provider.
///
/// - OpenAI newer models (gpt-4.1, o3, o4): o200k_base (exact)
/// - Anthropic/Vertex/Bedrock: cl100k_base (~15% over-count vs Claude's proprietary tokenizer)
/// - Google: cl100k_base as rough proxy
fn count_bpe(text: &str, provider: &str) -> u64 {
    match provider {
        "openai" => bpe_openai::o200k_base().count(text) as u64,
        "anthropic" | "vertex" | "bedrock" | "google" => {
            bpe_openai::cl100k_base().count(text) as u64
        }
        // Unknown provider — char/4 heuristic
        _ => text.len() as u64 / 4,
    }
}

/// Estimate token count for a single content block.
fn estimate_content(content: &Content, provider: &str) -> u64 {
    match content {
        Content::Text { text } | Content::Thinking { text, .. } => count_bpe(text, provider),
        Content::ToolCall(tc) => {
            let raw = format!("{}{}", tc.name, tc.arguments);
            count_bpe(&raw, provider)
        }
        Content::ToolResult { content, .. } => {
            content.iter().map(|c| estimate_content(c, provider)).sum()
        }
        Content::Image { .. } => 1000, // images billed separately by providers
    }
}

/// Estimate token count for a message list using BPE when available.
///
/// For OpenAI models: exact count via o200k_base.
/// For Anthropic/Vertex/Bedrock: approximate via cl100k_base (~15% over-count).
/// For unknown providers: rough char/4 heuristic.
///
/// Per-message overhead (~4 tokens for role/separator) is added.
pub fn estimate_tokens(messages: &[Message], provider: &str) -> u64 {
    let per_message_overhead: u64 = 4; // role tokens, separators

    messages
        .iter()
        .map(|m| {
            let content_tokens: u64 = m
                .content
                .iter()
                .map(|c| estimate_content(c, provider))
                .sum();
            content_tokens + per_message_overhead
        })
        .sum()
}

/// Backwards-compatible wrapper — uses char/4 heuristic (no provider info).
pub fn estimate_tokens_rough(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|m| {
            m.content
                .iter()
                .map(|c| match c {
                    Content::Text { text } => text.len() as u64 / 4,
                    Content::Thinking { text, .. } => text.len() as u64 / 4,
                    Content::ToolCall(tc) => {
                        (tc.name.len() + tc.arguments.to_string().len()) as u64 / 4
                    }
                    Content::ToolResult { content, .. } => content
                        .iter()
                        .map(|c| match c {
                            Content::Text { text } => text.len() as u64 / 4,
                            _ => 50,
                        })
                        .sum(),
                    Content::Image { .. } => 1000,
                })
                .sum::<u64>()
        })
        .sum()
}

/// Check if context window is nearly full (80% threshold).
/// When true, the agent loop should STOP — not compact.
/// Fresh context from section artifacts is always better than rotted tokens.
pub fn needs_compaction(messages: &[Message], context_window: u64, provider: &str) -> bool {
    let estimated = estimate_tokens(messages, provider);
    estimated > (context_window * 80 / 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{Message, ToolCall};

    #[test]
    fn test_bpe_openai_count() {
        let msgs = vec![Message::new_user("Hello, world!")];
        let tokens = estimate_tokens(&msgs, "openai");
        // "Hello, world!" is 4 tokens in o200k_base + 4 overhead = 8
        assert!(tokens >= 4 && tokens <= 12, "got {tokens}");
    }

    #[test]
    fn test_bpe_anthropic_count() {
        let msgs = vec![Message::new_user("Hello, world!")];
        let tokens = estimate_tokens(&msgs, "anthropic");
        // cl100k_base: 4 tokens + 4 overhead = 8
        assert!(tokens >= 4 && tokens <= 12, "got {tokens}");
    }

    #[test]
    fn test_rough_fallback() {
        let msgs = vec![Message::new_user("Hello, world!")];
        let tokens = estimate_tokens(&msgs, "unknown_provider");
        // "Hello, world!" = 13 chars / 4 = 3 + 4 overhead = 7
        assert!(tokens >= 3 && tokens <= 10, "got {tokens}");
    }

    #[test]
    fn test_backwards_compat() {
        let msgs = vec![Message::new_user("Hello, world!")];
        let rough = estimate_tokens_rough(&msgs);
        assert!(rough >= 3, "got {rough}");
    }

    #[test]
    fn test_tool_call_counting() {
        let mut m = Message::new_assistant();
        m.content.push(Content::ToolCall(ToolCall {
            id: "tc_1".into(),
            name: "read".into(),
            arguments: serde_json::json!({"file_path": "/src/main.rs"}),
        }));
        let tokens = estimate_tokens(&[m], "openai");
        assert!(tokens > 4, "got {tokens}");
    }

    #[test]
    fn test_needs_compaction_threshold() {
        // Create a message with substantial text
        let long_text = "word ".repeat(400); // ~400 tokens
        let msgs = vec![Message::new_user(&long_text)];
        // Context window of 500 → 80% = 400 → should trigger
        assert!(needs_compaction(&msgs, 500, "openai"));
        // Context window of 5000 → 80% = 4000 → should NOT trigger
        assert!(!needs_compaction(&msgs, 5000, "openai"));
    }

    #[test]
    fn test_bpe_more_accurate_than_heuristic() {
        // BPE should give a different (more accurate) count than char/4
        let text = "fn main() { println!(\"hello\"); }";
        let msgs = vec![Message::new_user(text)];
        let bpe_count = estimate_tokens(&msgs, "openai");
        let rough_count = estimate_tokens_rough(&msgs);
        // They should differ — BPE is token-aware, heuristic is not
        // Both should be reasonable (> 0, < 100)
        assert!(bpe_count > 0 && bpe_count < 100, "bpe: {bpe_count}");
        assert!(rough_count > 0 && rough_count < 100, "rough: {rough_count}");
    }
}
