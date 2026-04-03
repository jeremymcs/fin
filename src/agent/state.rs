// Fin + Agent State Management

use crate::llm::models::ModelConfig;
use crate::llm::types::{Message, ThinkingLevel, Usage};
use crate::tools::ToolRegistry;
use std::collections::HashSet;

/// Core agent state — everything the agent loop needs to operate.
pub struct AgentState {
    /// Full conversation history
    pub messages: Vec<Message>,
    /// Active LLM model
    pub model: ModelConfig,
    /// Thinking/reasoning level
    pub thinking_level: ThinkingLevel,
    /// Registered tools
    pub tool_registry: ToolRegistry,
    /// Constructed system prompt
    pub system_prompt: String,
    /// Currently streaming a response
    pub is_streaming: bool,
    /// Tool calls in flight
    #[allow(dead_code)]
    pub pending_tool_calls: HashSet<String>,
    /// Cumulative token usage for this session
    pub cumulative_usage: Usage,
    /// Working directory
    #[allow(dead_code)]
    pub cwd: std::path::PathBuf,
    /// Session ID
    pub session_id: String,
}

impl AgentState {
    pub fn new(model: ModelConfig, cwd: std::path::PathBuf) -> Self {
        Self {
            messages: Vec::new(),
            model,
            thinking_level: ThinkingLevel::default(),
            tool_registry: ToolRegistry::new(),
            system_prompt: String::new(),
            is_streaming: false,
            pending_tool_calls: HashSet::new(),
            cumulative_usage: Usage::default(),
            cwd,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn append_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn add_usage(&mut self, usage: &Usage) {
        self.cumulative_usage.input_tokens += usage.input_tokens;
        self.cumulative_usage.output_tokens += usage.output_tokens;
        self.cumulative_usage.cache_read_tokens += usage.cache_read_tokens;
        self.cumulative_usage.cache_write_tokens += usage.cache_write_tokens;
        self.cumulative_usage.cost.total += usage.cost.total;
    }
}
