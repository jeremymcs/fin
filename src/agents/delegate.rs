// Fin + Delegate Tool (Sub-Agent Spawning)

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::build_system_prompt;
use crate::agent::state::AgentState;
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::{Content, Message, Usage};
use crate::tools::{AgentTool, ToolRegistry, ToolResult};

use super::collector_io::collector_pair;
use super::registry::AgentRegistry;
use super::tier::resolve_model_tier;

/// Maximum nesting depth for delegated agents.
const MAX_DELEGATION_DEPTH: u32 = 3;

/// Maximum concurrent sub-agents in parallel mode.
const MAX_CONCURRENT_AGENTS: usize = 5;

/// Tool that delegates tasks to specialized sub-agents.
pub struct DelegateTool {
    agent_registry: Arc<AgentRegistry>,
    provider_registry: Arc<ProviderRegistry>,
    cwd: std::path::PathBuf,
    current_depth: u32,
}

impl DelegateTool {
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        provider_registry: Arc<ProviderRegistry>,
        cwd: std::path::PathBuf,
        current_depth: u32,
    ) -> Self {
        Self {
            agent_registry,
            provider_registry,
            cwd,
            current_depth,
        }
    }
}

#[async_trait]
impl AgentTool for DelegateTool {
    fn name(&self) -> &str {
        "delegate"
    }

    fn description(&self) -> &str {
        "Delegate a task to a specialized sub-agent. The sub-agent runs independently \
         with its own conversation, tools, and model, then returns its result. \
         Use the `parallel` parameter to run multiple agents concurrently."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "ID of the agent to delegate to (from available agents list)"
                },
                "task": {
                    "type": "string",
                    "description": "The task description / prompt for the sub-agent"
                },
                "parallel": {
                    "type": "array",
                    "description": "Array of {agent, task} objects for parallel delegation",
                    "items": {
                        "type": "object",
                        "required": ["agent", "task"],
                        "properties": {
                            "agent": {
                                "type": "string",
                                "description": "Agent ID"
                            },
                            "task": {
                                "type": "string",
                                "description": "Task for this agent"
                            }
                        }
                    }
                }
            }
        })
    }

    async fn execute(
        &self,
        _id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        // Check if parallel mode
        if let Some(parallel) = params.get("parallel").and_then(|p| p.as_array()) {
            return self.execute_parallel(parallel, cancel).await;
        }

        // Single delegation mode
        let agent_id = params
            .get("agent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: agent"))?;

        let task = params
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task"))?;

        match self.spawn_single(agent_id, task, cancel).await {
            Ok((text, usage)) => Ok(ToolResult {
                content: vec![Content::Text {
                    text: format_agent_result(agent_id, &text, &usage),
                }],
                is_error: false,
            }),
            Err(e) => Ok(ToolResult {
                content: vec![Content::Text {
                    text: format!("Agent '{agent_id}' failed: {e}"),
                }],
                is_error: true,
            }),
        }
    }
}

impl DelegateTool {
    async fn execute_parallel(
        &self,
        items: &[serde_json::Value],
        cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        if items.is_empty() {
            return Ok(ToolResult {
                content: vec![Content::Text {
                    text: "No agents specified in parallel array".to_string(),
                }],
                is_error: true,
            });
        }

        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_AGENTS));
        let mut handles = Vec::new();

        for item in items {
            let agent_id = item
                .get("agent")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let task = item
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let agent_registry = Arc::clone(&self.agent_registry);
            let provider_registry = Arc::clone(&self.provider_registry);
            let cwd = self.cwd.clone();
            let depth = self.current_depth;
            let child_cancel = cancel.child_token();
            let sem = Arc::clone(&semaphore);

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let result = spawn_sub_agent(
                    &agent_id,
                    &task,
                    agent_registry,
                    provider_registry,
                    cwd,
                    depth,
                    child_cancel,
                )
                .await;
                (agent_id, result)
            }));
        }

        // Collect all results
        let mut output = String::new();
        let mut any_error = false;

        for handle in handles {
            match handle.await {
                Ok((agent_id, Ok((text, usage)))) => {
                    output.push_str(&format_agent_result(&agent_id, &text, &usage));
                    output.push_str("\n---\n\n");
                }
                Ok((agent_id, Err(e))) => {
                    output.push_str(&format!("## Agent: {agent_id} (FAILED)\n\n{e}\n\n---\n\n"));
                    any_error = true;
                }
                Err(e) => {
                    output.push_str(&format!("## Agent task panicked: {e}\n\n---\n\n"));
                    any_error = true;
                }
            }
        }

        Ok(ToolResult {
            content: vec![Content::Text { text: output }],
            is_error: any_error,
        })
    }

    async fn spawn_single(
        &self,
        agent_id: &str,
        task: &str,
        cancel: CancellationToken,
    ) -> anyhow::Result<(String, Usage)> {
        spawn_sub_agent(
            agent_id,
            task,
            Arc::clone(&self.agent_registry),
            Arc::clone(&self.provider_registry),
            self.cwd.clone(),
            self.current_depth,
            cancel.child_token(),
        )
        .await
    }
}

/// Spawn a single sub-agent and return its collected output.
async fn spawn_sub_agent(
    agent_id: &str,
    task: &str,
    agent_registry: Arc<AgentRegistry>,
    provider_registry: Arc<ProviderRegistry>,
    cwd: std::path::PathBuf,
    depth: u32,
    cancel: CancellationToken,
) -> anyhow::Result<(String, Usage)> {
    // Check depth limit
    if depth >= MAX_DELEGATION_DEPTH {
        anyhow::bail!(
            "Maximum delegation depth ({MAX_DELEGATION_DEPTH}) reached. \
             Cannot delegate further."
        );
    }

    // Look up agent definition
    let agent_def = agent_registry
        .get(agent_id)
        .ok_or_else(|| anyhow::anyhow!("Agent not found: '{agent_id}'"))?
        .clone();

    // Resolve model for this agent
    let parent_provider = None; // Sub-agent uses its own tier preference
    let model = resolve_model_tier(&agent_def.model_tier, parent_provider).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not resolve model for tier '{}' (agent: {})",
            agent_def.model_tier,
            agent_id
        )
    })?;

    // Build filtered tool registry
    let mut tool_registry = ToolRegistry::filtered_defaults(&cwd, &agent_def.tools);

    // Add delegate tool if allowed and within depth limit
    if agent_def.tools.contains(&"delegate".to_string()) && depth + 1 < MAX_DELEGATION_DEPTH {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.clone(),
            depth + 1,
        )));
    }

    // Build system prompt: agent's role + filtered tool schemas
    let mut system_prompt = agent_def.system_prompt.clone();
    system_prompt.push_str("\n\n");

    // Append tool descriptions
    let tool_prompt = build_system_prompt(&tool_registry.schemas(), &cwd, None);
    // Extract just the tools and environment sections (skip the generic identity/guidelines)
    if let Some(tools_idx) = tool_prompt.find("# Available Tools") {
        system_prompt.push_str(&tool_prompt[tools_idx..]);
    }

    // Build agent state
    let mut state = AgentState::new(model.clone(), cwd);
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.messages.push(Message::new_user(task));

    // Create collector IO
    let (collector_io, receiver) = collector_pair();

    // Resolve provider
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    // Run the sub-agent loop
    run_agent_loop(&mut state, provider, &collector_io, cancel).await?;

    // Drop the sender side so the receiver can drain
    drop(collector_io);

    // Collect output
    let (text, mut usage) = receiver.collect().await;

    // Merge usage from state (in case collector missed some)
    usage.input_tokens = usage.input_tokens.max(state.cumulative_usage.input_tokens);
    usage.output_tokens = usage
        .output_tokens
        .max(state.cumulative_usage.output_tokens);

    Ok((text, usage))
}

fn format_agent_result(agent_id: &str, text: &str, usage: &Usage) -> String {
    format!(
        "## Agent: {agent_id}\n\n\
         {text}\n\n\
         *[tokens: {}in / {}out]*",
        usage.input_tokens, usage.output_tokens,
    )
}
