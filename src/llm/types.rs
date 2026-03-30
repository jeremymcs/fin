// Fin — LLM Core Types
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolCall(ToolCall),
    ToolResult {
        tool_call_id: String,
        content: Vec<Content>,
        is_error: bool,
    },
    Image {
        media_type: String,
        data: String, // base64
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
}

impl Message {
    pub fn new_user(text: &str) -> Self {
        Self {
            role: Role::User,
            content: vec![Content::Text {
                text: text.to_string(),
            }],
            usage: None,
            timestamp: chrono::Utc::now().timestamp(),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            model: None,
            provider: None,
            stop_reason: None,
        }
    }

    pub fn new_assistant() -> Self {
        Self {
            role: Role::Assistant,
            content: Vec::new(),
            usage: None,
            timestamp: chrono::Utc::now().timestamp(),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            model: None,
            provider: None,
            stop_reason: None,
        }
    }

    pub fn push_text(&mut self, delta: &str) {
        if let Some(Content::Text { text }) = self.content.last_mut() {
            text.push_str(delta);
        } else {
            self.content.push(Content::Text {
                text: delta.to_string(),
            });
        }
    }

    pub fn push_thinking(&mut self, delta: &str) {
        if let Some(Content::Thinking { text }) = self.content.last_mut() {
            text.push_str(delta);
        } else {
            self.content.push(Content::Thinking {
                text: delta.to_string(),
            });
        }
    }

    pub fn push_tool_call_delta(&mut self, _delta: &str) {
        // Tool call deltas accumulate JSON fragments
        // Finalized by finalize_tool_call
    }

    pub fn finalize_tool_call(&mut self, tool_call: ToolCall) {
        self.content.push(Content::ToolCall(tool_call));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost: Cost,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    XHigh,
}

/// Tool schema for LLM tool registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Context passed to the LLM provider.
pub struct LlmContext {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
}

/// Options for streaming.
pub struct StreamOptions {
    pub max_tokens: u32,
    pub thinking_level: ThinkingLevel,
    pub temperature: Option<f32>,
}

/// Events emitted during LLM streaming.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum StreamEvent {
    Start {
        partial: Message,
    },
    TextDelta {
        index: usize,
        delta: String,
    },
    ThinkingDelta {
        index: usize,
        delta: String,
    },
    ToolCallDelta {
        index: usize,
        delta: String,
    },
    ToolCallEnd {
        index: usize,
        tool_call: ToolCall,
    },
    Done {
        reason: StopReason,
        message: Message,
        usage: Usage,
    },
    Error {
        reason: String,
    },
}
