// Fin + AWS Bedrock Provider (Claude via Bedrock)
//
// Uses AWS credentials (env vars, profile, or IAM role) for auth.
// Bedrock uses binary EventStream framing for streaming, which is complex
// to parse natively. We use the AWS CLI for the invoke call and parse
// the response JSON.
//
// Set: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION
// Or: AWS_PROFILE for named profile auth

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;

pub struct BedrockProvider {
    #[allow(dead_code)]
    client: reqwest::Client,
}

impl BedrockProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LlmProvider for BedrockProvider {
    fn name(&self) -> &str {
        "bedrock"
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        context: &LlmContext,
        options: &StreamOptions,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + Unpin>>> {
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        // Build the Anthropic-format request body (minus model — goes in URL)
        let messages =
            crate::llm::anthropic::AnthropicProvider::convert_messages(&context.messages);

        let mut body = serde_json::json!({
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": options.max_tokens,
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

        // Use AWS CLI to invoke bedrock — avoids SigV4 + EventStream complexity
        // The CLI handles credential resolution, signing, and stream decoding
        let body_str = serde_json::to_string(&body)?;

        let output = tokio::process::Command::new("aws")
            .args([
                "bedrock-runtime",
                "invoke-model",
                "--model-id",
                &model.id,
                "--region",
                &region,
                "--content-type",
                "application/json",
                "--accept",
                "application/json",
                "--body",
                &body_str,
                "/dev/stdout",
            ])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to run AWS CLI: {e}. Install it or set AWS credentials.")
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::LlmError::ApiError {
                provider: "bedrock".into(),
                status: output.status.code().unwrap_or(1) as u16,
                body: stderr.to_string(),
            }
            .into());
        }

        // Parse the non-streaming response and emit as stream events
        let response_body: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| anyhow::anyhow!("Failed to parse Bedrock response: {e}"))?;

        let event_stream = bedrock_response_to_stream(response_body);
        Ok(Box::pin(event_stream))
    }

    fn supports_thinking(&self, model: &str) -> bool {
        model.contains("opus") || model.contains("sonnet")
    }

    fn supports_images(&self, _model: &str) -> bool {
        true
    }
}

/// Convert a Bedrock non-streaming response into a stream of StreamEvents.
/// This gives the agent loop a consistent interface even though Bedrock
/// returns the full response at once via the CLI.
fn bedrock_response_to_stream(
    response: serde_json::Value,
) -> impl Stream<Item = StreamEvent> + Send + Unpin {
    use futures::stream;

    let mut events = Vec::new();

    // Emit start
    events.push(StreamEvent::Start {
        partial: Message::new_assistant(),
    });

    // Extract content blocks
    let mut message = Message::new_assistant();
    let mut tool_index: usize = 0;

    if let Some(content) = response["content"].as_array() {
        for block in content {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(text) = block["text"].as_str() {
                        message.push_text(text);
                        events.push(StreamEvent::TextDelta {
                            index: 0,
                            delta: text.to_string(),
                        });
                    }
                }
                Some("thinking") => {
                    if let Some(text) = block["thinking"].as_str() {
                        message.push_thinking(text);
                        events.push(StreamEvent::ThinkingDelta {
                            index: 0,
                            delta: text.to_string(),
                        });
                    }
                }
                Some("tool_use") => {
                    let tc = ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: block["input"].clone(),
                    };
                    message.finalize_tool_call(tc.clone());
                    events.push(StreamEvent::ToolCallEnd {
                        index: tool_index,
                        tool_call: tc,
                    });
                    tool_index += 1;
                }
                _ => {}
            }
        }
    }

    // Extract usage
    let input_tokens = response["usage"]["input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = response["usage"]["output_tokens"].as_u64().unwrap_or(0);

    let stop = response["stop_reason"].as_str().unwrap_or("end_turn");
    let reason = match stop {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };

    message.stop_reason = Some(reason.clone());
    message.provider = Some("bedrock".into());
    message.usage = Some(Usage {
        input_tokens,
        output_tokens,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        cost: Cost::default(),
    });

    events.push(StreamEvent::Done {
        reason,
        message,
        usage: Usage {
            input_tokens,
            output_tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost: Cost::default(),
        },
    });

    stream::iter(events)
}
