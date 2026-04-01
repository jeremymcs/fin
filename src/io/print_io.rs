// Fin — Print/Headless IO Adapter
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use std::io::Write;

use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::types::Message;

/// Simple IO adapter that prints to stdout.
/// Used by print mode and headless mode.
pub struct PrintIO {
    show_thinking: bool,
    show_tools: bool,
}

impl PrintIO {
    pub fn new(show_thinking: bool, show_tools: bool) -> Self {
        Self {
            show_thinking,
            show_tools,
        }
    }
}

#[async_trait]
impl AgentIO for PrintIO {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        match event {
            AgentEvent::TextDelta { text } => {
                print!("{text}");
                std::io::stdout().flush().ok();
            }
            AgentEvent::ThinkingDelta { text } => {
                if self.show_thinking {
                    print!("\x1b[2m{text}\x1b[0m");
                    std::io::stdout().flush().ok();
                }
            }
            AgentEvent::ToolStart { name, .. } => {
                if self.show_tools {
                    eprintln!("\x1b[36m⚙ {name}\x1b[0m");
                }
            }
            AgentEvent::ToolEnd { name, is_error, .. } => {
                if self.show_tools {
                    if is_error {
                        eprintln!("\x1b[31m✗ {name} failed\x1b[0m");
                    } else {
                        eprintln!("\x1b[32m✓ {name}\x1b[0m");
                    }
                }
            }
            AgentEvent::TurnStart => {
                // Separator between turns
            }
            AgentEvent::TurnEnd => {}
            AgentEvent::ModelChanged { .. } => {}
            AgentEvent::AgentStart { .. } => {}
            AgentEvent::AgentEnd { usage } => {
                println!();
                eprintln!(
                    "\x1b[2m[in: {} | out: {} tokens | cost: ${:.4}]\x1b[0m",
                    usage.input_tokens, usage.output_tokens, usage.cost.total,
                );
            }
            // Workflow events — print to stderr for CLI visibility
            AgentEvent::WorkflowUnitStart {
                ref blueprint_id,
                ref stage,
                ref section_id,
                ref task_id,
                ..
            } => {
                let pos = format!(
                    "{}/{}{}",
                    blueprint_id,
                    section_id.as_deref().unwrap_or("-"),
                    task_id
                        .as_ref()
                        .map(|t| format!("/{t}"))
                        .unwrap_or_default()
                );
                eprintln!("\n\x1b[36m[{pos} {stage}]\x1b[0m");
            }
            AgentEvent::WorkflowUnitEnd { .. } => {}
            AgentEvent::WorkflowProgress {
                sections_done,
                sections_total,
                tasks_done,
                tasks_total,
                ..
            } => {
                eprintln!(
                    "\x1b[2m  sections: {}/{} | tasks: {}/{}\x1b[0m",
                    sections_done, sections_total, tasks_done, tasks_total
                );
            }
            AgentEvent::WorkflowComplete {
                ref blueprint_id,
                units_run,
            } => {
                eprintln!(
                    "\n\x1b[32m✓ Blueprint {blueprint_id} complete ({units_run} units)\x1b[0m"
                );
            }
            AgentEvent::WorkflowBlocked { ref reason, .. } => {
                eprintln!("\n\x1b[33m⏸ Blocked: {reason}\x1b[0m");
            }
            AgentEvent::WorkflowError { ref message } => {
                eprintln!("\n\x1b[31m✗ Workflow error: {message}\x1b[0m");
            }
            AgentEvent::StageTransition { ref from, ref to } => {
                eprintln!("\x1b[36m{from} → {to}\x1b[0m");
            }
            // TUI-layer signals — no output in print mode
            AgentEvent::AutoModeStart
            | AgentEvent::AutoModeEnd
            | AgentEvent::ContextUsage { .. }
            | AgentEvent::GitCommitUpdate { .. } => {}
        }
        Ok(())
    }

    async fn poll_steering(&self) -> Option<Message> {
        None // No steering in print mode
    }

    async fn poll_follow_up(&self) -> Option<Message> {
        None // No follow-ups in print mode
    }

    async fn request_input(&self, prompt: &str) -> anyhow::Result<String> {
        eprint!("{prompt}: ");
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    async fn request_confirmation(&self, prompt: &str) -> anyhow::Result<bool> {
        eprint!("{prompt} [y/N]: ");
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().eq_ignore_ascii_case("y"))
    }
}
