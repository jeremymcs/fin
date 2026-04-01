// Fin — RPC Mode (Persistent JSONL Protocol on stdin/stdout)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>
//
// Protocol: read JSONL commands from stdin, stream JSONL events to stdout.
// Unlike headless mode (single prompt), RPC keeps the session alive for
// multiple prompts — the programmatic equivalent of the TUI.
//
// Commands:
//   {"type":"prompt","message":"..."}        — send a prompt
//   {"type":"steer","message":"..."}         — inject a steering message
//   {"type":"clear"}                         — fresh context window
//   {"type":"set_model","model":"..."}       — switch model
//   {"type":"get_state"}                     — query session state
//   {"type":"quit"}                          — exit
//
// Workflow commands:
//   {"type":"init"}                          — initialize .fin/ directory
//   {"type":"blueprint_create","name":"..."}  — create a new blueprint
//   {"type":"blueprint_status"}              — get active blueprint status
//   {"type":"dispatch_next"}                 — run next dispatch unit (step)
//   {"type":"dispatch_auto"}                 — run all units to completion
//   {"type":"workflow_status"}               — get workflow progress snapshot

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::{AgentRegistry, DelegateTool};
use crate::config::paths::FinPaths;
use crate::db::session::SessionStore;
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::models::resolve_model;
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::*;
use crate::tools::ToolRegistry;
use crate::workflow::state::FinDir;

#[derive(Deserialize)]
struct RpcCommand {
    #[serde(rename = "type")]
    cmd_type: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    model: Option<String>,
    /// Blueprint name for blueprint_create.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Serialize)]
struct RpcEvent {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<String>,
    /// Structured data payload for workflow responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl RpcEvent {
    fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            text: None,
            tool_name: None,
            is_error: None,
            input_tokens: None,
            output_tokens: None,
            session_id: None,
            message_count: None,
            model_id: None,
            data: None,
        }
    }

    fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("{json}");
        }
    }
}

struct RpcIO;

#[async_trait::async_trait]
impl AgentIO for RpcIO {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        let rpc = match event {
            AgentEvent::TextDelta { text } => {
                let mut e = RpcEvent::new("text_delta");
                e.text = Some(text);
                e
            }
            AgentEvent::ThinkingDelta { text } => {
                let mut e = RpcEvent::new("thinking_delta");
                e.text = Some(text);
                e
            }
            AgentEvent::ToolStart { name, .. } => {
                let mut e = RpcEvent::new("tool_start");
                e.tool_name = Some(name);
                e
            }
            AgentEvent::ToolEnd { name, is_error, .. } => {
                let mut e = RpcEvent::new("tool_end");
                e.tool_name = Some(name);
                e.is_error = Some(is_error);
                e
            }
            AgentEvent::AgentStart { session_id } => {
                let mut e = RpcEvent::new("agent_start");
                e.session_id = Some(session_id);
                e
            }
            AgentEvent::AgentEnd { usage } => {
                let mut e = RpcEvent::new("agent_end");
                e.input_tokens = Some(usage.input_tokens);
                e.output_tokens = Some(usage.output_tokens);
                e
            }
            AgentEvent::TurnStart => RpcEvent::new("turn_start"),
            AgentEvent::TurnEnd => RpcEvent::new("turn_end"),
            AgentEvent::ModelChanged { ref display_name } => {
                let mut e = RpcEvent::new("model_changed");
                e.text = Some(display_name.clone());
                e
            }
            AgentEvent::WorkflowUnitStart {
                ref blueprint_id,
                ref section_id,
                ref task_id,
                ref stage,
                ref unit_type,
            } => {
                let mut e = RpcEvent::new("workflow_unit_start");
                e.data = Some(serde_json::json!({
                    "blueprint": blueprint_id,
                    "section": section_id,
                    "task": task_id,
                    "stage": stage,
                    "unit_type": unit_type,
                }));
                e
            }
            AgentEvent::WorkflowUnitEnd {
                ref blueprint_id,
                ref stage,
                ref artifacts,
                ..
            } => {
                let mut e = RpcEvent::new("workflow_unit_end");
                e.data = Some(serde_json::json!({
                    "blueprint": blueprint_id,
                    "stage": stage,
                    "artifacts": artifacts,
                }));
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
                let mut e = RpcEvent::new("workflow_progress");
                e.data = Some(serde_json::json!({
                    "blueprint": blueprint_id,
                    "sections_done": sections_done,
                    "sections_total": sections_total,
                    "tasks_done": tasks_done,
                    "tasks_total": tasks_total,
                }));
                e
            }
            AgentEvent::WorkflowComplete {
                ref blueprint_id,
                units_run,
            } => {
                let mut e = RpcEvent::new("workflow_complete");
                e.data = Some(serde_json::json!({
                    "blueprint": blueprint_id,
                    "units_run": units_run,
                }));
                e
            }
            AgentEvent::WorkflowBlocked {
                ref blueprint_id,
                ref reason,
            } => {
                let mut e = RpcEvent::new("workflow_blocked");
                e.data = Some(serde_json::json!({
                    "blueprint": blueprint_id,
                    "reason": reason,
                }));
                e
            }
            AgentEvent::WorkflowError { ref message } => {
                let mut e = RpcEvent::new("workflow_error");
                e.text = Some(message.clone());
                e.is_error = Some(true);
                e
            }
            AgentEvent::StageTransition { ref from, ref to } => {
                let mut e = RpcEvent::new("workflow_stage_transition");
                e.data = Some(serde_json::json!({"from": from, "to": to}));
                e
            }
        };
        rpc.emit();
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

/// Run the RPC server — persistent JSONL session on stdin/stdout.
pub async fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let model = crate::io::print::pick_default_model()?;

    // Build shared infrastructure
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

    // Persistent agent state
    let mut state = AgentState::new(model.clone(), cwd.clone());
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;

    let io = RpcIO;
    let cancel = CancellationToken::new();

    // Session store
    let session_store = FinPaths::resolve()
        .ok()
        .and_then(|p| SessionStore::new(&p.sessions_dir).ok());

    // Emit ready event
    let mut ready = RpcEvent::new("ready");
    ready.session_id = Some(state.session_id.clone());
    ready.model_id = Some(model.id.clone());
    ready.emit();

    // Read JSONL commands from stdin
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let cmd: RpcCommand = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                let mut err = RpcEvent::new("error");
                err.text = Some(format!("Invalid JSON: {e}"));
                err.emit();
                continue;
            }
        };

        match cmd.cmd_type.as_str() {
            "prompt" => {
                if cmd.message.is_empty() {
                    let mut err = RpcEvent::new("error");
                    err.text = Some("Empty prompt".into());
                    err.emit();
                    continue;
                }

                state.messages.push(Message::new_user(&cmd.message));
                let msgs_before = state.messages.len() - 1;

                let provider = match provider_registry.get(&state.model.provider) {
                    Some(p) => p,
                    None => {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("Provider not found: {}", state.model.provider));
                        err.emit();
                        continue;
                    }
                };

                if let Err(e) = run_agent_loop(&mut state, provider, &io, cancel.clone()).await {
                    let mut err = RpcEvent::new("error");
                    err.text = Some(format!("{e}"));
                    err.emit();
                }

                // Persist new messages
                if let Some(ref store) = session_store {
                    for msg in &state.messages[msgs_before..] {
                        if let Err(e) = store.append(&state.session_id, msg) {
                            tracing::warn!("Session persist failed: {e}");
                        }
                    }
                }
            }

            "steer" => {
                if !cmd.message.is_empty() {
                    state.messages.push(Message::new_user(&cmd.message));
                    let mut ack = RpcEvent::new("steering_ack");
                    ack.text = Some("Steering message injected".into());
                    ack.emit();
                }
            }

            "clear" => {
                state.messages.clear();
                state.cumulative_usage = Usage::default();
                state.session_id = uuid::Uuid::new_v4().to_string();

                let mut ack = RpcEvent::new("cleared");
                ack.session_id = Some(state.session_id.clone());
                ack.emit();
            }

            "set_model" => {
                if let Some(model_id) = cmd.model.as_deref() {
                    match resolve_model(model_id) {
                        Some(new_model) => {
                            state.model = new_model.clone();
                            let mut ack = RpcEvent::new("model_changed");
                            ack.model_id = Some(new_model.id);
                            ack.emit();
                        }
                        None => {
                            let mut err = RpcEvent::new("error");
                            err.text = Some(format!("Model not found: {model_id}"));
                            err.emit();
                        }
                    }
                }
            }

            "get_state" => {
                let mut evt = RpcEvent::new("state");
                evt.session_id = Some(state.session_id.clone());
                evt.message_count = Some(state.messages.len());
                evt.model_id = Some(state.model.id.clone());
                evt.input_tokens = Some(state.cumulative_usage.input_tokens);
                evt.output_tokens = Some(state.cumulative_usage.output_tokens);
                evt.emit();
            }

            // ── Workflow commands ────────────────────────────────────
            "init" => {
                let fin_dir = FinDir::new(&cwd);
                match fin_dir.init() {
                    Ok(()) => {
                        let mut ack = RpcEvent::new("init_ok");
                        ack.text = Some("Initialized .fin/ directory".into());
                        ack.emit();
                    }
                    Err(e) => {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("Init failed: {e}"));
                        err.emit();
                    }
                }
            }

            "blueprint_create" => {
                let name = cmd
                    .name
                    .as_deref()
                    .or(if cmd.message.is_empty() {
                        None
                    } else {
                        Some(cmd.message.as_str())
                    })
                    .unwrap_or("");

                if name.is_empty() {
                    let mut err = RpcEvent::new("error");
                    err.text = Some("Blueprint name required".into());
                    err.emit();
                    continue;
                }

                let fin_dir = FinDir::new(&cwd);
                if !fin_dir.exists() {
                    if let Err(e) = fin_dir.init() {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("Failed to initialize .fin/: {e}"));
                        err.emit();
                        continue;
                    }
                }

                let blueprints = fin_dir.list_blueprints();
                let id = format!("B{:03}", blueprints.len() + 1);

                if let Err(e) = fin_dir.create_blueprint(&id) {
                    let mut err = RpcEvent::new("error");
                    err.text = Some(format!("Create failed: {e}"));
                    err.emit();
                    continue;
                }

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

                let mut ack = RpcEvent::new("blueprint_created");
                ack.data = Some(serde_json::json!({
                    "id": id,
                    "name": name,
                }));
                ack.emit();
            }

            "blueprint_status" => {
                let fin_dir = FinDir::new(&cwd);
                let status = fin_dir.active_blueprint_status();
                let mut evt = RpcEvent::new("blueprint_status");
                match &status {
                    crate::workflow::state::BlueprintStatus::Idle => {
                        evt.data = Some(serde_json::json!({"status": "idle"}));
                    }
                    crate::workflow::state::BlueprintStatus::InProgress {
                        id,
                        stage,
                        section,
                        task,
                    } => {
                        evt.data = Some(serde_json::json!({
                            "status": "in_progress",
                            "id": id,
                            "stage": stage,
                            "section": section,
                            "task": task,
                        }));
                    }
                    crate::workflow::state::BlueprintStatus::Complete(id) => {
                        evt.data = Some(serde_json::json!({
                            "status": "complete",
                            "id": id,
                        }));
                    }
                }
                evt.emit();
            }

            "workflow_status" => {
                let fin_dir = FinDir::new(&cwd);
                let status = fin_dir.active_blueprint_status();
                let mut evt = RpcEvent::new("workflow_status");

                match &status {
                    crate::workflow::state::BlueprintStatus::InProgress {
                        id,
                        stage,
                        section,
                        task,
                    } => {
                        let snap = fin_dir.progress_snapshot(id);
                        evt.data = Some(serde_json::json!({
                            "active": true,
                            "blueprint_id": id,
                            "stage": stage,
                            "section": section,
                            "task": task,
                            "sections_total": snap.sections_total,
                            "sections_done": snap.sections_done,
                            "tasks_total": snap.tasks_total,
                            "tasks_done": snap.tasks_done,
                        }));
                    }
                    _ => {
                        evt.data = Some(serde_json::json!({"active": false}));
                    }
                }
                evt.emit();
            }

            "blueprint_complete" => {
                match crate::workflow::commands::cmd_blueprint_complete(&cwd).await {
                    Ok(()) => {
                        let mut evt = RpcEvent::new("blueprint_completed");
                        evt.text = Some("Blueprint completed successfully.".into());
                        evt.emit();
                    }
                    Err(e) => {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("{e}"));
                        err.emit();
                    }
                }
            }

            "blueprint_list" => {
                let fin_dir = FinDir::new(&cwd);
                let listing = fin_dir.list_blueprints_display();
                let mut evt = RpcEvent::new("blueprint_list");
                evt.text = Some(listing);
                evt.emit();
            }

            "dispatch_next" => {
                // Guard: require active blueprint for dispatch
                let fin_dir = FinDir::new(&cwd);
                if !matches!(
                    fin_dir.active_blueprint_status(),
                    crate::workflow::state::BlueprintStatus::InProgress { .. }
                ) {
                    let mut evt = RpcEvent::new("error");
                    evt.text = Some("No active blueprint. Create one first.".into());
                    evt.emit();
                    continue;
                }

                let provider = match provider_registry.get(&state.model.provider) {
                    Some(p) => p,
                    None => {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("Provider not found: {}", state.model.provider));
                        err.emit();
                        continue;
                    }
                };

                let result = crate::workflow::auto_loop::run_loop(
                    &cwd,
                    &state.model,
                    provider,
                    crate::workflow::auto_loop::LoopMode::Step,
                    cancel.clone(),
                    Some(Arc::clone(&provider_registry)),
                    &io,
                )
                .await;

                let mut evt = RpcEvent::new("dispatch_result");
                evt.data = Some(serde_json::json!({
                    "units_run": result.units_run,
                    "outcome": format!("{:?}", result.outcome),
                }));
                evt.emit();
            }

            "dispatch_auto" => {
                let provider = match provider_registry.get(&state.model.provider) {
                    Some(p) => p,
                    None => {
                        let mut err = RpcEvent::new("error");
                        err.text = Some(format!("Provider not found: {}", state.model.provider));
                        err.emit();
                        continue;
                    }
                };

                let result = crate::workflow::auto_loop::run_loop(
                    &cwd,
                    &state.model,
                    provider,
                    crate::workflow::auto_loop::LoopMode::Auto,
                    cancel.clone(),
                    Some(Arc::clone(&provider_registry)),
                    &io,
                )
                .await;

                let mut evt = RpcEvent::new("dispatch_result");
                evt.data = Some(serde_json::json!({
                    "units_run": result.units_run,
                    "outcome": format!("{:?}", result.outcome),
                }));
                evt.emit();
            }

            "quit" => {
                let mut bye = RpcEvent::new("quit");
                bye.session_id = Some(state.session_id.clone());
                bye.emit();
                break;
            }

            other => {
                let mut err = RpcEvent::new("error");
                err.text = Some(format!("Unknown command: {other}"));
                err.emit();
            }
        }
    }

    Ok(())
}
