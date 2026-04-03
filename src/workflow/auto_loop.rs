// Fin + Auto Loop (Dispatch → Run Unit → Finalize → Repeat)
//
// Same loop for both modes:
//   Auto:   runs to completion (all tasks in section/blueprint)
//   Manual: runs one unit, then stops
//
// Each unit gets a FRESH context window — no token rot.
// State is derived from .fin/ artifacts, not from conversation history.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::models::ModelConfig;
use crate::llm::provider::{LlmProvider, ProviderRegistry};
use crate::workflow::dispatch::{self, DispatchResult, DispatchUnit};
use crate::workflow::git::WorkflowGit;
use crate::workflow::markdown;
use crate::workflow::phases::{StageArtifact, StageContext};
use crate::workflow::state::FinDir;

/// How the loop should behave after running a unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopMode {
    /// Run one unit, then stop. User drives each step.
    Step,
    /// Run all units to completion (or until error/blocked).
    Auto,
}

/// Result of an auto-loop run.
pub struct LoopResult {
    pub units_run: u32,
    pub outcome: LoopOutcome,
}

#[derive(Debug)]
pub enum LoopOutcome {
    /// All work dispatched and completed.
    Complete(String),
    /// Stopped because dispatch is blocked (needs user input).
    Blocked(String),
    /// Stopped after one unit (step mode).
    Stepped(String),
    /// Stopped due to error.
    Error(String),
    /// User cancelled.
    Cancelled,
}

/// Safety valve — prevent infinite loops.
const MAX_ITERATIONS: u32 = 200;

/// Main loop. Derives state from .fin/ each iteration — no stale context.
pub async fn run_loop(
    cwd: &std::path::Path,
    model: &ModelConfig,
    provider: &dyn LlmProvider,
    mode: LoopMode,
    cancel: CancellationToken,
    provider_registry: Option<Arc<ProviderRegistry>>,
    io: &dyn AgentIO,
) -> LoopResult {
    let fin_dir = FinDir::new(cwd);
    let mut units_run: u32 = 0;
    let mut prev_stage: Option<String> = None;

    for _iteration in 0..MAX_ITERATIONS {
        if cancel.is_cancelled() {
            return LoopResult {
                units_run,
                outcome: LoopOutcome::Cancelled,
            };
        }

        // 1. DERIVE — read .fin/ state fresh each iteration
        let dispatch_result = dispatch::dispatch(&fin_dir);

        match dispatch_result {
            DispatchResult::Complete(msg) => {
                let b_id = extract_blueprint_id(&fin_dir);
                let _ = io
                    .emit(AgentEvent::WorkflowComplete {
                        blueprint_id: b_id,
                        units_run,
                    })
                    .await;
                return LoopResult {
                    units_run,
                    outcome: LoopOutcome::Complete(msg),
                };
            }
            DispatchResult::Blocked(msg) => {
                let b_id = extract_blueprint_id(&fin_dir);
                let _ = io
                    .emit(AgentEvent::WorkflowBlocked {
                        blueprint_id: b_id,
                        reason: msg.clone(),
                    })
                    .await;
                return LoopResult {
                    units_run,
                    outcome: LoopOutcome::Blocked(msg),
                };
            }
            DispatchResult::Unit(unit) => {
                let unit_desc = format!(
                    "{}/{} {:?} {} {}",
                    unit.blueprint_id,
                    unit.section_id.as_deref().unwrap_or("-"),
                    unit.unit_type,
                    unit.task_id.as_deref().unwrap_or(""),
                    unit.stage,
                );
                tracing::info!("Dispatching unit: {unit_desc}");

                // Emit stage transition if stage changed (per D-11)
                let current_stage_label = unit.stage.label().to_string();
                if let Some(ref from) = prev_stage {
                    if *from != current_stage_label {
                        let _ = io
                            .emit(AgentEvent::StageTransition {
                                from: from.clone(),
                                to: current_stage_label.clone(),
                            })
                            .await;
                    }
                }
                prev_stage = Some(current_stage_label);

                // Emit workflow unit start
                let _ = io
                    .emit(AgentEvent::WorkflowUnitStart {
                        blueprint_id: unit.blueprint_id.clone(),
                        section_id: unit.section_id.clone(),
                        task_id: unit.task_id.clone(),
                        stage: unit.stage.label().to_string(),
                        unit_type: format!("{:?}", unit.unit_type),
                    })
                    .await;

                // 2. RUN UNIT — fresh context window
                let result = run_unit(
                    cwd,
                    &fin_dir,
                    &unit,
                    model,
                    provider,
                    io,
                    cancel.clone(),
                    provider_registry.clone(),
                )
                .await;

                match result {
                    Ok(artifacts) => {
                        units_run += 1;

                        // Verify expected artifacts exist on disk.
                        // Skip for interactive stages (define, architect) — they produce
                        // artifacts after multi-turn Q&A, not after a single agent run.
                        let is_interactive = matches!(
                            unit.unit_type,
                            dispatch::UnitType::DefineBlueprint
                                | dispatch::UnitType::DefineSection
                                | dispatch::UnitType::ArchitectBlueprint
                                | dispatch::UnitType::ArchitectSection
                        );
                        if !is_interactive {
                            for artifact in &artifacts {
                                if artifact.path.as_os_str().is_empty() {
                                    continue; // Sentinel paths (e.g., "validation-results")
                                }
                                if !artifact.path.exists() {
                                    tracing::warn!(
                                        "Expected artifact missing after {:?}: {} ({})",
                                        unit.unit_type,
                                        artifact.path.display(),
                                        artifact.description,
                                    );
                                    let _ = io
                                        .emit(AgentEvent::WorkflowError {
                                            message: format!(
                                                "Warning: expected artifact not written: {}",
                                                artifact.description
                                            ),
                                        })
                                        .await;
                                }
                            }
                        }

                        let artifact_descs: Vec<String> =
                            artifacts.iter().map(|a| a.description.clone()).collect();

                        // Emit workflow unit end
                        let _ = io
                            .emit(AgentEvent::WorkflowUnitEnd {
                                blueprint_id: unit.blueprint_id.clone(),
                                section_id: unit.section_id.clone(),
                                task_id: unit.task_id.clone(),
                                stage: unit.stage.label().to_string(),
                                artifacts: artifact_descs,
                            })
                            .await;

                        // 3. FINALIZE — update STATUS.md + git ops for next iteration
                        finalize(&fin_dir, &unit, cwd, &artifacts).await;

                        // Emit progress snapshot
                        let snap = fin_dir.progress_snapshot(&unit.blueprint_id);
                        let _ = io
                            .emit(AgentEvent::WorkflowProgress {
                                blueprint_id: unit.blueprint_id.clone(),
                                sections_total: snap.sections_total,
                                sections_done: snap.sections_done,
                                tasks_total: snap.tasks_total,
                                tasks_done: snap.tasks_done,
                                current_stage: unit.stage.label().to_string(),
                                current_section: unit.section_id.clone(),
                                current_task: unit.task_id.clone(),
                            })
                            .await;
                    }
                    Err(e) => {
                        tracing::error!("Unit failed: {e}");
                        let _ = io
                            .emit(AgentEvent::WorkflowError {
                                message: format!("{e}"),
                            })
                            .await;
                        return LoopResult {
                            units_run,
                            outcome: LoopOutcome::Error(format!("{e}")),
                        };
                    }
                }

                // 4. MODE CHECK — step mode exits after one unit
                if mode == LoopMode::Step {
                    let msg = format!(
                        "Completed {:?} for {}/{}",
                        unit.unit_type,
                        unit.blueprint_id,
                        unit.section_id.as_deref().unwrap_or("-"),
                    );
                    return LoopResult {
                        units_run,
                        outcome: LoopOutcome::Stepped(msg),
                    };
                }
            }
        }
    }

    let _ = io
        .emit(AgentEvent::WorkflowError {
            message: format!("Hit max iterations ({MAX_ITERATIONS})"),
        })
        .await;

    LoopResult {
        units_run,
        outcome: LoopOutcome::Error(format!("Hit max iterations ({MAX_ITERATIONS})")),
    }
}

/// Run a single dispatch unit in a FRESH context window.
///
/// Each call creates a new AgentState — no conversation history carries over.
/// Context comes from .fin/ artifacts (specs, reports, ledger), not from
/// prior agent conversations.
#[allow(clippy::too_many_arguments)]
async fn run_unit(
    _cwd: &std::path::Path,
    fin_dir: &FinDir,
    unit: &DispatchUnit,
    model: &ModelConfig,
    provider: &dyn LlmProvider,
    io: &dyn AgentIO,
    cancel: CancellationToken,
    provider_registry: Option<Arc<ProviderRegistry>>,
) -> anyhow::Result<Vec<StageArtifact>> {
    // Load stage context from .fin/ artifacts
    let mut ctx = StageContext::load(
        fin_dir,
        &unit.blueprint_id,
        unit.section_id.as_deref(),
        unit.task_id.as_deref(),
        unit.stage,
    );
    ctx.provider_registry = provider_registry;

    // Get the stage runner
    let runner = super::commands::get_stage_runner(unit.stage);

    // Run in a fresh agent context
    let artifacts = runner
        .run(&ctx, fin_dir, model, provider, io, cancel)
        .await?;

    Ok(artifacts)
}

/// Update STATUS.md, write task markers, and run git automation after a unit completes.
/// Git operations are controlled by workflow preferences.
async fn finalize(
    fin_dir: &FinDir,
    unit: &DispatchUnit,
    cwd: &std::path::Path,
    artifacts: &[StageArtifact],
) {
    let b = &unit.blueprint_id;
    let s = unit.section_id.as_deref();
    let t = unit.task_id.as_deref();
    let prefs = crate::config::preferences::Preferences::resolve(cwd);
    let git = WorkflowGit::new(cwd);

    // Write task-level status markers so dispatch knows what stage to run next
    if let (Some(s_id), Some(t_id)) = (s, t) {
        match unit.unit_type {
            dispatch::UnitType::BuildTask => {
                if let Err(e) = write_task_marker(fin_dir, b, s_id, t_id, "executed") {
                    tracing::error!("Failed to write .executed marker for {t_id}: {e}");
                }
            }
            dispatch::UnitType::ValidateTask => {
                if let Err(e) = write_task_marker(fin_dir, b, s_id, t_id, "validated") {
                    tracing::error!("Failed to write .validated marker for {t_id}: {e}");
                }
            }
            _ => {}
        }
    }

    // ── Git automation (preference-controlled) ──────────────────────

    // Auto-branch: create section branch on first BuildTask for a section
    if prefs.workflow.auto_branch && unit.unit_type == dispatch::UnitType::BuildTask {
        if let Some(s_id) = s {
            // Only create if we're not already on a section branch
            if let Ok(current) = git.current_branch().await {
                let expected = format!("fin/{}/{}", b, s_id);
                if current != expected {
                    if let Err(e) = git.create_section_branch(b, s_id).await {
                        tracing::warn!("Auto-branch failed for {s_id}: {e}");
                    } else {
                        tracing::info!("Created section branch: fin/{b}/{s_id}");
                    }
                }
            }
        }
    }

    // Auto-commit artifacts: commit .fin/ artifact files after stages that produce them
    if prefs.workflow.auto_commit_artifacts {
        let artifact_paths: Vec<std::path::PathBuf> = artifacts
            .iter()
            .filter(|a| a.path.exists())
            .map(|a| a.path.clone())
            .collect();
        if !artifact_paths.is_empty() {
            let scope = match (s, t) {
                (_, Some(t_id)) => t_id.to_string(),
                (Some(s_id), _) => s_id.to_string(),
                _ => b.to_string(),
            };
            let msg = format!("{} complete", unit.stage.label());
            if let Err(e) = git.commit_artifacts(&scope, &msg, &artifact_paths).await {
                tracing::debug!("Auto-commit artifacts skipped: {e}");
            }
        }
    }

    // Auto-squash: merge section branch back to main after SealSection
    if prefs.workflow.auto_squash && unit.unit_type == dispatch::UnitType::SealSection {
        if let Some(s_id) = s {
            // Only squash if we're on a section branch
            if let Ok(current) = git.current_branch().await {
                let expected = format!("fin/{}/{}", b, s_id);
                if current == expected {
                    let title = format!("Section {s_id}");
                    if let Err(e) = git.squash_merge_section(b, s_id, &title, &[]).await {
                        tracing::warn!("Auto-squash failed for {s_id}: {e}");
                    } else {
                        tracing::info!("Squash-merged section branch: fin/{b}/{s_id}");
                    }
                }
            }
        }
    }

    // ── Database sync (best-effort) ───────────────────────────────
    // Sync newly created sections/tasks to SQLite after architect stages
    if matches!(
        unit.unit_type,
        dispatch::UnitType::ArchitectBlueprint | dispatch::UnitType::ArchitectSection
    ) {
        let db_path = cwd.join(".fin").join("fin.db");
        if let Ok(db) = crate::workflow::crud::WorkflowDb::open(&db_path) {
            // Sync sections
            for s_id in fin_dir.list_sections(b) {
                if db.get_section(&s_id).ok().flatten().is_none() {
                    let _ = db.create_section(&s_id, b, &s_id);
                }
            }
            // Sync tasks for the current section
            if let Some(s_id) = s {
                for t_id in fin_dir.list_tasks(b, s_id) {
                    if db.get_task(&t_id).ok().flatten().is_none() {
                        let _ = db.create_task(&t_id, b, s_id, &t_id);
                    }
                }
            }
        }
    }

    // Update task status in DB after build/validate
    if let (Some(s_id), Some(t_id)) = (s, t) {
        let db_path = cwd.join(".fin").join("fin.db");
        if let Ok(db) = crate::workflow::crud::WorkflowDb::open(&db_path) {
            match unit.unit_type {
                dispatch::UnitType::BuildTask => {
                    let _ = db.update_task_status(t_id, "executed");
                }
                dispatch::UnitType::ValidateTask => {
                    let _ = db.update_task_status(t_id, "validated");
                }
                dispatch::UnitType::SealSection => {
                    let _ = db.update_section_status(s_id, "complete");
                }
                _ => {}
            }
        }
    }

    // ── STATUS.md update ────────────────────────────────────────────

    // Derive the next stage hint for STATUS.md
    let (next_stage, hint) = match unit.unit_type {
        dispatch::UnitType::DefineBlueprint => ("explore", format!("Define complete for {b}.")),
        dispatch::UnitType::ExploreBlueprint => ("architect", format!("Explore complete for {b}.")),
        dispatch::UnitType::ArchitectBlueprint => (
            "build",
            format!("Architect complete for {b}. Sections created."),
        ),
        dispatch::UnitType::DefineSection => (
            "architect",
            format!("Define complete for section {}.", s.unwrap_or("?")),
        ),
        dispatch::UnitType::ExploreSection => (
            "architect",
            format!("Explore complete for section {}.", s.unwrap_or("?")),
        ),
        dispatch::UnitType::ArchitectSection => (
            "build",
            format!(
                "Architect complete for section {}. Tasks created.",
                s.unwrap_or("?")
            ),
        ),
        dispatch::UnitType::BuildTask => (
            "validate",
            format!("Build complete for task {}.", t.unwrap_or("?")),
        ),
        dispatch::UnitType::ValidateTask => (
            "advance",
            format!("Validate complete for task {}.", t.unwrap_or("?")),
        ),
        dispatch::UnitType::SealSection => {
            ("advance", format!("Section {} sealed.", s.unwrap_or("?")))
        }
        dispatch::UnitType::AdvanceTask | dispatch::UnitType::AdvanceSection => {
            ("dispatch", "Advanced. Re-dispatching.".into())
        }
    };

    let state = markdown::status_template(
        &format!("{b} — (active)"),
        s.map(|sid| format!("{sid} — (active)")).as_deref(),
        t.map(|tid| format!("{tid} — (active)")).as_deref(),
        next_stage,
        &hint,
    );
    if let Err(e) = fin_dir.write_state(&state) {
        tracing::error!("Failed to write STATUS.md: {e}");
    }
}

/// Write a status marker for a task (e.g., .executed, .verified).
fn write_task_marker(
    fin_dir: &FinDir,
    b_id: &str,
    s_id: &str,
    t_id: &str,
    status: &str,
) -> anyhow::Result<()> {
    let task_dir = fin_dir.task_dir(b_id, s_id, t_id);
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(task_dir.join(format!(".{status}")), "")?;
    Ok(())
}

/// Extract the active blueprint ID from STATUS.md, or return "unknown".
fn extract_blueprint_id(fin_dir: &FinDir) -> String {
    fin_dir
        .read_state()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("**Active Blueprint:**"))
                .map(|rest| {
                    rest.split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}
