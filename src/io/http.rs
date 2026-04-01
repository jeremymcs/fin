// Fin — HTTP API Server (axum + SSE)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use axum::{
    Router,
    extract::State as AxumState,
    response::{Json, Sse, sse},
    routing::{get, post},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::{AgentRegistry, DelegateTool};
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::io::print_io::PrintIO;
use crate::llm::models::{default_models, resolve_model};
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::*;
use crate::tools::ToolRegistry;
use crate::workflow::state::FinDir;

struct AppState {
    provider_registry: Arc<ProviderRegistry>,
    agent_registry: Arc<AgentRegistry>,
}

pub async fn run(host: &str, port: u16) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let cwd = std::env::current_dir()?;
    let agent_registry = Arc::new(AgentRegistry::load_for_project(&cwd));

    let state = Arc::new(AppState {
        provider_registry,
        agent_registry,
    });

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/models", get(list_models))
        .route("/api/prompt", post(prompt_handler))
        .route("/api/prompt/stream", post(prompt_stream_handler))
        // Workflow endpoints
        .route("/api/init", post(init_handler))
        .route("/api/blueprint", post(blueprint_create_handler))
        .route("/api/blueprint/complete", post(blueprint_complete_handler))
        .route("/api/blueprint/list", get(blueprint_list_handler))
        .route("/api/blueprint/status", get(blueprint_status_handler))
        .route("/api/workflow/status", get(workflow_status_handler))
        .route("/api/dispatch/next", post(dispatch_next_handler))
        .route("/api/dispatch/auto", post(dispatch_auto_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    eprintln!("Fin HTTP server listening on http://{host}:{port}");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn list_models() -> Json<Vec<crate::llm::models::ModelConfig>> {
    Json(default_models())
}

#[derive(Deserialize)]
struct PromptRequest {
    prompt: String,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Serialize)]
struct PromptResponse {
    text: String,
    input_tokens: u64,
    output_tokens: u64,
}

/// Build tool registry with extensions and agent delegation.
fn build_tool_registry(
    cwd: &std::path::Path,
    agent_registry: &Arc<AgentRegistry>,
    provider_registry: &Arc<ProviderRegistry>,
) -> ToolRegistry {
    let mut tool_registry = ToolRegistry::with_defaults(cwd);
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }
    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(agent_registry),
            Arc::clone(provider_registry),
            cwd.to_path_buf(),
            0,
        )));
    }
    tool_registry
}

/// Build system prompt with agent context.
fn build_prompt(
    tool_registry: &ToolRegistry,
    cwd: &std::path::Path,
    agent_registry: &AgentRegistry,
) -> String {
    let agent_context = if !agent_registry.is_empty() {
        Some(AgentPromptContext {
            available_agents: Some(agent_registry.prompt_summary()),
            agent_role: None,
        })
    } else {
        None
    };
    build_system_prompt(&tool_registry.schemas(), cwd, agent_context.as_ref())
}

async fn prompt_handler(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(req): Json<PromptRequest>,
) -> Result<Json<PromptResponse>, axum::http::StatusCode> {
    let model = match req.model.as_deref() {
        Some(id) => resolve_model(id),
        None => crate::io::print::pick_model(None).ok(),
    }
    .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    let cwd = std::env::current_dir().unwrap_or_default();
    let tool_registry = build_tool_registry(&cwd, &state.agent_registry, &state.provider_registry);
    let system_prompt = build_prompt(&tool_registry, &cwd, &state.agent_registry);

    let mut agent_state = AgentState::new(model.clone(), cwd);
    agent_state.tool_registry = tool_registry;
    agent_state.system_prompt = system_prompt;
    agent_state.messages.push(Message::new_user(&req.prompt));

    let io = PrintIO::new(false, false);
    let provider = state
        .provider_registry
        .get(&model.provider)
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let cancel = tokio_util::sync::CancellationToken::new();
    run_agent_loop(&mut agent_state, provider, &io, cancel)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let text = agent_state
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)
        .map(|m| {
            m.content
                .iter()
                .filter_map(|c| match c {
                    Content::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    Ok(Json(PromptResponse {
        text,
        input_tokens: agent_state.cumulative_usage.input_tokens,
        output_tokens: agent_state.cumulative_usage.output_tokens,
    }))
}

async fn prompt_stream_handler(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(req): Json<PromptRequest>,
) -> Sse<impl Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::unbounded_channel::<AgentEvent>();
    let agent_registry = Arc::clone(&state.agent_registry);
    let provider_registry = Arc::clone(&state.provider_registry);

    tokio::spawn(async move {
        let model = match req.model.as_deref() {
            Some(id) => resolve_model(id),
            None => crate::io::print::pick_model(None).ok(),
        };

        let Some(model) = model else { return };

        let cwd = std::env::current_dir().unwrap_or_default();
        let tool_registry = build_tool_registry(&cwd, &agent_registry, &provider_registry);
        let system_prompt = build_prompt(&tool_registry, &cwd, &agent_registry);

        let mut agent_state = AgentState::new(model.clone(), cwd);
        agent_state.tool_registry = tool_registry;
        agent_state.system_prompt = system_prompt;
        agent_state.messages.push(Message::new_user(&req.prompt));

        let io = crate::io::channel_io::ChannelIO::new(tx);

        if let Some(provider) = provider_registry.get(&model.provider) {
            let cancel = tokio_util::sync::CancellationToken::new();
            let _ = run_agent_loop(&mut agent_state, provider, &io, cancel).await;
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    let sse_stream = futures::StreamExt::map(stream, |event| {
        let data = match &event {
            AgentEvent::TextDelta { text } => {
                serde_json::json!({"type": "text_delta", "text": text})
            }
            AgentEvent::ToolStart { name, .. } => {
                serde_json::json!({"type": "tool_start", "name": name})
            }
            AgentEvent::ToolEnd { name, is_error, .. } => {
                serde_json::json!({"type": "tool_end", "name": name, "is_error": is_error})
            }
            AgentEvent::AgentEnd { usage } => {
                serde_json::json!({"type": "agent_end", "input_tokens": usage.input_tokens, "output_tokens": usage.output_tokens})
            }
            AgentEvent::ThinkingDelta { text } => {
                serde_json::json!({"type": "thinking_delta", "text": text})
            }
            AgentEvent::WorkflowUnitStart {
                blueprint_id,
                section_id,
                task_id,
                stage,
                unit_type,
            } => {
                serde_json::json!({"type": "workflow_unit_start", "blueprint": blueprint_id, "section": section_id, "task": task_id, "stage": stage, "unit_type": unit_type})
            }
            AgentEvent::WorkflowUnitEnd {
                blueprint_id,
                stage,
                artifacts,
                ..
            } => {
                serde_json::json!({"type": "workflow_unit_end", "blueprint": blueprint_id, "stage": stage, "artifacts": artifacts})
            }
            AgentEvent::WorkflowProgress {
                blueprint_id,
                sections_total,
                sections_done,
                tasks_total,
                tasks_done,
                ..
            } => {
                serde_json::json!({"type": "workflow_progress", "blueprint": blueprint_id, "sections_done": sections_done, "sections_total": sections_total, "tasks_done": tasks_done, "tasks_total": tasks_total})
            }
            AgentEvent::WorkflowComplete {
                blueprint_id,
                units_run,
            } => {
                serde_json::json!({"type": "workflow_complete", "blueprint": blueprint_id, "units_run": units_run})
            }
            AgentEvent::WorkflowBlocked {
                blueprint_id,
                reason,
            } => {
                serde_json::json!({"type": "workflow_blocked", "blueprint": blueprint_id, "reason": reason})
            }
            AgentEvent::WorkflowError { message } => {
                serde_json::json!({"type": "workflow_error", "message": message})
            }
            AgentEvent::StageTransition { from, to } => {
                serde_json::json!({"type": "workflow_stage_transition", "from": from, "to": to})
            }
            _ => serde_json::json!({"type": "other"}),
        };
        Ok(sse::Event::default().data(data.to_string()))
    });

    Sse::new(sse_stream)
}

// ── Workflow endpoints ─────────────────────────────────────────────

async fn init_handler() -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let fin_dir = FinDir::new(&cwd);
    match fin_dir.init() {
        Ok(()) => Json(serde_json::json!({"ok": true, "message": "Initialized .fin/ directory"})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": format!("{e}")})),
    }
}

#[derive(Deserialize)]
struct BlueprintCreateRequest {
    name: String,
}

async fn blueprint_create_handler(
    Json(req): Json<BlueprintCreateRequest>,
) -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let fin_dir = FinDir::new(&cwd);

    if !fin_dir.exists() {
        if let Err(e) = fin_dir.init() {
            return Json(
                serde_json::json!({"ok": false, "error": format!("Failed to initialize .fin/: {e}")}),
            );
        }
    }

    let blueprints = fin_dir.list_blueprints();
    let id = format!("B{:03}", blueprints.len() + 1);

    if let Err(e) = fin_dir.create_blueprint(&id) {
        return Json(serde_json::json!({"ok": false, "error": format!("{e}")}));
    }

    let vision = crate::workflow::markdown::blueprint_vision(&id, &req.name, "");
    let _ = std::fs::write(fin_dir.blueprint_vision(&id), &vision);

    let status_md = crate::workflow::markdown::status_template(
        &format!("{id} — {}", req.name),
        None,
        None,
        "define",
        "Blueprint created. Ready for define stage.",
    );
    let _ = fin_dir.write_state(&status_md);

    Json(serde_json::json!({"ok": true, "id": id, "name": req.name}))
}

async fn blueprint_complete_handler() -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    match crate::workflow::commands::cmd_blueprint_complete(&cwd).await {
        Ok(()) => Json(serde_json::json!({"ok": true, "message": "Blueprint completed"})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": format!("{e}")})),
    }
}

async fn blueprint_list_handler() -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let fin_dir = FinDir::new(&cwd);
    let listing = fin_dir.list_blueprints_display();
    Json(serde_json::json!({"blueprints": listing}))
}

async fn blueprint_status_handler() -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let fin_dir = FinDir::new(&cwd);
    let status = fin_dir.active_blueprint_status();

    match &status {
        crate::workflow::state::BlueprintStatus::Idle => {
            Json(serde_json::json!({"status": "idle"}))
        }
        crate::workflow::state::BlueprintStatus::InProgress {
            id,
            stage,
            section,
            task,
        } => Json(serde_json::json!({
            "status": "in_progress",
            "id": id,
            "stage": stage,
            "section": section,
            "task": task,
        })),
        crate::workflow::state::BlueprintStatus::Complete(id) => {
            Json(serde_json::json!({"status": "complete", "id": id}))
        }
    }
}

async fn workflow_status_handler() -> Json<serde_json::Value> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let fin_dir = FinDir::new(&cwd);
    let status = fin_dir.active_blueprint_status();

    match &status {
        crate::workflow::state::BlueprintStatus::InProgress {
            id,
            stage,
            section,
            task,
        } => {
            let snap = fin_dir.progress_snapshot(id);
            Json(serde_json::json!({
                "active": true,
                "blueprint_id": id,
                "stage": stage,
                "section": section,
                "task": task,
                "sections_total": snap.sections_total,
                "sections_done": snap.sections_done,
                "tasks_total": snap.tasks_total,
                "tasks_done": snap.tasks_done,
            }))
        }
        _ => Json(serde_json::json!({"active": false})),
    }
}

#[derive(Deserialize)]
struct DispatchRequest {
    #[serde(default)]
    model: Option<String>,
}

async fn dispatch_next_handler(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(req): Json<DispatchRequest>,
) -> Sse<impl Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    dispatch_handler(state, req, crate::workflow::auto_loop::LoopMode::Step).await
}

async fn dispatch_auto_handler(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(req): Json<DispatchRequest>,
) -> Sse<impl Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    dispatch_handler(state, req, crate::workflow::auto_loop::LoopMode::Auto).await
}

async fn dispatch_handler(
    state: Arc<AppState>,
    req: DispatchRequest,
    mode: crate::workflow::auto_loop::LoopMode,
) -> Sse<impl Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::unbounded_channel::<AgentEvent>();
    let provider_registry = Arc::clone(&state.provider_registry);

    tokio::spawn(async move {
        let model = match req.model.as_deref() {
            Some(id) => resolve_model(id),
            None => crate::io::print::pick_model(None).ok(),
        };

        let Some(model) = model else { return };

        let cwd = std::env::current_dir().unwrap_or_default();
        let io = crate::io::channel_io::ChannelIO::new(tx);

        if let Some(provider) = provider_registry.get(&model.provider) {
            let cancel = tokio_util::sync::CancellationToken::new();
            let result = crate::workflow::auto_loop::run_loop(
                &cwd,
                &model,
                provider,
                mode,
                cancel,
                Some(Arc::clone(&provider_registry)),
                &io,
            )
            .await;

            // Emit final result as a text event
            let _ = io
                .emit(AgentEvent::TextDelta {
                    text: format!(
                        "\nDispatch complete: {} units, {:?}\n",
                        result.units_run, result.outcome
                    ),
                })
                .await;
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    let sse_stream = futures::StreamExt::map(stream, |event| {
        let data = match &event {
            AgentEvent::TextDelta { text } => {
                serde_json::json!({"type": "text_delta", "text": text})
            }
            AgentEvent::WorkflowUnitStart {
                blueprint_id,
                section_id,
                task_id,
                stage,
                unit_type,
            } => {
                serde_json::json!({"type": "workflow_unit_start", "blueprint": blueprint_id, "section": section_id, "task": task_id, "stage": stage, "unit_type": unit_type})
            }
            AgentEvent::WorkflowUnitEnd {
                blueprint_id,
                stage,
                artifacts,
                ..
            } => {
                serde_json::json!({"type": "workflow_unit_end", "blueprint": blueprint_id, "stage": stage, "artifacts": artifacts})
            }
            AgentEvent::WorkflowProgress {
                blueprint_id,
                sections_total,
                sections_done,
                tasks_total,
                tasks_done,
                ..
            } => {
                serde_json::json!({"type": "workflow_progress", "blueprint": blueprint_id, "sections_done": sections_done, "sections_total": sections_total, "tasks_done": tasks_done, "tasks_total": tasks_total})
            }
            AgentEvent::WorkflowComplete {
                blueprint_id,
                units_run,
            } => {
                serde_json::json!({"type": "workflow_complete", "blueprint": blueprint_id, "units_run": units_run})
            }
            AgentEvent::WorkflowBlocked {
                blueprint_id,
                reason,
            } => {
                serde_json::json!({"type": "workflow_blocked", "blueprint": blueprint_id, "reason": reason})
            }
            AgentEvent::WorkflowError { message } => {
                serde_json::json!({"type": "workflow_error", "message": message})
            }
            AgentEvent::ToolStart { name, .. } => {
                serde_json::json!({"type": "tool_start", "name": name})
            }
            AgentEvent::ToolEnd { name, is_error, .. } => {
                serde_json::json!({"type": "tool_end", "name": name, "is_error": is_error})
            }
            AgentEvent::StageTransition { from, to } => {
                serde_json::json!({"type": "workflow_stage_transition", "from": from, "to": to})
            }
            _ => serde_json::json!({"type": "other"}),
        };
        Ok(sse::Event::default().data(data.to_string()))
    });

    Sse::new(sse_stream)
}
