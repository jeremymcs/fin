// Fin — Headless Mode (JSONL stdin/stdout)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::{AgentRegistry, DelegateTool};
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::*;
use crate::tools::ToolRegistry;

#[allow(dead_code)]
#[derive(Deserialize)]
struct HeadlessCommand {
    #[serde(rename = "type")]
    cmd_type: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Serialize)]
struct HeadlessEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<u64>,
}

impl HeadlessEvent {
    fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            text: None,
            tool_name: None,
            is_error: None,
            input_tokens: None,
            output_tokens: None,
        }
    }
}

struct HeadlessIO;

#[async_trait::async_trait]
impl AgentIO for HeadlessIO {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        let he = match event {
            AgentEvent::TextDelta { text } => {
                let mut e = HeadlessEvent::new("text_delta");
                e.text = Some(text);
                e
            }
            AgentEvent::ThinkingDelta { text } => {
                let mut e = HeadlessEvent::new("thinking_delta");
                e.text = Some(text);
                e
            }
            AgentEvent::ToolStart { name, .. } => {
                let mut e = HeadlessEvent::new("tool_start");
                e.tool_name = Some(name);
                e
            }
            AgentEvent::ToolEnd { name, is_error, .. } => {
                let mut e = HeadlessEvent::new("tool_end");
                e.tool_name = Some(name);
                e.is_error = Some(is_error);
                e
            }
            AgentEvent::AgentStart { .. } => HeadlessEvent::new("agent_start"),
            AgentEvent::AgentEnd { usage } => {
                let mut e = HeadlessEvent::new("agent_end");
                e.input_tokens = Some(usage.input_tokens);
                e.output_tokens = Some(usage.output_tokens);
                e
            }
            AgentEvent::TurnStart => HeadlessEvent::new("turn_start"),
            AgentEvent::TurnEnd => HeadlessEvent::new("turn_end"),
            AgentEvent::ModelChanged { ref display_name } => {
                let mut e = HeadlessEvent::new("model_changed");
                e.text = Some(display_name.clone());
                e
            }
            // Workflow events — serialize as JSONL for programmatic consumers
            AgentEvent::WorkflowUnitStart {
                ref blueprint_id,
                ref section_id,
                ref task_id,
                ref stage,
                ref unit_type,
            } => {
                let mut e = HeadlessEvent::new("workflow_unit_start");
                e.text = Some(
                    serde_json::json!({
                        "blueprint": blueprint_id,
                        "section": section_id,
                        "task": task_id,
                        "stage": stage,
                        "unit_type": unit_type,
                    })
                    .to_string(),
                );
                e
            }
            AgentEvent::WorkflowUnitEnd {
                ref blueprint_id,
                ref stage,
                ref artifacts,
                ..
            } => {
                let mut e = HeadlessEvent::new("workflow_unit_end");
                e.text = Some(
                    serde_json::json!({
                        "blueprint": blueprint_id,
                        "stage": stage,
                        "artifacts": artifacts,
                    })
                    .to_string(),
                );
                e
            }
            AgentEvent::WorkflowProgress {
                ref blueprint_id,
                sections_total,
                sections_done,
                tasks_total,
                tasks_done,
                ..
            } => {
                let mut e = HeadlessEvent::new("workflow_progress");
                e.text = Some(
                    serde_json::json!({
                        "blueprint": blueprint_id,
                        "sections_done": sections_done,
                        "sections_total": sections_total,
                        "tasks_done": tasks_done,
                        "tasks_total": tasks_total,
                    })
                    .to_string(),
                );
                e
            }
            AgentEvent::WorkflowComplete {
                ref blueprint_id,
                units_run,
            } => {
                let mut e = HeadlessEvent::new("workflow_complete");
                e.text = Some(
                    serde_json::json!({
                        "blueprint": blueprint_id,
                        "units_run": units_run,
                    })
                    .to_string(),
                );
                e
            }
            AgentEvent::WorkflowBlocked {
                ref blueprint_id,
                ref reason,
            } => {
                let mut e = HeadlessEvent::new("workflow_blocked");
                e.text = Some(
                    serde_json::json!({
                        "blueprint": blueprint_id,
                        "reason": reason,
                    })
                    .to_string(),
                );
                e
            }
            AgentEvent::WorkflowError { ref message } => {
                let mut e = HeadlessEvent::new("workflow_error");
                e.text = Some(message.clone());
                e.is_error = Some(true);
                e
            }
            AgentEvent::StageTransition { ref from, ref to } => {
                let mut e = HeadlessEvent::new("workflow_stage_transition");
                e.text = Some(format!("{from} → {to}"));
                e
            }
            // TUI-layer signals — not serialized in headless mode
            AgentEvent::AutoModeStart
            | AgentEvent::AutoModeEnd
            | AgentEvent::ContextUsage { .. }
            | AgentEvent::GitCommitUpdate { .. } => return Ok(()),
        };

        let json = serde_json::to_string(&he)?;
        println!("{json}");
        Ok(())
    }

    async fn poll_steering(&self) -> Option<Message> {
        None
    }
    async fn poll_follow_up(&self) -> Option<Message> {
        None
    }
    async fn request_input(&self, _: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
    async fn request_confirmation(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

pub async fn run(
    prompt: String,
    _timeout_ms: u64,
    _json_output: bool,
    _auto: bool,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let io = HeadlessIO;

    // Check if prompt is a workflow command
    let trimmed = prompt.trim();
    if let Some(workflow_result) = try_workflow_command(trimmed, &cwd, &io).await {
        return workflow_result;
    }

    // Regular prompt — run agent loop
    let model = crate::io::print::pick_model(None)?;

    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let agent_registry = Arc::new(AgentRegistry::load_default());

    let mut tool_registry = ToolRegistry::with_defaults(&cwd);
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }

    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.clone(),
            0,
        )));
    }

    let agent_context = if !agent_registry.is_empty() {
        Some(AgentPromptContext {
            available_agents: Some(agent_registry.prompt_summary()),
            agent_role: None,
        })
    } else {
        None
    };
    let system_prompt = build_system_prompt(&tool_registry.schemas(), &cwd, agent_context.as_ref());

    let mut state = AgentState::new(model.clone(), cwd);
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.messages.push(Message::new_user(&prompt));

    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let cancel = tokio_util::sync::CancellationToken::new();
    run_agent_loop(&mut state, provider, &io, cancel).await?;

    Ok(())
}

/// Try to handle the prompt as a workflow command.
/// Returns Some(result) if it was a workflow command, None if it's a regular prompt.
async fn try_workflow_command(
    prompt: &str,
    cwd: &std::path::Path,
    io: &HeadlessIO,
) -> Option<anyhow::Result<()>> {
    let fin_dir = crate::workflow::state::FinDir::new(cwd);

    match prompt {
        "/init" => {
            let result = fin_dir.init();
            let event = match &result {
                Ok(()) => {
                    let mut e = HeadlessEvent::new("init_ok");
                    e.text = Some("Initialized .fin/ directory".into());
                    e
                }
                Err(err) => {
                    let mut e = HeadlessEvent::new("error");
                    e.text = Some(format!("{err}"));
                    e.is_error = Some(true);
                    e
                }
            };
            let _ = serde_json::to_string(&event).map(|j| println!("{j}"));
            Some(result.map(|_| ()))
        }

        "/status" => {
            let status = fin_dir.active_blueprint_status();
            let data = match &status {
                crate::workflow::state::BlueprintStatus::Idle => {
                    serde_json::json!({"status": "idle"})
                }
                crate::workflow::state::BlueprintStatus::InProgress {
                    id,
                    stage,
                    section,
                    task,
                } => {
                    let snap = fin_dir.progress_snapshot(id);
                    serde_json::json!({
                        "status": "in_progress",
                        "id": id,
                        "stage": stage,
                        "section": section,
                        "task": task,
                        "sections_total": snap.sections_total,
                        "sections_done": snap.sections_done,
                        "tasks_total": snap.tasks_total,
                        "tasks_done": snap.tasks_done,
                    })
                }
                crate::workflow::state::BlueprintStatus::Complete(id) => {
                    serde_json::json!({"status": "complete", "id": id})
                }
            };
            let mut e = HeadlessEvent::new("workflow_status");
            e.text = Some(data.to_string());
            let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
            Some(Ok(()))
        }

        "/next" | "/auto" => {
            // /next is context-aware: workflow dispatch if blueprint active,
            // otherwise no-op in headless (no conversation state to hand off).
            if prompt == "/next" {
                let status = fin_dir.active_blueprint_status();
                if !matches!(
                    status,
                    crate::workflow::state::BlueprintStatus::InProgress { .. }
                ) {
                    let mut e = HeadlessEvent::new("info");
                    e.text = Some(
                        "No active blueprint. /next hands off context in TUI mode; \
                         in headless mode there is no persistent state to carry forward."
                            .into(),
                    );
                    let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
                    return Some(Ok(()));
                }
            }

            let mode = if prompt == "/auto" {
                crate::workflow::auto_loop::LoopMode::Auto
            } else {
                crate::workflow::auto_loop::LoopMode::Step
            };

            let model = match crate::io::print::pick_model(None) {
                Ok(m) => m,
                Err(e) => return Some(Err(e)),
            };

            let client = reqwest::Client::new();
            let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
            let provider = match provider_registry.get(&model.provider) {
                Some(p) => p,
                None => {
                    return Some(Err(anyhow::anyhow!(
                        "Provider not found: {}",
                        model.provider
                    )));
                }
            };

            let cancel = tokio_util::sync::CancellationToken::new();
            let result = crate::workflow::auto_loop::run_loop(
                cwd,
                &model,
                provider,
                mode,
                cancel,
                Some(Arc::clone(&provider_registry)),
                io,
            )
            .await;

            let mut e = HeadlessEvent::new("dispatch_result");
            e.text = Some(
                serde_json::json!({
                    "units_run": result.units_run,
                    "outcome": format!("{:?}", result.outcome),
                })
                .to_string(),
            );
            let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
            Some(Ok(()))
        }

        "/blueprint list" => {
            let listing = fin_dir.list_blueprints_display();
            let mut e = HeadlessEvent::new("blueprint_list");
            e.text = Some(listing);
            let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
            Some(Ok(()))
        }

        "/blueprint complete" => {
            match crate::workflow::commands::cmd_blueprint_complete(cwd).await {
                Ok(()) => {
                    let mut e = HeadlessEvent::new("blueprint_completed");
                    e.text = Some("Blueprint completed successfully.".into());
                    let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
                }
                Err(err) => {
                    let mut e = HeadlessEvent::new("error");
                    e.text = Some(format!("{err}"));
                    e.is_error = Some(true);
                    let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
                }
            }
            Some(Ok(()))
        }

        _ if prompt.starts_with("/blueprint ") => {
            let name = prompt.strip_prefix("/blueprint ").unwrap().trim();
            if name.is_empty() {
                let mut e = HeadlessEvent::new("error");
                e.text = Some("Blueprint name required".into());
                e.is_error = Some(true);
                let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
                return Some(Ok(()));
            }

            if !fin_dir.exists() {
                if let Err(e) = fin_dir.init() {
                    let mut err = HeadlessEvent::new("error");
                    err.text = Some(format!("Failed to initialize .fin/: {e}"));
                    err.is_error = Some(true);
                    let _ = serde_json::to_string(&err).map(|j| println!("{j}"));
                    return Some(Ok(()));
                }
            }

            let blueprints = fin_dir.list_blueprints();
            let id = format!("B{:03}", blueprints.len() + 1);
            let _ = fin_dir.create_blueprint(&id);
            let vision = crate::workflow::markdown::blueprint_vision(&id, name, "");
            let _ = std::fs::write(fin_dir.blueprint_vision(&id), &vision);
            let status_md = crate::workflow::markdown::status_template(
                &format!("{id} — {name}"),
                None,
                None,
                "define",
                "Blueprint created. Ready for define stage.",
            );
            let _ = fin_dir.write_state(&status_md);

            let mut e = HeadlessEvent::new("blueprint_created");
            e.text = Some(serde_json::json!({"id": id, "name": name}).to_string());
            let _ = serde_json::to_string(&e).map(|j| println!("{j}"));
            Some(Ok(()))
        }

        _ => None, // Not a workflow command
    }
}
