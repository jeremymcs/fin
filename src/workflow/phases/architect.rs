// Fin + Architect Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct ArchitectStage;

#[async_trait]
impl StageRunner for ArchitectStage {
    fn stage(&self) -> Stage {
        Stage::Architect
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: None,
            stage: Stage::Architect,
        };
        prompts::architect_prompt(&position)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        let constraint = "\n\nIMPORTANT: Do NOT write code or create project source files. \
             The write tool is ONLY for writing planning artifacts (VISION.md, SPEC.md) to .fin/.";
        match &ctx.section_id {
            Some(s) => format!(
                "Architect section {s} of blueprint {}. Read the VISION.md, boundary map, and any \
                 BRIEF.md / FINDINGS.md files. Decompose into tasks sized for one context window \
                 (2-5 steps, 3-8 files each). Write S{s}-SPEC.md and individual T##-SPEC.md files. \
                 Self-audit: every acceptance gate maps to a task, validation is executable, no circular deps.{constraint}",
                ctx.blueprint_id
            ),
            None => format!(
                "Architect blueprint {}. Read BRIEF.md and FINDINGS.md first. Explore the codebase \
                 to ground your plan in reality. Decompose into 4-10 demoable vertical sections, \
                 risk-first ordering. Write VISION.md with vision, success criteria, proof strategy, \
                 section checkboxes, and boundary map.{constraint}",
                ctx.blueprint_id
            ),
        }
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![
            "read".into(),
            "grep".into(),
            "glob".into(),
            "write".into(),
        ])
    }

    fn delegatable_roles(&self) -> Vec<&str> {
        vec!["planner"]
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

        let mut artifacts = vec![StageArtifact {
            path: std::path::PathBuf::from(format!(
                ".fin/blueprints/{}/{}-VISION.md",
                ctx.blueprint_id, ctx.blueprint_id
            )),
            description: "Blueprint vision with sections".into(),
        }];

        if let Some(ref s) = ctx.section_id {
            artifacts.push(StageArtifact {
                path: std::path::PathBuf::from(format!(
                    ".fin/blueprints/{}/sections/{}/{}-SPEC.md",
                    ctx.blueprint_id, s, s
                )),
                description: format!("Section {s} task decomposition"),
            });
        }

        Ok(artifacts)
    }
}
