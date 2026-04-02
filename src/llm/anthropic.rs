// Fin — Anthropic Messages API Provider
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use futures::stream::Stream;
use serde::Deserialize;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

const API_BASE: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: reqwest::Client,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: API_BASE.to_string(),
        }
    }

    pub fn convert_messages(messages: &[Message]) -> Vec<serde_json::Value> {
        let mut out: Vec<serde_json::Value> = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            let msg = &messages[i];
            match msg.role {
                Role::User => {
                    let content: Vec<serde_json::Value> = msg
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            Content::Text { text } => Some(serde_json::json!({
                                "type": "text",
                                "text": text
                            })),
                            Content::Image { media_type, data } => Some(serde_json::json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": data
                                }
                            })),
                            _ => None,
                        })
                        .collect();
                    out.push(serde_json::json!({ "role": "user", "content": content }));
                    i += 1;
                }
                Role::Assistant => {
                    let content: Vec<serde_json::Value> = msg
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            Content::Text { text } => Some(serde_json::json!({
                                "type": "text",
                                "text": text
                            })),
                            Content::Thinking { text, signature } => Some(serde_json::json!({
                                "type": "thinking",
                                "thinking": text,
                                "signature": signature
                            })),
                            Content::ToolCall(tc) => Some(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.arguments
                            })),
                            _ => None,
                        })
                        .collect();
                    out.push(serde_json::json!({ "role": "assistant", "content": content }));
                    i += 1;
                }
                Role::ToolResult => {
                    // Batch ALL consecutive ToolResult messages into a single user message.
                    // The Anthropic API requires that all tool results from one assistant turn
                    // appear in a single user message — multiple consecutive user messages are
                    // invalid and return 400.
                    let mut tool_results: Vec<serde_json::Value> = Vec::new();
                    while i < messages.len() && messages[i].role == Role::ToolResult {
                        let tr = &messages[i];
                        let text = tr
                            .content
                            .iter()
                            .find_map(|c| match c {
                                Content::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tr.tool_call_id.as_deref().unwrap_or(""),
                            "content": text,
                            "is_error": tr.is_error.unwrap_or(false)
                        }));
                        i += 1;
                    }
                    out.push(serde_json::json!({ "role": "user", "content": tool_results }));
                }
            }
        }

        out
    }

    pub fn convert_tools(tools: &[ToolSchema]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>> {
        let auth = crate::config::auth::AuthStore::default();
        let api_key = auth
            .get_api_key("anthropic")
            .ok_or_else(|| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        let messages = Self::convert_messages(&context.messages);

        let mut body = serde_json::json!({
            "model": model.id,
            "max_tokens": options.max_tokens,
            "stream": true,
            "messages": messages,
        });

        if !context.system_prompt.is_empty() {
            body["system"] = serde_json::json!(context.system_prompt);
        }

        if !context.tools.is_empty() {
            body["tools"] = serde_json::json!(Self::convert_tools(&context.tools));
        }

        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Enable thinking for supported models
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
            // Thinking requires higher max_tokens
            if let Some(max) = body["max_tokens"].as_u64() {
                if max < (budget as u64 + 1024) {
                    body["max_tokens"] = serde_json::json!(budget + 4096);
                }
            }
        }

        let mut req = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json");

        if use_thinking {
            req = req.header("anthropic-beta", "interleaved-thinking-2025-05-14");
        }

        let response = req.json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::LlmError::ApiError {
                provider: "anthropic".into(),
                status,
                body,
            }
            .into());
        }

        // Parse SSE stream
        let byte_stream = response.bytes_stream();
        let event_stream = parse_anthropic_sse(byte_stream);

        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, model: &str) -> bool {
        model.contains("opus") || model.contains("sonnet")
    }

    fn supports_images(&self, _model: &str) -> bool {
        true
    }
}

/// Anthropic SSE event types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessage },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: ContentDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaBody,
        usage: Option<AnthropicUsage>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: AnthropicApiError },
}

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessageDeltaBody {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicApiError {
    #[serde(default)]
    message: String,
}

/// Parse an Anthropic SSE byte stream into StreamEvents.
/// Public so Vertex AI (same SSE format) can reuse it.
pub fn parse_anthropic_sse(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>>
    + Send
    + Unpin
    + 'static,
) -> impl Stream<Item = StreamEvent> + Send + Unpin {
    use futures::StreamExt;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut buffer = String::new();
        let mut current_event_type = String::new();
        let mut message = Message::new_assistant();
        let mut input_usage = AnthropicUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        // Track tool call state: (id, name, accumulated_json)
        let mut tool_calls: std::collections::HashMap<usize, (String, String, String)> =
            std::collections::HashMap::new();

        let mut byte_stream = std::pin::pin!(byte_stream);

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error {
                        reason: format!("Stream read error: {e}"),
                    });
                    return;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE lines
            while let Some(pos) = buffer.find("\n\n") {
                let block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in block.lines() {
                    if let Some(event_name) = line.strip_prefix("event: ") {
                        current_event_type = event_name.to_string();
                    } else if let Some(data) = line.strip_prefix("data: ") {
                        if current_event_type.is_empty() {
                            continue;
                        }

                        let event: AnthropicEvent = match serde_json::from_str(data) {
                            Ok(e) => e,
                            Err(e) => {
                                tracing::debug!("SSE parse skip ({}): {e}", current_event_type);
                                continue;
                            }
                        };

                        match event {
                            AnthropicEvent::MessageStart { message: msg } => {
                                if let Some(u) = msg.usage {
                                    input_usage = u;
                                }
                                let _ = tx.send(StreamEvent::Start {
                                    partial: message.clone(),
                                });
                            }

                            AnthropicEvent::ContentBlockStart {
                                index,
                                content_block,
                            } => {
                                if let ContentBlock::ToolUse { id, name } = content_block {
                                    tool_calls.insert(index, (id, name, String::new()));
                                }
                            }

                            AnthropicEvent::ContentBlockDelta { index, delta } => match delta {
                                ContentDelta::TextDelta { text } => {
                                    message.push_text(&text);
                                    let _ = tx.send(StreamEvent::TextDelta { index, delta: text });
                                }
                                ContentDelta::ThinkingDelta { thinking } => {
                                    message.push_thinking(&thinking);
                                    let _ = tx.send(StreamEvent::ThinkingDelta {
                                        index,
                                        delta: thinking,
                                    });
                                }
                                ContentDelta::SignatureDelta { signature } => {
                                    // Associate signature with the thinking block at this index.
                                    // Must be sent back verbatim in subsequent API calls.
                                    message.set_last_thinking_signature(&signature);
                                    let _ = tx
                                        .send(StreamEvent::ThinkingSignature { index, signature });
                                }
                                ContentDelta::InputJsonDelta { partial_json } => {
                                    if let Some(tc) = tool_calls.get_mut(&index) {
                                        tc.2.push_str(&partial_json);
                                    }
                                    let _ = tx.send(StreamEvent::ToolCallDelta {
                                        index,
                                        delta: partial_json,
                                    });
                                }
                            },

                            AnthropicEvent::ContentBlockStop { index } => {
                                if let Some((id, name, json_str)) = tool_calls.remove(&index) {
                                    let arguments: serde_json::Value = serde_json::from_str(
                                        &json_str,
                                    )
                                    .unwrap_or(serde_json::Value::Object(Default::default()));
                                    let tc = ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments: arguments.clone(),
                                    };
                                    message.finalize_tool_call(tc.clone());
                                    let _ = tx.send(StreamEvent::ToolCallEnd {
                                        index,
                                        tool_call: tc,
                                    });
                                }
                            }

                            AnthropicEvent::MessageDelta {
                                delta,
                                usage: output_usage,
                            } => {
                                let mut total_output = input_usage.output_tokens;
                                if let Some(u) = &output_usage {
                                    total_output = u.output_tokens;
                                }

                                let stop = delta.stop_reason.as_deref().unwrap_or("end_turn");
                                let reason = match stop {
                                    "end_turn" => StopReason::EndTurn,
                                    "tool_use" => StopReason::ToolUse,
                                    "max_tokens" => StopReason::MaxTokens,
                                    "stop_sequence" => StopReason::StopSequence,
                                    _ => StopReason::EndTurn,
                                };

                                message.stop_reason = Some(reason.clone());
                                message.provider = Some("anthropic".into());

                                let usage = Usage {
                                    input_tokens: input_usage.input_tokens,
                                    output_tokens: total_output,
                                    cache_read_tokens: input_usage.cache_read_input_tokens,
                                    cache_write_tokens: input_usage.cache_creation_input_tokens,
                                    cost: Cost::default(), // Calculated by caller
                                };
                                message.usage = Some(usage.clone());

                                let _ = tx.send(StreamEvent::Done {
                                    reason,
                                    message: message.clone(),
                                    usage,
                                });
                            }

                            AnthropicEvent::MessageStop => {
                                // Already handled in MessageDelta
                            }

                            AnthropicEvent::Error { error } => {
                                let _ = tx.send(StreamEvent::Error {
                                    reason: error.message,
                                });
                            }

                            AnthropicEvent::Ping => {}
                        }

                        current_event_type.clear();
                    }
                }
            }
        }
    });

    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}
