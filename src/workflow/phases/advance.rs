// Fin + Advance Stage Runner

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{StageArtifact, StageContext, StageRunner};
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::models::ModelConfig;
use crate::llm::provider::LlmProvider;
use crate::workflow::Stage;
use crate::workflow::markdown;
use crate::workflow::state::FinDir;

/// The advance stage is pure logic — no agent loop needed.
/// It marks work done, updates STATE.md, and advances to the next item.
pub struct AdvanceStage;

#[async_trait]
impl StageRunner for AdvanceStage {
    fn stage(&self) -> Stage {
        Stage::Advance
    }

    fn system_prompt(&self, _ctx: &StageContext) -> String {
        String::new() // Not used — advance is pure logic
    }

    fn initial_message(&self, _ctx: &StageContext) -> String {
        String::new() // Not used
    }

    fn allowed_tools(&self) -> Option<Vec<String>> {
        Some(vec![]) // No tools needed
    }

    async fn run(
        &self,
        ctx: &StageContext,
        fin_dir: &FinDir,
        _model: &ModelConfig,
        _provider: &dyn LlmProvider,
        io: &dyn AgentIO,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Vec<StageArtifact>> {
        let m = &ctx.blueprint_id;

        // Determine what to advance based on current position
        match (&ctx.section_id, &ctx.task_id) {
            (Some(section), Some(task)) => {
                // Task completed — check if more tasks in section
                let tasks = fin_dir.list_tasks(m, section);
                let task_idx = tasks.iter().position(|t| t == task);

                if let Some(idx) = task_idx {
                    if idx + 1 < tasks.len() {
                        // More tasks in this section — advance to next task
                        let next_task = &tasks[idx + 1];
                        let state = markdown::status_template(
                            &format!("{m} — (active)"),
                            Some(&format!("{section} — (active)")),
                            Some(&format!("{next_task} — (pending)")),
                            "build",
                            &format!("Run `fin stage build` to start task {next_task}."),
                        );
                        fin_dir.write_state(&state)?;
                        let _ = io
                            .emit(AgentEvent::TextDelta {
                                text: format!(
                                    "Advanced to task {next_task} in section {section}.\n"
                                ),
                            })
                            .await;
                    } else {
                        let msg = advance_to_next_section(fin_dir, m, section)?;
                        let _ = io.emit(AgentEvent::TextDelta { text: msg }).await;
                    }
                } else {
                    let msg = advance_to_next_section(fin_dir, m, section)?;
                    let _ = io.emit(AgentEvent::TextDelta { text: msg }).await;
                }
            }
            (Some(section), None) => {
                // Section-level advance (e.g., after section planning)
                let tasks = fin_dir.list_tasks(m, section);
                if tasks.is_empty() {
                    let state = markdown::status_template(
                        &format!("{m} — (active)"),
                        Some(&format!("{section} — (active)")),
                        None,
                        "architect",
                        &format!(
                            "Run `fin stage architect` to decompose section {section} into tasks."
                        ),
                    );
                    fin_dir.write_state(&state)?;
                } else {
                    let first_task = &tasks[0];
                    let state = markdown::status_template(
                        &format!("{m} — (active)"),
                        Some(&format!("{section} — (active)")),
                        Some(&format!("{first_task} — (pending)")),
                        "build",
                        &format!("Run `fin stage build` to start task {first_task}."),
                    );
                    fin_dir.write_state(&state)?;
                }
            }
            (None, _) => {
                // Blueprint-level advance — find first section
                let sections = fin_dir.list_sections(m);
                if sections.is_empty() {
                    let state = markdown::status_template(
                        &format!("{m} — (active)"),
                        None,
                        None,
                        "architect",
                        &format!(
                            "Run `fin stage architect` to decompose blueprint {m} into sections."
                        ),
                    );
                    fin_dir.write_state(&state)?;
                } else {
                    let first_section = &sections[0];
                    let state = markdown::status_template(
                        &format!("{m} — (active)"),
                        Some(&format!("{first_section} — (pending)")),
                        None,
                        "architect",
                        &format!("Run `fin stage architect` to plan section {first_section}."),
                    );
                    fin_dir.write_state(&state)?;
                }
            }
        }

        Ok(vec![StageArtifact {
            path: fin_dir.status_path(),
            description: "Updated STATE.md with new position".into(),
        }])
    }
}

fn advance_to_next_section(
    fin_dir: &FinDir,
    m: &str,
    current_section: &str,
) -> anyhow::Result<String> {
    let sections = fin_dir.list_sections(m);
    let section_idx = sections.iter().position(|s| s == current_section);

    if let Some(idx) = section_idx {
        if idx + 1 < sections.len() {
            let next_section = &sections[idx + 1];
            let state = markdown::status_template(
                &format!("{m} — (active)"),
                Some(&format!("{next_section} — (pending)")),
                None,
                "define",
                &format!(
                    "Section {current_section} complete. Run `fin stage define` or `fin stage architect` for section {next_section}."
                ),
            );
            fin_dir.write_state(&state)?;
            return Ok(format!(
                "Section {current_section} complete. Advanced to section {next_section}.\n"
            ));
        } else {
            // All sections complete — blueprint done
            let state = markdown::status_template(
                &format!("{m} — COMPLETE"),
                None,
                None,
                "idle",
                &format!(
                    "All sections in blueprint {m} are complete! Run `fin blueprint complete` or `fin blueprint new <name>`."
                ),
            );
            fin_dir.write_state(&state)?;
            return Ok(format!("Blueprint {m} complete! All sections done.\n"));
        }
    }

    Ok(String::new())
}
