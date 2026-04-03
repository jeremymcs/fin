// Fin + Build Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct BuildStage;

#[async_trait]
impl StageRunner for BuildStage {
    fn stage(&self) -> Stage {
        Stage::Build
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        let task_spec = ctx.task_spec_md.as_deref().unwrap_or("No task spec found.");
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: ctx.task_id.clone(),
            stage: Stage::Build,
        };
        prompts::build_prompt(&position, task_spec)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        let task = ctx.task_id.as_deref().unwrap_or("T01");
        let section = ctx.section_id.as_deref().unwrap_or("S01");
        format!(
            "Build task {task} from section {section} of blueprint {}. \
             Read referenced files before changing them — validate the architect's assumptions. \
             Build the real thing (no stubs). Write or update tests. \
             Run builds after changes. Mark progress with [DONE:N]. \
             Note any deviations from the spec.",
            ctx.blueprint_id
        )
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        // Build stage gets ALL tools
        None
    }

    fn delegatable_roles(&self) -> Vec<&str> {
        vec!["builder"]
    }

    async fn run(
        &self,
        ctx: &StageContext,
        fin_dir: &FinDir,
        model: &ModelConfig,
        provider: &dyn LlmProvider,
        io: &dyn AgentIO,
        cancel: CancellationToken,
    ) -> anyhow::Result<Vec<StageArtifact>> {
        let cwd = std::env::current_dir()?;

        run_stage_agent(
            &self.system_prompt(ctx),
            &self.initial_message(ctx),
            None, // all tools
            &ctx.context_for_prompt(),
            model,
            &cwd,
            provider,
            io,
            cancel,
            ctx,
        )
        .await?;

        let task = ctx.task_id.as_deref().unwrap_or("T01");
        let section = ctx.section_id.as_deref().unwrap_or("S01");

        let mut artifacts = vec![StageArtifact {
            path: cwd,
            description: "Code changes from task execution".into(),
        }];

        // Check if the builder wrote a task report (expected per prompt instructions)
        let report_path = fin_dir.task_report(&ctx.blueprint_id, section, task);
        if report_path.exists() {
            artifacts.push(StageArtifact {
                path: report_path,
                description: format!("Task {task} report"),
            });
        }

        Ok(artifacts)
    }
}
