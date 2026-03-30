// Fin — Core Agent Loop
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::agent::compaction;
use crate::agent::state::AgentState;
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::provider::LlmProvider;
use crate::llm::types::{Content, LlmContext, Message, Role, StreamEvent, StreamOptions, ToolCall};

/// Maximum number of turns before the agent stops to prevent infinite loops.
const MAX_TURNS: u32 = 200;

/// Maximum size (chars) for a single tool result before truncation.
const MAX_TOOL_RESULT_CHARS: usize = 100_000;

/// Main agent execution loop.
///
/// Cycles: build context → stream LLM response → execute tool calls → repeat
/// until the LLM stops requesting tools (or cancelled).
pub async fn run_agent_loop(
    state: &mut AgentState,
    provider: &dyn LlmProvider,
    io: &dyn AgentIO,
    cancel: CancellationToken,
) -> Result<()> {
    io.emit(AgentEvent::AgentStart {
        session_id: state.session_id.clone(),
    })
    .await?;

    let mut turn_count: u32 = 0;

    loop {
        if cancel.is_cancelled() {
            break;
        }

        // Guard: max turn limit
        turn_count += 1;
        if turn_count > MAX_TURNS {
            tracing::warn!("Agent hit max turn limit ({MAX_TURNS}). Stopping.");
            io.emit(AgentEvent::TextDelta {
                text: format!("\n\n[Agent stopped: reached {MAX_TURNS} turn limit]"),
            })
            .await?;
            break;
        }

        // Guard: context window — hard stop, no compaction (avoid token rot)
        if compaction::needs_compaction(
            &state.messages,
            state.model.context_window,
            &state.model.provider,
        ) {
            tracing::warn!("Context window at capacity. Stopping agent loop.");
            io.emit(AgentEvent::TextDelta {
                text: "\n\n[Context window nearly full. Use /next to rotate context with a handoff, or /clear to start fresh.]".to_string(),
            }).await?;
            break;
        }

        // Inject any pending steering messages
        if let Some(steering) = io.poll_steering().await {
            state.append_message(steering);
        }

        io.emit(AgentEvent::TurnStart).await?;

        // Build LLM context from current state
        let context = build_llm_context(state);
        let options = build_stream_options(state);

        // Stream the LLM response (with retry on transient errors)
        state.is_streaming = true;
        let mut stream = {
            let mut last_err = None;
            let mut attempt = 0u32;
            const MAX_RETRIES: u32 = 5;
            loop {
                attempt += 1;
                match provider
                    .stream(&state.model, &context, &options, cancel.clone())
                    .await
                {
                    Ok(s) => break s,
                    Err(e) => {
                        let err_str = format!("{e}");

                        // Auth/billing errors — don't retry, surface immediately
                        if err_str.contains("401")
                            || err_str.contains("403")
                            || err_str.contains("Unauthorized")
                            || err_str.contains("Forbidden")
                            || err_str.contains("invalid_api_key")
                            || err_str.contains("insufficient_quota")
                            || err_str.contains("billing")
                            || err_str.contains("exceeded")
                        {
                            let hint = if err_str.contains("insufficient_quota")
                                || err_str.contains("exceeded")
                            {
                                "Quota exhausted. Top up credits or switch provider with /model."
                            } else {
                                "Check your API key."
                            };
                            io.emit(AgentEvent::TextDelta {
                                text: format!(
                                    "\n[Provider '{}': {hint}]\n",
                                    state.model.provider
                                ),
                            })
                            .await?;
                            return Err(e);
                        }

                        let is_retryable = err_str.contains("429")
                            || err_str.contains("503")
                            || err_str.contains("529")
                            || err_str.contains("overloaded")
                            || err_str.contains("timeout")
                            || err_str.contains("connection")
                            || err_str.contains("reset");

                        if is_retryable && attempt <= MAX_RETRIES {
                            // Exponential backoff: 2s, 4s, 8s, 16s, 32s
                            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
                            tracing::warn!(
                                "LLM request failed (attempt {attempt}/{MAX_RETRIES}): {e}. Retrying in {delay:?}"
                            );
                            io.emit(AgentEvent::TextDelta {
                                text: format!(
                                    "\n[Retrying ({attempt}/{MAX_RETRIES}) in {}s: {}]\n",
                                    delay.as_secs(),
                                    if err_str.contains("429") {
                                        "rate limited"
                                    } else if err_str.contains("503") || err_str.contains("overloaded") {
                                        "provider overloaded"
                                    } else {
                                        "transient error"
                                    }
                                ),
                            })
                            .await?;
                            tokio::time::sleep(delay).await;
                            last_err = Some(e);
                            continue;
                        }

                        io.emit(AgentEvent::TextDelta {
                            text: format!("\n[LLM request failed after {attempt} attempts: {e}]\n"),
                        })
                        .await?;
                        return Err(last_err.unwrap_or(e));
                    }
                }
            }
        };

        let assistant_msg = process_stream(&mut stream, io).await?;
        state.is_streaming = false;

        // Track usage
        if let Some(ref usage) = assistant_msg.usage {
            state.add_usage(usage);
        }

        state.append_message(assistant_msg.clone());

        // Extract tool calls from the response
        let tool_calls = extract_tool_calls(&assistant_msg);

        if tool_calls.is_empty() {
            io.emit(AgentEvent::TurnEnd).await?;

            // Check for follow-up messages
            if let Some(follow_up) = io.poll_follow_up().await {
                state.append_message(follow_up);
                continue;
            }
            break; // No tools, no follow-up — done
        }

        // Execute tool calls
        for tool_call in &tool_calls {
            if cancel.is_cancelled() {
                break;
            }

            io.emit(AgentEvent::ToolStart {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
            })
            .await?;

            let result = state
                .tool_registry
                .execute(
                    &tool_call.name,
                    &tool_call.id,
                    tool_call.arguments.clone(),
                    cancel.clone(),
                )
                .await;

            let (content, is_error) = match result {
                Ok(r) => {
                    if r.is_error {
                        tracing::warn!("Tool '{}' returned error", tool_call.name);
                    }
                    (r.content, r.is_error)
                }
                Err(e) => {
                    tracing::error!("Tool '{}' execution failed: {e}", tool_call.name);
                    let err_content = vec![Content::Text {
                        text: format!("Tool error: {e}"),
                    }];
                    (err_content, true)
                }
            };

            io.emit(AgentEvent::ToolEnd {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                is_error,
            })
            .await?;

            // Truncate large tool results to prevent context overflow
            let content = truncate_tool_result(content);

            // Append tool result as a message
            state.append_message(Message {
                role: Role::ToolResult,
                content: content.clone(),
                usage: None,
                timestamp: chrono::Utc::now().timestamp(),
                tool_call_id: Some(tool_call.id.clone()),
                tool_name: Some(tool_call.name.clone()),
                is_error: Some(is_error),
                model: None,
                provider: None,
                stop_reason: None,
            });
        }

        io.emit(AgentEvent::TurnEnd).await?;
    }

    io.emit(AgentEvent::AgentEnd {
        usage: state.cumulative_usage.clone(),
    })
    .await?;

    Ok(())
}

fn build_llm_context(state: &AgentState) -> LlmContext {
    LlmContext {
        system_prompt: state.system_prompt.clone(),
        messages: state.messages.clone(),
        tools: state.tool_registry.schemas(),
    }
}

fn build_stream_options(state: &AgentState) -> StreamOptions {
    StreamOptions {
        max_tokens: state.model.max_tokens,
        thinking_level: state.thinking_level.clone(),
        temperature: None,
    }
}

fn extract_tool_calls(msg: &Message) -> Vec<ToolCall> {
    msg.content
        .iter()
        .filter_map(|c| match c {
            Content::ToolCall(tc) => Some(tc.clone()),
            _ => None,
        })
        .collect()
}

async fn process_stream(
    stream: &mut (dyn futures::Stream<Item = StreamEvent> + Unpin + Send),
    io: &dyn AgentIO,
) -> Result<Message> {
    use futures::StreamExt;

    let mut message = Message::new_assistant();

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta { delta, .. } => {
                io.emit(AgentEvent::TextDelta {
                    text: delta.clone(),
                })
                .await?;
                message.push_text(&delta);
            }
            StreamEvent::ThinkingDelta { delta, .. } => {
                io.emit(AgentEvent::ThinkingDelta {
                    text: delta.clone(),
                })
                .await?;
                message.push_thinking(&delta);
            }
            StreamEvent::ToolCallDelta { delta, .. } => {
                message.push_tool_call_delta(&delta);
            }
            StreamEvent::ToolCallEnd { tool_call, .. } => {
                message.finalize_tool_call(tool_call);
            }
            StreamEvent::Done { usage, reason, .. } => {
                message.usage = Some(usage);
                message.stop_reason = Some(reason);
                break;
            }
            StreamEvent::Error { reason } => {
                return Err(anyhow::anyhow!("Stream error: {reason}"));
            }
            StreamEvent::Start { .. } => {}
        }
    }

    Ok(message)
}

/// Truncate large tool results to prevent context overflow.
fn truncate_tool_result(content: Vec<Content>) -> Vec<Content> {
    content
        .into_iter()
        .map(|c| match c {
            Content::Text { text } if text.len() > MAX_TOOL_RESULT_CHARS => Content::Text {
                text: format!(
                    "{}...\n\n[Truncated: {} chars total, showing first {}]",
                    &text[..MAX_TOOL_RESULT_CHARS],
                    text.len(),
                    MAX_TOOL_RESULT_CHARS,
                ),
            },
            other => other,
        })
        .collect()
}
