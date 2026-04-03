// Fin + Define Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct DefineStage;

#[async_trait]
impl StageRunner for DefineStage {
    fn stage(&self) -> Stage {
        Stage::Define
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: None,
            stage: Stage::Define,
        };
        prompts::define_prompt(&position)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        format!(
            "Start the define stage for blueprint {}. \
             \n\nIMPORTANT RULES:\
             \n- This is a CONVERSATION stage. Do NOT write code or create project files.\
             \n- Do NOT use the write tool to create source files (.js, .py, .rs, .html, etc).\
             \n- The write tool is ONLY for writing BRIEF.md to .fin/ after the conversation is complete.\
             \n- Investigate the codebase silently first, then share your reflection.\
             \n- Ask ONE question at a time and WAIT for my answer before continuing.\
             \n- After 3-5 answered questions, summarize decisions and ask for confirmation.\
             \n- Only then write the BRIEF.md file.",
            ctx.blueprint_id
        )
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![
            "read".into(),
            "grep".into(),
            "glob".into(),
            "write".into(),
        ])
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
        let stage_prompt = self.system_prompt(ctx);
        let initial = self.initial_message(ctx);

        run_stage_agent(
            &stage_prompt,
            &initial,
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
                ".fin/blueprints/{}/{}-BRIEF.md",
                ctx.blueprint_id, ctx.blueprint_id
            )),
            description: "Discovery session context with user decisions".into(),
        }])
    }
}
