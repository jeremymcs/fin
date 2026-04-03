// Fin + Explore Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct ExploreStage;

#[async_trait]
impl StageRunner for ExploreStage {
    fn stage(&self) -> Stage {
        Stage::Explore
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: None,
            stage: Stage::Explore,
        };
        prompts::explore_prompt(&position)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        let scope = match &ctx.section_id {
            Some(s) => format!("section {s} of blueprint {}", ctx.blueprint_id),
            None => format!("blueprint {}", ctx.blueprint_id),
        };
        format!(
            "Explore the codebase for {scope}. Read any BRIEF.md and LEDGER.md first. \
             Scout existing patterns, dependencies, integration points, and constraints. \
             Calibrate depth to complexity — don't invent problems. \
             Write your findings to FINDINGS.md.\
             \n\nIMPORTANT: Do NOT write code or create project files. \
             The write tool is ONLY for writing FINDINGS.md to .fin/. \
             Use bash only for read-only checks (e.g., dependency versions, build status)."
        )
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![
            "read".into(),
            "grep".into(),
            "glob".into(),
            "bash".into(),
            "write".into(),
        ])
    }

    fn delegatable_roles(&self) -> Vec<&str> {
        vec!["researcher"]
    }

    async fn run(
        &self,
        ctx: &StageContext,
        _fin_dir: &FinDir,
        model: &ModelConfig,
        provider: &dyn LlmProvider,
        io: &dyn AgentIO,
        cancel: CancellationToken,
    ) -> anyhow::Result<Vec<StageArtifact>> {
        let cwd = std::env::current_dir()?;
        let tools: Vec<String> = self.allowed_tools().unwrap();

        run_stage_agent(
            &self.system_prompt(ctx),
            &self.initial_message(ctx),
            Some(&tools),
            &ctx.context_for_prompt(),
            model,
            &cwd,
            provider,
            io,
            cancel,
            ctx,
        )
        .await?;

        Ok(vec![StageArtifact {
            path: std::path::PathBuf::from(format!(
                ".fin/blueprints/{}/{}-FINDINGS.md",
                ctx.blueprint_id, ctx.blueprint_id
            )),
            description: "Codebase and technology exploration findings".into(),
        }])
    }
}
