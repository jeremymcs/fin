// Fin + Google Gemini API Provider

use async_trait::async_trait;
use futures::stream::Stream;
use serde::Deserialize;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GoogleProvider {
    client: reqwest::Client,
    base_url: String,
}

impl GoogleProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: API_BASE.to_string(),
        }
    }

    fn convert_messages(
        system: &str,
        messages: &[Message],
    ) -> (Option<serde_json::Value>, Vec<serde_json::Value>) {
        let system_instruction = if system.is_empty() {
            None
        } else {
            Some(serde_json::json!({
                "parts": [{ "text": system }]
            }))
        };

        let contents: Vec<serde_json::Value> = messages
            .iter()
            .filter_map(|msg| {
                let role = match msg.role {
                    Role::User | Role::ToolResult => "user",
                    Role::Assistant => "model",
                };

                let parts: Vec<serde_json::Value> = msg
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::Text { text } => Some(serde_json::json!({ "text": text })),
                        Content::ToolCall(tc) => Some(serde_json::json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments
                            }
                        })),
                        _ => None,
                    })
                    .collect();

                // For tool results, wrap as functionResponse
                if msg.role == Role::ToolResult {
                    let text = msg
                        .content
                        .iter()
                        .find_map(|c| match c {
                            Content::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();

                    return Some(serde_json::json!({
                        "role": role,
                        "parts": [{
                            "functionResponse": {
                                "name": msg.tool_name.as_deref().unwrap_or("unknown"),
                                "response": { "result": text }
                            }
                        }]
                    }));
                }

                if parts.is_empty() {
                    return None;
                }

                Some(serde_json::json!({ "role": role, "parts": parts }))
            })
            .collect();

        (system_instruction, contents)
    }

    fn convert_tools(tools: &[ToolSchema]) -> serde_json::Value {
        let functions: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })
            })
            .collect();

        serde_json::json!([{ "functionDeclarations": functions }])
    }
}

#[async_trait]
impl LlmProvider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>> {
        let auth = crate::config::auth::AuthStore::default();
        let env_api_key = std::env::var("GOOGLE_API_KEY")
            .ok()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok());

        let mut google_oauth = if env_api_key.is_none() {
            auth.get_google_oauth()
        } else {
            None
        };

        let api_key = auth.get_api_key("google").or(env_api_key);

        let (system_instruction, contents) =
            Self::convert_messages(&context.system_prompt, &context.messages);

        let mut body = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": options.max_tokens,
            }
        });

        if let Some(si) = system_instruction {
            body["systemInstruction"] = si;
        }

        if !context.tools.is_empty() {
            body["tools"] = Self::convert_tools(&context.tools);
        }

        if let Some(temp) = options.temperature {
            body["generationConfig"]["temperature"] = serde_json::json!(temp);
        }

        // Thinking for Gemini 2.5
        if model.capabilities.thinking && !matches!(options.thinking_level, ThinkingLevel::Off) {
            let budget = match options.thinking_level {
                ThinkingLevel::Minimal => 1024,
                ThinkingLevel::Low => 2048,
                ThinkingLevel::Medium => 8192,
                ThinkingLevel::High => 16384,
                ThinkingLevel::XHigh => 32768,
                ThinkingLevel::Off => 0,
            };
            body["generationConfig"]["thinkingConfig"] = serde_json::json!({
                "thinkingBudget": budget
            });
        }

        let response = if let Some(ref mut oauth) = google_oauth {
            let paths = crate::config::paths::FinPaths::resolve()?;
            let access_token = crate::config::oauth::ensure_google_access_token(
                &self.client,
                &paths.auth_file,
                oauth,
            )
            .await?;
            let mut request = self
                .client
                .post(format!(
                    "{}/models/{}:streamGenerateContent?alt=sse",
                    self.base_url, model.id
                ))
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {access_token}"));
            if let Some(project_id) = oauth.project_id.as_deref() {
                request = request.header("x-goog-user-project", project_id);
            }
            request.json(&body).send().await?
        } else {
            let api_key =
                api_key.ok_or_else(|| anyhow::anyhow!("GOOGLE_API_KEY, GEMINI_API_KEY, or Google OAuth not set"))?;
            self.client
                .post(format!(
                    "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                    self.base_url, model.id, api_key
                ))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await?
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::LlmError::ApiError {
                provider: "google".into(),
                status,
                body,
            }
            .into());
        }

        let byte_stream = response.bytes_stream();
        let event_stream = parse_google_sse(byte_stream);

        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, model: &str) -> bool {
        model.contains("2.5")
    }

    fn supports_images(&self, _model: &str) -> bool {
        true
    }
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    Thought {
        thought: bool,
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
}

#[derive(Debug, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u64,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u64,
}

fn parse_google_sse(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>>
    + Send
    + Unpin
    + 'static,
) -> impl Stream<Item = StreamEvent> + Send + Unpin {
    use futures::StreamExt;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut buffer = String::new();
        let mut message = Message::new_assistant();
        let mut started = false;
        let mut last_usage = GeminiUsage {
            prompt_token_count: 0,
            candidates_token_count: 0,
        };
        let mut tool_index = 0usize;

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

            // Google sends SSE with `data: {...}\n\n`
            while let Some(pos) = buffer.find("\n\n") {
                let block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in block.lines() {
                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d,
                        None => continue,
                    };

                    let response: GeminiResponse = match serde_json::from_str(data) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    if let Some(u) = response.usage_metadata {
                        last_usage = u;
                    }

                    for candidate in &response.candidates {
                        if !started {
                            started = true;
                            let _ = tx.send(StreamEvent::Start {
                                partial: message.clone(),
                            });
                        }

                        if let Some(ref content) = candidate.content {
                            for part in &content.parts {
                                match part {
                                    GeminiPart::Text { text } => {
                                        message.push_text(text);
                                        let _ = tx.send(StreamEvent::TextDelta {
                                            index: 0,
                                            delta: text.clone(),
                                        });
                                    }
                                    GeminiPart::Thought { text, .. } => {
                                        message.push_thinking(text);
                                        let _ = tx.send(StreamEvent::ThinkingDelta {
                                            index: 0,
                                            delta: text.clone(),
                                        });
                                    }
                                    GeminiPart::FunctionCall { function_call } => {
                                        let tc = ToolCall {
                                            id: format!("call_{}", uuid::Uuid::new_v4()),
                                            name: function_call.name.clone(),
                                            arguments: function_call.args.clone(),
                                        };
                                        message.finalize_tool_call(tc.clone());
                                        let _ = tx.send(StreamEvent::ToolCallEnd {
                                            index: tool_index,
                                            tool_call: tc,
                                        });
                                        tool_index += 1;
                                    }
                                }
                            }
                        }

                        if let Some(ref reason) = candidate.finish_reason {
                            let stop = match reason.as_str() {
                                "STOP" => StopReason::EndTurn,
                                "MAX_TOKENS" => StopReason::MaxTokens,
                                "SAFETY" => StopReason::Error,
                                _ => StopReason::EndTurn,
                            };
                            message.stop_reason = Some(stop);
                        }
                    }
                }
            }
        }

        // Emit Done
        message.provider = Some("google".into());
        let usage = Usage {
            input_tokens: last_usage.prompt_token_count,
            output_tokens: last_usage.candidates_token_count,
            ..Default::default()
        };
        message.usage = Some(usage.clone());
        let reason = message.stop_reason.clone().unwrap_or(StopReason::EndTurn);
        let _ = tx.send(StreamEvent::Done {
            reason,
            message,
            usage,
        });
    });

    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}
