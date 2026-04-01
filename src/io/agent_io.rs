// Fin — Agent I/O Trait (Transport-Agnostic)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::llm::types::{Message, Usage};
use async_trait::async_trait;

/// Events emitted by the agent during execution.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AgentEvent {
    AgentStart {
        session_id: String,
    },
    AgentEnd {
        usage: Usage,
    },
    TurnStart,
    TurnEnd,
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    ToolStart {
        id: String,
        name: String,
    },
    ToolEnd {
        id: String,
        name: String,
        is_error: bool,
    },
    ModelChanged {
        display_name: String,
    },

    // ── Workflow events ──────────────────────────────────────
    /// A dispatch unit is about to start execution.
    WorkflowUnitStart {
        blueprint_id: String,
        section_id: Option<String>,
        task_id: Option<String>,
        stage: String,
        unit_type: String,
    },
    /// A dispatch unit completed successfully.
    WorkflowUnitEnd {
        blueprint_id: String,
        section_id: Option<String>,
        task_id: Option<String>,
        stage: String,
        artifacts: Vec<String>,
    },
    /// Progress snapshot — emitted after each unit completes.
    WorkflowProgress {
        blueprint_id: String,
        sections_total: u32,
        sections_done: u32,
        tasks_total: u32,
        tasks_done: u32,
        current_stage: String,
        current_section: Option<String>,
        current_task: Option<String>,
    },
    /// Workflow completed (all sections done).
    WorkflowComplete {
        blueprint_id: String,
        units_run: u32,
    },
    /// Workflow blocked — needs user input.
    WorkflowBlocked {
        blueprint_id: String,
        reason: String,
    },
    /// Workflow error.
    WorkflowError {
        message: String,
    },
    /// Stage transition — emitted when workflow moves between stages (e.g., Build → Validate).
    StageTransition {
        from: String,
        to: String,
    },
    /// Auto-loop execution started — TUI-layer signal (D-13).
    AutoModeStart,
    /// Auto-loop execution ended (completed, blocked, or cancelled) — TUI-layer signal (D-13).
    AutoModeEnd,
    /// Context window utilization — emitted once per agent turn completion (D-07).
    ContextUsage { pct: u8 },
    /// Git commit update — result of async git log fetch after WorkflowUnitEnd.
    GitCommitUpdate { hash: String, msg: String },
}

/// Transport-agnostic interface for agent communication.
///
/// Implementations:
/// - InteractiveIO: TUI mode (renders to terminal)
/// - HeadlessIO: JSON over stdin/stdout
/// - HttpIO: SSE streaming over HTTP
/// - McpIO: MCP protocol
#[async_trait]
pub trait AgentIO: Send + Sync {
    /// Emit an event to the client.
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()>;

    /// Poll for a steering message (interrupts current turn). Non-blocking.
    async fn poll_steering(&self) -> Option<Message>;

    /// Poll for a follow-up message (after agent completion). Non-blocking.
    async fn poll_follow_up(&self) -> Option<Message>;

    /// Request text input from the user.
    #[allow(dead_code)]
    async fn request_input(&self, prompt: &str) -> anyhow::Result<String>;

    /// Request a yes/no confirmation from the user.
    #[allow(dead_code)]
    async fn request_confirmation(&self, prompt: &str) -> anyhow::Result<bool>;
}
