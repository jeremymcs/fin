// Fin + Seal Section Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct SealSectionStage;

#[async_trait]
impl StageRunner for SealSectionStage {
    fn stage(&self) -> Stage {
        Stage::SealSection
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: None,
            stage: Stage::SealSection,
        };
        prompts::seal_section_prompt(&position)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        let section = ctx.section_id.as_deref().unwrap_or("S01");
        format!(
            "Seal section {section} of blueprint {}. \
             Read all task reports, run section-level validation commands from the spec, \
             then write {section}-REPORT.md with Forward Intelligence and {section}-ACCEPTANCE.md \
             with concrete test cases. If any validation fails, report it clearly.\
             \n\nIMPORTANT: Do NOT modify code or create project source files. \
             The write tool is ONLY for writing REPORT.md and ACCEPTANCE.md to .fin/. \
             Use bash only for running validation commands.",
            ctx.blueprint_id
        )
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![
            "read".into(),
            "write".into(),
            "grep".into(),
            "glob".into(),
            "bash".into(),
        ])
    }

    fn delegatable_roles(&self) -> Vec<&str> {
        vec!["reviewer"]
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

        let section = ctx.section_id.as_deref().unwrap_or("S01");
        let mut artifacts = vec![StageArtifact {
            path: std::path::PathBuf::from(format!(
                ".fin/blueprints/{}/sections/{}/{}-REPORT.md",
                ctx.blueprint_id, section, section
            )),
            description: format!("Section {section} report with forward intelligence"),
        }];

        artifacts.push(StageArtifact {
            path: std::path::PathBuf::from(format!(
                ".fin/blueprints/{}/sections/{}/{}-ACCEPTANCE.md",
                ctx.blueprint_id, section, section
            )),
            description: format!("Section {section} acceptance test cases"),
        });

        Ok(artifacts)
    }
}
