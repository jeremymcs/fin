// Fin — Stage Runner Trait, Context, and Implementations
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

pub mod advance;
pub mod architect;
pub mod build;
pub mod define;
pub mod explore;
pub mod seal_section;
pub mod validate;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::Stage;
use super::state::FinDir;
use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::DelegateTool;
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::{LlmProvider, ProviderRegistry};
use crate::llm::types::Message;
use crate::tools::ToolRegistry;

/// Artifact produced by a stage.
pub struct StageArtifact {
    pub path: PathBuf,
    pub description: String,
}

/// Context loaded for stage execution.
pub struct StageContext {
    pub blueprint_id: String,
    pub section_id: Option<String>,
    pub task_id: Option<String>,
    pub stage: Stage,
    pub status_md: String,
    pub ledger_md: Option<String>,
    pub brief_md: Option<String>,
    pub findings_md: Option<String>,
    pub vision_md: Option<String>,
    pub section_spec_md: Option<String>,
    pub task_spec_md: Option<String>,
    pub summaries: Vec<(String, String)>,
    /// Codebase map from `fin map` — structural overview of the project.
    /// Injected into planning stages to reduce exploratory tool calls.
    pub codebase_map_md: Option<String>,
    /// Workflow-safe agents (fin-* only) from .fin/agents/.
    /// Stages can delegate sub-tasks to these agents.
    pub workflow_agents: crate::agents::AgentRegistry,
    /// Provider registry for sub-agent delegation.
    /// None when running in contexts where delegation isn't available.
    pub provider_registry: Option<Arc<ProviderRegistry>>,
}

impl StageContext {
    /// Build from current FinDir state and position.
    pub fn load(
        fin_dir: &FinDir,
        blueprint_id: &str,
        section_id: Option<&str>,
        task_id: Option<&str>,
        stage: Stage,
    ) -> Self {
        let status_md = fin_dir.read_state().unwrap_or_default();
        let ledger_md = std::fs::read_to_string(fin_dir.ledger_path()).ok();
        let brief_md = std::fs::read_to_string(fin_dir.blueprint_brief(blueprint_id)).ok();
        let findings_md = std::fs::read_to_string(fin_dir.blueprint_findings(blueprint_id)).ok();
        let vision_md = std::fs::read_to_string(fin_dir.blueprint_vision(blueprint_id)).ok();

        let (section_spec_md, task_spec_md) = match (section_id, task_id) {
            (Some(s), Some(t)) => (
                std::fs::read_to_string(fin_dir.section_spec(blueprint_id, s)).ok(),
                std::fs::read_to_string(fin_dir.task_spec(blueprint_id, s, t)).ok(),
            ),
            (Some(s), None) => (
                std::fs::read_to_string(fin_dir.section_spec(blueprint_id, s)).ok(),
                None,
            ),
            _ => (None, None),
        };

        // Load completed task summaries for the current section
        let mut summaries = Vec::new();
        if let Some(s) = section_id {
            let tasks = fin_dir.list_tasks(blueprint_id, s);
            for t in &tasks {
                let report_path = fin_dir.task_report(blueprint_id, s, t);
                if let Ok(content) = std::fs::read_to_string(&report_path) {
                    summaries.push((t.clone(), content));
                }
            }
        }

        // Load codebase map if it exists (written by `fin map`)
        let codebase_map_md = std::fs::read_to_string(fin_dir.map_path()).ok();

        // Load workflow agents from .fin/agents/ (fin-* only)
        let project_root = fin_dir.root().parent().unwrap_or(fin_dir.root());
        let workflow_agents = crate::agents::AgentRegistry::load_workflow_agents(project_root);

        Self {
            blueprint_id: blueprint_id.to_string(),
            section_id: section_id.map(String::from),
            task_id: task_id.map(String::from),
            stage,
            status_md,
            ledger_md,
            brief_md,
            findings_md,
            vision_md,
            section_spec_md,
            task_spec_md,
            summaries,
            codebase_map_md,
            workflow_agents,
            provider_registry: None,
        }
    }

    /// Find a workflow-safe agent for a given role (e.g. "researcher").
    /// Returns the first matching fin-* agent, or None.
    pub fn find_agent_for_role(
        &self,
        role: &str,
    ) -> Option<&crate::agents::definition::AgentDefinition> {
        self.workflow_agents
            .find_workflow_role(role)
            .into_iter()
            .next()
    }

    /// Build a stage-aware context string for system prompt injection.
    /// Each stage gets only the upstream artifacts it needs — no more, no less.
    pub fn context_for_prompt(&self) -> String {
        let mut ctx = String::new();

        match self.stage {
            Stage::Define => {
                // Define: inject codebase map so the agent starts oriented.
                // Without the map, Define burns several API turns on exploratory globs/reads.
                self.inject_codebase_map(&mut ctx);
            }
            Stage::Explore => {
                // Explore needs: codebase map + BRIEF.md + LEDGER.md
                self.inject_codebase_map(&mut ctx);
                self.inject_brief(&mut ctx);
                self.inject_ledger(&mut ctx);
            }
            Stage::Architect => {
                if self.section_id.is_some() {
                    // Section planning: codebase map + VISION + BRIEF + FINDINGS + dependency summaries
                    self.inject_codebase_map(&mut ctx);
                    self.inject_vision(&mut ctx);
                    self.inject_brief(&mut ctx);
                    self.inject_findings(&mut ctx);
                    self.inject_ledger(&mut ctx);
                    self.inject_summaries_full(&mut ctx);
                } else {
                    // Blueprint planning: codebase map + BRIEF + FINDINGS + LEDGER
                    self.inject_codebase_map(&mut ctx);
                    self.inject_brief(&mut ctx);
                    self.inject_findings(&mut ctx);
                    self.inject_ledger(&mut ctx);
                }
            }
            Stage::Build => {
                // Build: task spec + section spec validation section + carry-forward summaries
                self.inject_task_spec(&mut ctx);
                self.inject_section_validation(&mut ctx);
                self.inject_summaries_compressed(&mut ctx);
            }
            Stage::Validate => {
                // Validate: task spec + section spec validation section
                self.inject_task_spec(&mut ctx);
                self.inject_section_validation(&mut ctx);
            }
            Stage::SealSection => {
                // SealSection: section spec + all task summaries + VISION
                self.inject_section_spec(&mut ctx);
                self.inject_summaries_full(&mut ctx);
                self.inject_vision(&mut ctx);
            }
            Stage::Advance => {
                // Advance is pure logic — no context needed
            }
        }

        ctx
    }

    // ── Context injection helpers ─────────────────────────────────

    fn inject_codebase_map(&self, ctx: &mut String) {
        if let Some(ref map) = self.codebase_map_md {
            const MAP_LIMIT: usize = 6000;
            let content = if map.len() > MAP_LIMIT {
                &map[..MAP_LIMIT]
            } else {
                map.as_str()
            };
            ctx.push_str("## Codebase Map\n\n");
            ctx.push_str(content);
            if map.len() > MAP_LIMIT {
                ctx.push_str("\n\n*(map truncated — run `fin map` to refresh)*");
            }
            ctx.push_str("\n\n");
        }
    }

    fn inject_ledger(&self, ctx: &mut String) {
        if let Some(ref ledger) = self.ledger_md {
            if ledger.len() < 4000 {
                ctx.push_str("## Decisions Ledger\n\n");
                ctx.push_str(ledger);
                ctx.push_str("\n\n");
            }
        }
    }

    fn inject_brief(&self, ctx: &mut String) {
        if let Some(ref brief) = self.brief_md {
            ctx.push_str("## Brief (from define stage)\n\n");
            ctx.push_str(brief);
            ctx.push_str("\n\n");
        }
    }

    fn inject_findings(&self, ctx: &mut String) {
        if let Some(ref findings) = self.findings_md {
            let truncated = if findings.len() > 4000 {
                &findings[..4000]
            } else {
                findings
            };
            ctx.push_str("## Findings\n\n");
            ctx.push_str(truncated);
            ctx.push_str("\n\n");
        }
    }

    fn inject_vision(&self, ctx: &mut String) {
        if let Some(ref vision) = self.vision_md {
            ctx.push_str("## Vision\n\n");
            ctx.push_str(vision);
            ctx.push_str("\n\n");
        }
    }

    fn inject_section_spec(&self, ctx: &mut String) {
        if let Some(ref spec) = self.section_spec_md {
            ctx.push_str("## Current Section Spec\n\n");
            ctx.push_str(spec);
            ctx.push_str("\n\n");
        }
    }

    fn inject_task_spec(&self, ctx: &mut String) {
        if let Some(ref task_spec) = self.task_spec_md {
            ctx.push_str("## Current Task Spec\n\n");
            ctx.push_str(task_spec);
            ctx.push_str("\n\n");
        }
    }

    /// Inject only the Validation section from the section spec.
    fn inject_section_validation(&self, ctx: &mut String) {
        if let Some(ref spec) = self.section_spec_md {
            if let Some(section) = extract_section(spec, "Validation") {
                ctx.push_str("## Section Validation Requirements\n\n");
                ctx.push_str(&section);
                ctx.push_str("\n\n");
            }
        }
    }

    /// Full task summaries — used by SealSection and Architect stages.
    fn inject_summaries_full(&self, ctx: &mut String) {
        if !self.summaries.is_empty() {
            ctx.push_str("## Completed Task Summaries\n\n");
            for (id, summary) in &self.summaries {
                let truncated = if summary.len() > 2000 {
                    &summary[..2000]
                } else {
                    summary
                };
                ctx.push_str(&format!("### {id}\n{truncated}\n\n"));
            }
        }
    }

    /// Compressed carry-forward summaries for Build stage.
    /// Extracts key fields from YAML frontmatter + one-liner for each prior task.
    fn inject_summaries_compressed(&self, ctx: &mut String) {
        if !self.summaries.is_empty() {
            ctx.push_str("## Prior Task Context (carry-forward)\n\n");
            for (id, summary) in &self.summaries {
                ctx.push_str(&compress_summary(id, summary));
                ctx.push('\n');
            }
        }
    }
}

// ── Context extraction helpers ────────────────────────────────

/// Extract content under a `## Heading` until the next `##` heading or end of string.
fn extract_section(markdown: &str, heading: &str) -> Option<String> {
    let target = format!("## {heading}");
    let mut lines = markdown.lines();
    let mut found = false;
    let mut content = String::new();

    for line in &mut lines {
        if found {
            if line.starts_with("## ") {
                break;
            }
            content.push_str(line);
            content.push('\n');
        } else if line.trim().starts_with(&target) {
            found = true;
        }
    }

    if found && !content.trim().is_empty() {
        Some(content.trim().to_string())
    } else {
        None
    }
}

/// Compress a task summary into a compact carry-forward block.
/// Extracts YAML frontmatter fields (provides, key_decisions, patterns_established,
/// key_files) plus the one-liner for downstream task context.
fn compress_summary(task_id: &str, summary: &str) -> String {
    let mut result = format!("### {task_id}\n");

    // Extract one-liner (first bold line after frontmatter)
    let content = if summary.contains("---") {
        // Skip YAML frontmatter
        let parts: Vec<&str> = summary.splitn(3, "---").collect();
        if parts.len() >= 3 { parts[2] } else { summary }
    } else {
        summary
    };

    // Find one-liner (first non-empty, non-heading line after frontmatter)
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            result.push_str(&format!("**Summary:** {trimmed}\n"));
            break;
        }
    }

    // Extract YAML frontmatter fields
    if let Some(frontmatter) = extract_yaml_frontmatter(summary) {
        for field in &[
            "provides",
            "key_decisions",
            "patterns_established",
            "key_files",
        ] {
            if let Some(values) = extract_yaml_list(&frontmatter, field) {
                if !values.is_empty() {
                    result.push_str(&format!("**{field}:** {}\n", values.join(", ")));
                }
            }
        }
    }

    result
}

/// Extract the YAML frontmatter block from a markdown string.
fn extract_yaml_frontmatter(markdown: &str) -> Option<String> {
    let trimmed = markdown.trim();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("---")?;
    Some(rest[..end].to_string())
}

/// Extract a YAML list field (simple parser — handles `  - value` format).
fn extract_yaml_list(yaml: &str, field: &str) -> Option<Vec<String>> {
    let mut values = Vec::new();
    let mut in_field = false;
    let target = format!("{}:", field);

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&target) {
            in_field = true;
            // Check for inline value (field: value)
            let after = trimmed[target.len()..].trim();
            if !after.is_empty() && !after.starts_with('[') {
                values.push(after.to_string());
                in_field = false;
            }
            continue;
        }
        if in_field {
            if let Some(val) = trimmed.strip_prefix("- ") {
                values.push(val.trim().to_string());
            } else if !trimmed.is_empty() {
                // Hit a non-list line — end of this field
                break;
            }
        }
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Trait that each stage implements.
#[async_trait]
pub trait StageRunner: Send + Sync {
    /// Which stage this runner handles.
    fn stage(&self) -> Stage;

    /// System prompt fragment for this stage.
    fn system_prompt(&self, ctx: &StageContext) -> String;

    /// Initial user message that kicks off the stage.
    fn initial_message(&self, ctx: &StageContext) -> String;

    /// Tool restriction: which tools are available during this stage.
    /// None means all tools available.
    fn allowed_tools(&self) -> Option<Vec<String>>;

    /// Workflow roles this stage can delegate sub-tasks to.
    /// Empty means no delegation — stage runs inline only.
    fn delegatable_roles(&self) -> Vec<&str> {
        vec![]
    }

    /// Run the stage using the agent loop.
    async fn run(
        &self,
        ctx: &StageContext,
        fin_dir: &FinDir,
        model: &ModelConfig,
        provider: &dyn LlmProvider,
        io: &dyn AgentIO,
        cancel: CancellationToken,
    ) -> anyhow::Result<Vec<StageArtifact>>;
}

/// Shared helper: run the agent loop with stage-specific prompt and tools.
#[allow(clippy::too_many_arguments)]
pub async fn run_stage_agent(
    stage_prompt: &str,
    initial_message: &str,
    allowed_tools: Option<&[String]>,
    project_context: &str,
    model: &ModelConfig,
    cwd: &std::path::Path,
    provider: &dyn LlmProvider,
    io: &dyn AgentIO,
    cancel: CancellationToken,
    ctx: &StageContext,
) -> anyhow::Result<()> {
    // Build tool registry (filtered or full)
    let mut tool_registry = match allowed_tools {
        Some(tools) => ToolRegistry::filtered_defaults(cwd, tools),
        None => ToolRegistry::with_defaults(cwd),
    };

    // Check for workflow agents available for delegation
    let available_agents_summary = if !ctx.workflow_agents.is_empty() {
        // Build a workflow-only agent registry (fin-* agents from .fin/agents/)
        let workflow_registry = Arc::new(ctx.workflow_agents.clone());

        // Register the delegate tool if we have a provider registry
        if let Some(ref provider_reg) = ctx.provider_registry {
            tool_registry.register(Box::new(DelegateTool::new(
                Arc::clone(&workflow_registry),
                Arc::clone(provider_reg),
                cwd.to_path_buf(),
                0,
            )));
        }

        // Build agent summary for the system prompt
        Some(workflow_registry.prompt_summary())
    } else {
        None
    };

    // Build system prompt with stage context + agent awareness
    let mut role_prompt = format!("{stage_prompt}\n\n# Project Context\n\n{project_context}");
    if let Some(ref agents_summary) = available_agents_summary {
        if !agents_summary.is_empty() {
            role_prompt.push_str("\n\n# Available Agents\n\n");
            role_prompt.push_str(agents_summary);
            role_prompt.push_str(
                "\nYou may delegate research, review, or implementation sub-tasks to these agents \
                 when it would be more efficient than doing the work inline. The stage workflow \
                 prompt above defines YOUR primary responsibility — delegation is optional.\n",
            );
        }
    }

    let agent_context = AgentPromptContext {
        available_agents: None,
        agent_role: Some(role_prompt),
    };
    let system_prompt = build_system_prompt(&tool_registry.schemas(), cwd, Some(&agent_context));

    // Build agent state
    let mut state = AgentState::new(model.clone(), cwd.to_path_buf());
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.messages.push(Message::new_user(initial_message));

    // Run agent loop
    run_agent_loop(&mut state, provider, io, cancel).await?;

    Ok(())
}
