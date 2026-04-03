// Fin + Validate Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner, run_stage_agent};
use crate::io::agent_io::AgentIO;
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::prompts;
use crate::workflow::state::FinDir;

pub struct ValidateStage;

#[async_trait]
impl StageRunner for ValidateStage {
    fn stage(&self) -> Stage {
        Stage::Validate
    }

    fn system_prompt(&self, ctx: &StageContext) -> String {
        // Extract acceptance gates from the task spec
        let acceptance_gates = extract_acceptance_gates(ctx.task_spec_md.as_deref().unwrap_or(""));
        let position = crate::workflow::WorkflowPosition {
            blueprint_id: ctx.blueprint_id.clone(),
            section_id: ctx.section_id.clone(),
            task_id: ctx.task_id.clone(),
            stage: Stage::Validate,
        };
        prompts::validate_prompt(&position, &acceptance_gates)
    }

    fn initial_message(&self, ctx: &StageContext) -> String {
        let task = ctx.task_id.as_deref().unwrap_or("T01");
        let section = ctx.section_id.as_deref().unwrap_or("S01");
        format!(
            "Validate task {task} from section {section} of blueprint {}. \
             For each acceptance gate: determine the strongest validation tier (static → command → behavioral), \
             run the check, record exact command and output, classify PASS/FAIL/PARTIAL. \
             Failures block validation — do not skip them. \
             Also run any section-level validation commands from the spec.",
            ctx.blueprint_id
        )
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![
            "read".into(),
            "grep".into(),
            "glob".into(),
            "bash".into(),
        ])
    }

    fn delegatable_roles(&self) -> Vec<&str> {
        vec!["reviewer", "tester"]
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
            path: std::path::PathBuf::from("validation-results"),
            description: "Validation evidence".into(),
        }])
    }
}

/// Extract acceptance gate items from a task spec markdown.
fn extract_acceptance_gates(spec: &str) -> Vec<String> {
    let mut gates = Vec::new();
    let mut in_gates = false;

    for line in spec.lines() {
        if line.starts_with("## Acceptance Gates")
            || line.starts_with("### Truths")
            || line.starts_with("### Artifacts")
            || line.starts_with("### Key Links")
        {
            in_gates = true;
            continue;
        }
        if line.starts_with("## ") && in_gates {
            in_gates = false;
            continue;
        }
        if in_gates {
            let trimmed = line.trim().trim_start_matches("- ").trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                gates.push(trimmed.to_string());
            }
        }
    }

    gates
}
