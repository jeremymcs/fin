// Fin — OpenAI Chat Completions API Provider
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use futures::stream::Stream;
use serde::Deserialize;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

const API_BASE: &str = "https://api.openai.com/v1";

pub struct OpenAIProvider {
    client: reqwest::Client,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: API_BASE.to_string(),
        }
    }

    pub(crate) fn convert_messages(system: &str, messages: &[Message]) -> Vec<serde_json::Value> {
        let mut out = Vec::new();

        if !system.is_empty() {
            out.push(serde_json::json!({
                "role": "system",
                "content": system
            }));
        }

        for msg in messages {
            match msg.role {
                Role::User => {
                    let text = msg
                        .content
                        .iter()
                        .find_map(|c| match c {
                            Content::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    out.push(serde_json::json!({ "role": "user", "content": text }));
                }
                Role::Assistant => {
                    let mut content_text = String::new();
                    let mut tool_calls_json = Vec::new();

                    for c in &msg.content {
                        match c {
                            Content::Text { text } => content_text.push_str(text),
                            Content::ToolCall(tc) => {
                                tool_calls_json.push(serde_json::json!({
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {
                                        "name": tc.name,
                                        "arguments": tc.arguments.to_string()
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    // Skip messages that only contained thinking (no text or tool calls).
                    // OpenAI doesn't understand thinking blocks — sending an empty assistant
                    // message would cause a 400.
                    if content_text.is_empty() && tool_calls_json.is_empty() {
                        continue;
                    }
                    let mut msg_json = serde_json::json!({ "role": "assistant" });
                    if !content_text.is_empty() {
                        msg_json["content"] = serde_json::json!(content_text);
                    }
                    if !tool_calls_json.is_empty() {
                        msg_json["tool_calls"] = serde_json::json!(tool_calls_json);
                    }
                    out.push(msg_json);
                }
                Role::ToolResult => {
                    let text = msg
                        .content
                        .iter()
                        .find_map(|c| match c {
                            Content::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    out.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": text
                    }));
                }
            }
        }

        out
    }

    pub(crate) fn convert_tools(tools: &[ToolSchema]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
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
            .get_api_key("openai")
            .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY not set"))?;

        let messages = Self::convert_messages(&context.system_prompt, &context.messages);

        let mut body = serde_json::json!({
            "model": model.id,
            "max_tokens": options.max_tokens,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": messages,
        });

        if !context.tools.is_empty() {
            body["tools"] = serde_json::json!(Self::convert_tools(&context.tools));
        }

        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Reasoning effort for o-series models
        if model.capabilities.thinking && !matches!(options.thinking_level, ThinkingLevel::Off) {
            let effort = match options.thinking_level {
                ThinkingLevel::Minimal | ThinkingLevel::Low => "low",
                ThinkingLevel::Medium => "medium",
                ThinkingLevel::High | ThinkingLevel::XHigh => "high",
                ThinkingLevel::Off => "low",
            };
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::LlmError::ApiError {
                provider: "openai".into(),
                status,
                body,
            }
            .into());
        }

        let byte_stream = response.bytes_stream();
        let event_stream = parse_openai_sse(byte_stream, "openai");

        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, model: &str) -> bool {
        model.starts_with("o3") || model.starts_with("o4")
    }

    fn supports_images(&self, _model: &str) -> bool {
        true
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIChunk {
    choices: Vec<OpenAIChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    delta: OpenAIDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAIFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

pub(crate) fn parse_openai_sse(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>>
    + Send
    + Unpin
    + 'static,
    provider_name: &str,
) -> impl Stream<Item = StreamEvent> + Send + Unpin {
    let provider_name = provider_name.to_string();
    use futures::StreamExt;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut buffer = String::new();
        let mut message = Message::new_assistant();
        // Track tool calls: index → (id, name, accumulated_args)
        let mut tool_calls: std::collections::HashMap<usize, (String, String, String)> =
            std::collections::HashMap::new();
        let mut last_usage = OpenAIUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
        };
        let mut started = false;

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

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d,
                    None => continue,
                };

                if data == "[DONE]" {
                    // Finalize any remaining tool calls
                    for (index, (id, name, args_json)) in &tool_calls {
                        let arguments: serde_json::Value = serde_json::from_str(args_json)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        let tc = ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments,
                        };
                        message.finalize_tool_call(tc.clone());
                        let _ = tx.send(StreamEvent::ToolCallEnd {
                            index: *index,
                            tool_call: tc,
                        });
                    }

                    message.provider = Some(provider_name.clone());
                    let usage = Usage {
                        input_tokens: last_usage.prompt_tokens,
                        output_tokens: last_usage.completion_tokens,
                        ..Default::default()
                    };
                    message.usage = Some(usage.clone());
                    let reason = message.stop_reason.clone().unwrap_or(StopReason::EndTurn);
                    let _ = tx.send(StreamEvent::Done {
                        reason,
                        message: message.clone(),
                        usage,
                    });
                    return;
                }

                let chunk: OpenAIChunk = match serde_json::from_str(data) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(u) = chunk.usage {
                    last_usage = u;
                }

                for choice in &chunk.choices {
                    if !started {
                        started = true;
                        let _ = tx.send(StreamEvent::Start {
                            partial: message.clone(),
                        });
                    }

                    // Text content
                    if let Some(ref text) = choice.delta.content {
                        message.push_text(text);
                        let _ = tx.send(StreamEvent::TextDelta {
                            index: 0,
                            delta: text.clone(),
                        });
                    }

                    // Tool calls
                    if let Some(ref tcs) = choice.delta.tool_calls {
                        for tc_delta in tcs {
                            let entry = tool_calls
                                .entry(tc_delta.index)
                                .or_insert_with(|| (String::new(), String::new(), String::new()));
                            if let Some(ref id) = tc_delta.id {
                                entry.0 = id.clone();
                            }
                            if let Some(ref func) = tc_delta.function {
                                if let Some(ref name) = func.name {
                                    entry.1 = name.clone();
                                }
                                if let Some(ref args) = func.arguments {
                                    entry.2.push_str(args);
                                    let _ = tx.send(StreamEvent::ToolCallDelta {
                                        index: tc_delta.index,
                                        delta: args.clone(),
                                    });
                                }
                            }
                        }
                    }

                    // Stop reason
                    if let Some(ref reason) = choice.finish_reason {
                        message.stop_reason = Some(match reason.as_str() {
                            "stop" => StopReason::EndTurn,
                            "tool_calls" => StopReason::ToolUse,
                            "length" => StopReason::MaxTokens,
                            _ => StopReason::EndTurn,
                        });
                    }
                }
            }
        }
    });

    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}
