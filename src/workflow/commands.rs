// Fin + Workflow CLI Command Handlers

use std::path::Path;
use std::sync::Arc;

use crate::io::print_io::PrintIO;
use crate::llm::provider::ProviderRegistry;

use super::Stage;
use super::markdown;
use super::phases::advance::AdvanceStage;
use super::phases::architect::ArchitectStage;
use super::phases::build::BuildStage;
use super::phases::define::DefineStage;
use super::phases::explore::ExploreStage;
use super::phases::seal_section::SealSectionStage;
use super::phases::validate::ValidateStage;
use super::phases::{StageContext, StageRunner};
use super::state::FinDir;

/// Map the codebase — run a read-only agent that writes `.fin/CODEBASE_MAP.md`.
pub async fn cmd_map(cwd: &Path, model_override: Option<&str>) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);

    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Run `fin init` first.");
    }

    let model = crate::io::print::pick_model(model_override)?;
    eprintln!(
        "[Mapping codebase — {} via {}]",
        model.display_name, model.provider
    );
    eprintln!("Exploring {} ...\n", cwd.display());

    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let io = PrintIO::new(true, true);
    let cancel = tokio_util::sync::CancellationToken::new();

    // Read-only tool set + write (for the map output only)
    let allowed_tools: Vec<String> = ["read", "glob", "grep", "bash", "write"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let tool_registry = crate::tools::ToolRegistry::filtered_defaults(cwd, &allowed_tools);

    let prompt_content = super::prompts::map_prompt(&cwd.display().to_string());
    let agent_context = crate::agent::prompt::AgentPromptContext {
        available_agents: None,
        agent_role: Some(prompt_content.clone()),
    };
    let system_prompt = crate::agent::prompt::build_system_prompt(
        &tool_registry.schemas(),
        cwd,
        Some(&agent_context),
    );

    // Remove any stale map so the agent writes fresh
    let map_path = fin_dir.map_path();
    if map_path.exists() {
        eprintln!("Replacing existing CODEBASE_MAP.md ...");
    }

    let mut agent_state = crate::agent::state::AgentState::new(model.clone(), cwd.to_path_buf());
    agent_state.tool_registry = tool_registry;
    agent_state.system_prompt = system_prompt;
    agent_state
        .messages
        .push(crate::llm::types::Message::new_user(
            "Map this codebase. Follow the instructions in your system prompt exactly.",
        ));

    crate::agent::agent_loop::run_agent_loop(&mut agent_state, provider, &io, cancel).await?;

    if map_path.exists() {
        eprintln!("\nMap saved to {}", map_path.display());
        eprintln!(
            "All agents will now reference this map. Re-run `fin map` after significant changes."
        );
    } else {
        eprintln!("\nWarning: CODEBASE_MAP.md was not written. Check agent output above.");
    }

    Ok(())
}

/// Initialize .fin/ workflow directory in the current project.
pub async fn cmd_init(cwd: &Path) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);

    if fin_dir.exists() {
        eprintln!(".fin/ already exists at {}", cwd.display());
        return Ok(());
    }

    fin_dir.init()?;
    eprintln!("Initialized .fin/ workflow directory at {}", cwd.display());
    eprintln!("Next steps:");
    eprintln!(
        "  1. `fin map`              — map the codebase (agents reference this before planning)"
    );
    eprintln!("  2. `fin blueprint new <name>` — create your first blueprint");
    Ok(())
}

/// Show current workflow status with progress summary.
pub async fn cmd_status(cwd: &Path) -> anyhow::Result<String> {
    let fin_dir = FinDir::new(cwd);

    if !fin_dir.exists() {
        return Ok("No .fin/ directory found. Run `fin init` to start.".to_string());
    }

    let status = match fin_dir.read_state() {
        Some(s) => s,
        None => return Ok("STATUS.md not found. Run `fin init` to reinitialize.".to_string()),
    };

    let progress = fin_dir.blueprint_progress_summary();
    if progress.contains("No active blueprint") {
        Ok(status)
    } else {
        Ok(format!("{status}\n{progress}"))
    }
}

/// Create a new blueprint.
/// Guards against concurrent blueprints — only one active at a time.
pub async fn cmd_blueprint_new(cwd: &Path, name: &str) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);

    if !fin_dir.exists() {
        fin_dir.init()?;
    }

    // Guard: no concurrent blueprints
    if let super::state::BlueprintStatus::InProgress { id, stage, .. } =
        fin_dir.active_blueprint_status()
    {
        eprintln!("Blueprint {id} is already in progress (stage: {stage}).");
        eprintln!("  (Ignoring \"{name}\" — finish or complete {id} first.)");
        eprintln!("\nRunning health check:");

        let report = fin_dir.blueprint_health_check();
        if !report.issues.is_empty() {
            eprintln!("  Health: {}", report.summary);
            for fix in &report.fixed {
                eprintln!("  Fixed: {fix}");
            }
        } else {
            eprintln!("  Health: state is healthy.");
        }

        let progress = fin_dir.blueprint_progress_summary();
        eprintln!("\n{progress}");
        eprintln!("\nUse `fin next` or `fin auto` to continue.");
        return Ok(());
    }

    // Find next blueprint ID
    let blueprints = fin_dir.list_blueprints();
    let next_num = blueprints.len() + 1;
    let id = format!("B{:03}", next_num);

    fin_dir.create_blueprint(&id)?;

    // Persist to SQLite database (best-effort — file state is authoritative)
    let db_path = cwd.join(".fin").join("fin.db");
    if let Ok(db) = super::crud::WorkflowDb::open(&db_path) {
        if let Err(e) = db.create_blueprint(&id, name, "") {
            tracing::warn!("Failed to write blueprint to DB: {e}");
        }
    }

    // Write initial vision
    let vision = markdown::blueprint_vision(&id, name, "");
    let vision_path = fin_dir.blueprint_vision(&id);
    std::fs::write(&vision_path, &vision)?;

    // Update STATUS.md
    let state = markdown::status_template(
        &format!("{id} — {name}"),
        None,
        None,
        "define",
        &format!(
            "Run `fin define` to identify gray areas for {id}, or `fin architect` to start planning."
        ),
    );
    fin_dir.write_state(&state)?;

    eprintln!("Created blueprint {id}: {name}");
    eprintln!("Next: run `fin define` or `fin architect` to get started.");
    Ok(())
}

/// Create a blueprint from a PRD or ADR document.
/// Runs the analyst agent to produce BRIEF, FINDINGS, and VISION artifacts,
/// then the dispatch loop picks up at the architect stage.
pub async fn cmd_blueprint_from_doc(
    cwd: &Path,
    doc_type: &str,
    doc_path: &str,
    model: &crate::llm::models::ModelConfig,
    provider: &dyn crate::llm::provider::LlmProvider,
    provider_registry: Arc<ProviderRegistry>,
    io: &dyn crate::io::agent_io::AgentIO,
) -> anyhow::Result<String> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        fin_dir.init()?;
    }

    // Guard: no concurrent blueprints
    if let super::state::BlueprintStatus::InProgress { id, .. } = fin_dir.active_blueprint_status()
    {
        anyhow::bail!("Blueprint {id} is already in progress. Complete or finish it first.");
    }

    // Resolve document path
    let doc_full_path = if std::path::Path::new(doc_path).is_absolute() {
        std::path::PathBuf::from(doc_path)
    } else {
        cwd.join(doc_path)
    };
    if !doc_full_path.exists() {
        anyhow::bail!("Document not found: {}", doc_full_path.display());
    }
    let doc_content = std::fs::read_to_string(&doc_full_path)?;
    let doc_name = doc_full_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    // Create blueprint
    let blueprints = fin_dir.list_blueprints();
    let next_num = blueprints.len() + 1;
    let id = format!("B{:03}", next_num);
    fin_dir.create_blueprint(&id)?;

    // Write initial status
    let state = markdown::status_template(
        &format!("{id} — {doc_name} ({doc_type})"),
        None,
        None,
        "analyze",
        &format!("Analyzing {doc_type} document..."),
    );
    fin_dir.write_state(&state)?;

    // Persist to SQLite
    let db_path = cwd.join(".fin").join("fin.db");
    if let Ok(db) = super::crud::WorkflowDb::open(&db_path) {
        let _ = db.create_blueprint(&id, doc_name, &format!("From {doc_type}: {doc_path}"));
    }

    // Build analyst prompt
    let analyst_prompt = format!(
        r#"# Document Analysis: {doc_type}

You are analyzing a {doc_type} document to produce workflow artifacts for blueprint {id}.

## Source Document

```
{doc_content}
```

## Your Task

Read this {doc_type} document AND explore the codebase at {cwd}. Then produce these three files:

### 1. Write `{id}-BRIEF.md` to `.fin/blueprints/{id}/`
Extract from the document:
- ## Vision (1-2 paragraph summary of what needs to be built)
- ## Implementation Decisions (key decisions from the document)
- ## Constraints (non-functional requirements, limitations)
- ## Deferred Ideas (things explicitly out of scope)

### 2. Write `{id}-FINDINGS.md` to `.fin/blueprints/{id}/`
Cross-reference the document against the actual codebase:
- ## Summary (feasibility assessment + primary recommendation)
- ## Key Files (existing code that's relevant)
- ## Build Order (what to prove first, risk-ordered)
- ## Constraints (hard limits from the codebase)
- ## Open Risks (unknowns from the document or codebase)

### 3. Write `{id}-VISION.md` to `.fin/blueprints/{id}/`
Decompose into sections:
- ## Vision (from the document)
- ## Success Criteria (measurable outcomes)
- ## Sections (4-10 vertical, demoable slices — each gets a directory)
  For each section: ### S01 — Title, what it delivers, acceptance criteria
- ## Key Risks (risk → which section retires it)

After writing VISION.md, create a section directory under `.fin/blueprints/{id}/sections/` for each section (e.g., S01, S02, S03).

## Rules
- Ground everything in the ACTUAL codebase, not just the document
- Flag conflicts between the document and existing code
- Flag gaps: what does the document NOT address?
- Order sections risk-first (hardest/most uncertain first)
"#,
        doc_type = doc_type.to_uppercase(),
        id = id,
        doc_content = if doc_content.len() > 50000 {
            format!(
                "{}...\n\n(Document truncated at 50K chars)",
                &doc_content[..50000]
            )
        } else {
            doc_content
        },
        cwd = cwd.display(),
    );

    // Run the analyst agent
    let agent_registry = Arc::new(crate::agents::AgentRegistry::load_default());
    let mut tool_registry = crate::tools::ToolRegistry::with_defaults(cwd);

    // Add extension tools
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }

    // Add delegate tool if agents available
    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(crate::agents::DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.to_path_buf(),
            0,
        )));
    }

    let system_prompt =
        crate::agent::prompt::build_system_prompt(&tool_registry.schemas(), cwd, None);

    let mut agent_state = crate::agent::state::AgentState::new(model.clone(), cwd.to_path_buf());
    agent_state.tool_registry = tool_registry;
    agent_state.system_prompt = system_prompt;
    agent_state
        .messages
        .push(crate::llm::types::Message::new_user(&analyst_prompt));

    let cancel = tokio_util::sync::CancellationToken::new();
    crate::agent::agent_loop::run_agent_loop(&mut agent_state, provider, io, cancel).await?;

    // Update status to architect stage (artifacts should now exist)
    let state = markdown::status_template(
        &format!("{id} — {doc_name} ({doc_type})"),
        None,
        None,
        "architect",
        &format!("{doc_type} analysis complete. Use /next or /auto to continue."),
    );
    fin_dir.write_state(&state)?;

    Ok(id)
}

/// List blueprints.
pub async fn cmd_blueprint_list(cwd: &Path) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);

    if !fin_dir.exists() {
        eprintln!("No .fin/ directory found. Run `fin init` first.");
        return Ok(());
    }

    let blueprints = fin_dir.list_blueprints();
    if blueprints.is_empty() {
        eprintln!("No blueprints yet. Run `fin blueprint new <name>` to create one.");
    } else {
        for b in &blueprints {
            let vision_path = fin_dir.blueprint_vision(b);
            let title = if vision_path.exists() {
                std::fs::read_to_string(&vision_path)
                    .ok()
                    .and_then(|c| {
                        c.lines()
                            .next()
                            .map(|l| l.trim_start_matches("# ").to_string())
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            eprintln!("  {b}: {title}");
        }
    }
    Ok(())
}

/// Run a specific workflow stage.
pub async fn cmd_stage(
    cwd: &Path,
    stage_name: &str,
    model_override: Option<&str>,
) -> anyhow::Result<()> {
    let stage = Stage::from_str(stage_name)
        .ok_or_else(|| anyhow::anyhow!(
            "Unknown stage: '{stage_name}'. Valid: define, explore, architect, build, validate, seal-section, advance"
        ))?;

    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Run `fin init` first.");
    }

    // Parse current position from STATUS.md
    let position = parse_position_from_state(&fin_dir)?;
    eprintln!(
        "[{}] Running {} stage for {}",
        position.blueprint_id, stage, position.blueprint_id
    );

    // Build stage context
    let mut ctx = StageContext::load(
        &fin_dir,
        &position.blueprint_id,
        position.section_id.as_deref(),
        position.task_id.as_deref(),
        stage,
    );

    // Resolve model
    let model = crate::io::print::pick_model(model_override)?;
    eprintln!("[{} via {}]", model.display_name, model.provider);

    // Build provider registry
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    ctx.provider_registry = Some(Arc::clone(&provider_registry));
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    // IO adapter
    let io = PrintIO::new(true, true);

    // Cancellation
    let cancel = tokio_util::sync::CancellationToken::new();

    // Get the right stage runner and execute
    let runner = get_stage_runner(stage);
    let artifacts = runner
        .run(&ctx, &fin_dir, &model, provider, &io, cancel)
        .await?;

    // Report artifacts produced
    if !artifacts.is_empty() {
        eprintln!("\nArtifacts produced:");
        for artifact in &artifacts {
            eprintln!("  {} — {}", artifact.path.display(), artifact.description);
        }
    }

    // Update STATUS.md stage
    if let Some(next_stage) = stage.next() {
        let state = markdown::status_template(
            &format!("{} — (active)", position.blueprint_id),
            position
                .section_id
                .as_deref()
                .map(|s| format!("{s} — (active)"))
                .as_deref(),
            position
                .task_id
                .as_deref()
                .map(|t| format!("{t} — (active)"))
                .as_deref(),
            next_stage.label(),
            &format!("Run `fin stage {}` to continue.", next_stage.label()),
        );
        fin_dir.write_state(&state)?;
        eprintln!("\nNext stage: {next_stage}. Run `fin stage {next_stage}` to continue.");
    } else {
        eprintln!("\nWorkflow cycle complete for current scope.");
    }

    Ok(())
}

/// Run the next logical unit based on dispatch (step mode — one unit, then stop).
pub async fn cmd_next(cwd: &Path, model_override: Option<&str>) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Run `fin init` first.");
    }

    let model = crate::io::print::pick_model(model_override)?;
    eprintln!("[{} via {}]", model.display_name, model.provider);

    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let io = crate::io::print_io::PrintIO::new(true, true);
    let cancel = tokio_util::sync::CancellationToken::new();
    let result = super::auto_loop::run_loop(
        cwd,
        &model,
        provider,
        super::auto_loop::LoopMode::Step,
        cancel,
        Some(Arc::clone(&provider_registry)),
        &io,
    )
    .await;

    eprintln!(
        "\nRan {} unit(s). Outcome: {:?}",
        result.units_run, result.outcome
    );
    Ok(())
}

/// Run autonomously — dispatch → build → validate → repeat until done.
pub async fn cmd_auto(cwd: &Path, model_override: Option<&str>) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Run `fin init` first.");
    }

    let model = crate::io::print::pick_model(model_override)?;
    eprintln!(
        "[Auto mode — {} via {}]",
        model.display_name, model.provider
    );
    eprintln!("Running all units to completion. Ctrl+C to stop.\n");

    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let io = crate::io::print_io::PrintIO::new(true, true);
    let cancel = tokio_util::sync::CancellationToken::new();
    let result = super::auto_loop::run_loop(
        cwd,
        &model,
        provider,
        super::auto_loop::LoopMode::Auto,
        cancel,
        Some(Arc::clone(&provider_registry)),
        &io,
    )
    .await;

    eprintln!(
        "\nAuto mode complete. Ran {} unit(s). Outcome: {:?}",
        result.units_run, result.outcome
    );
    Ok(())
}

/// Pause: write handoff.md for current work.
pub fn cmd_pause(cwd: &Path) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);

    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Nothing to pause.");
    }

    let state = fin_dir.read_state().unwrap_or_default();
    eprintln!("Current state:\n{state}");
    eprintln!("\nTo write a handoff point, the agent uses the handoff protocol during execution.");
    Ok(())
}

/// Resume from handoff.md — find the most recent handoff point and pick up.
pub async fn cmd_resume(cwd: &Path, model_override: Option<&str>) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found. Run `fin init` first.");
    }

    let position = parse_position_from_state(&fin_dir)?;
    let b_id = &position.blueprint_id;

    // Search sections for a handoff.md
    let sections = fin_dir.list_sections(b_id);
    let mut found = None;
    for s_id in &sections {
        if super::continue_protocol::has_continue(&fin_dir, b_id, s_id) {
            found = Some(s_id.clone());
            break;
        }
    }

    let s_id = found
        .ok_or_else(|| anyhow::anyhow!("No handoff.md found in {b_id}. Nothing to resume."))?;

    let state = super::continue_protocol::read_continue(&fin_dir, b_id, &s_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse handoff.md for {b_id}/{s_id}"))?;

    eprintln!(
        "Resuming {}/{}/{} — step {}/{}",
        b_id, s_id, state.task_id, state.step, state.total_steps
    );
    eprintln!("Completed: {}", state.completed_work);
    eprintln!("Next action: {}", state.next_action);

    // Resume by running the build stage with the resume context injected
    cmd_stage(cwd, "build", model_override).await?;

    // Clean up handoff.md after successful resume
    super::continue_protocol::remove_continue(&fin_dir, b_id, &s_id)?;
    eprintln!("Removed handoff.md for {b_id}/{s_id}.");

    Ok(())
}

/// Ship: squash-merge current section branch to main.
pub async fn cmd_ship(cwd: &Path) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found.");
    }

    let position = parse_position_from_state(&fin_dir)?;
    let b_id = &position.blueprint_id;

    let git = super::git::WorkflowGit::new(cwd);
    let current_branch = git.current_branch().await?;
    let main_branch = git.main_branch().await;

    // Ensure we're on a section branch
    if !current_branch.starts_with("fin/") {
        anyhow::bail!(
            "Not on a section branch (current: {current_branch}). \
             Ship squash-merges a section branch to {main_branch}."
        );
    }

    // Parse section ID from branch name (fin/B001/S01 → S01)
    let parts: Vec<&str> = current_branch.split('/').collect();
    let s_id = parts.get(2).ok_or_else(|| {
        anyhow::anyhow!("Unexpected branch format: {current_branch}. Expected fin/<b_id>/<s_id>.")
    })?;

    // Check for uncommitted changes
    if git.has_changes().await? {
        anyhow::bail!("Working tree has uncommitted changes. Commit or stash before shipping.");
    }

    // Read section title from SPEC.md if available
    let spec_path = fin_dir.section_spec(b_id, s_id);
    let section_title = if spec_path.exists() {
        std::fs::read_to_string(&spec_path)
            .ok()
            .and_then(|c| {
                c.lines()
                    .next()
                    .map(|l| l.trim_start_matches("# ").to_string())
            })
            .unwrap_or_else(|| s_id.to_string())
    } else {
        s_id.to_string()
    };

    // Collect task reports
    let tasks = fin_dir.list_tasks(b_id, s_id);
    let reports: Vec<String> = tasks
        .iter()
        .filter_map(|t_id| {
            let report_path = fin_dir.task_report(b_id, s_id, t_id);
            if report_path.exists() {
                std::fs::read_to_string(&report_path).ok().and_then(|c| {
                    c.lines()
                        .next()
                        .map(|l| l.trim_start_matches("# ").to_string())
                })
            } else {
                Some(t_id.clone())
            }
        })
        .collect();

    eprintln!("Shipping {b_id}/{s_id}: {section_title}");
    eprintln!("  Branch: {current_branch} → {main_branch} (squash merge)");
    eprintln!("  Tasks: {}", reports.join(", "));

    // Squash merge
    git.squash_merge_section(b_id, s_id, &section_title, &reports)
        .await?;

    eprintln!("\nShipped! Branch {current_branch} merged to {main_branch} and deleted.");

    // Update STATUS.md
    let state = markdown::status_template(
        &format!("{b_id} — (active)"),
        None,
        None,
        "define",
        &format!("{s_id} shipped. Create next section or run `fin blueprint complete`."),
    );
    fin_dir.write_state(&state)?;

    Ok(())
}

/// Complete the current blueprint — verify all sections done, write report, update STATUS.md.
pub async fn cmd_blueprint_complete(cwd: &Path) -> anyhow::Result<()> {
    let fin_dir = FinDir::new(cwd);
    if !fin_dir.exists() {
        anyhow::bail!("No .fin/ directory found.");
    }

    let position = parse_position_from_state(&fin_dir)?;
    let b_id = &position.blueprint_id;

    // Verify all sections are complete (have reports)
    let sections = fin_dir.list_sections(b_id);
    if sections.is_empty() {
        anyhow::bail!("Blueprint {b_id} has no sections.");
    }

    let mut incomplete = Vec::new();
    for s_id in &sections {
        if !fin_dir.section_report(b_id, s_id).exists() {
            incomplete.push(s_id.clone());
        }
    }

    if !incomplete.is_empty() {
        anyhow::bail!(
            "Cannot complete blueprint {b_id} — sections still incomplete: {}",
            incomplete.join(", ")
        );
    }

    // Collect section reports for the blueprint report
    let mut section_one_liners = Vec::new();
    for s_id in &sections {
        let report_path = fin_dir.section_report(b_id, s_id);
        if let Ok(content) = std::fs::read_to_string(&report_path) {
            let one_liner = content
                .lines()
                .find(|l| !l.starts_with('#') && !l.trim().is_empty())
                .unwrap_or(s_id)
                .trim()
                .to_string();
            section_one_liners.push(format!("- {s_id}: {one_liner}"));
        }
    }

    // Write blueprint report
    let title = format!("Complete ({} sections)", sections.len());
    let mut report = markdown::blueprint_report(b_id, &title);
    report.push_str("\n## Section Reports\n\n");
    report.push_str(&section_one_liners.join("\n"));
    report.push('\n');
    let report_path = fin_dir.blueprint_report(b_id);
    std::fs::write(&report_path, &report)?;

    // Update STATUS.md
    let state = markdown::status_template(
        &format!("{b_id} — COMPLETE"),
        None,
        None,
        "idle",
        &format!(
            "Blueprint {b_id} complete ({} sections). Run `fin blueprint new <name>` for next.",
            sections.len()
        ),
    );
    fin_dir.write_state(&state)?;

    // Update database
    let db_path = cwd.join(".fin").join("fin.db");
    if let Ok(db) = super::crud::WorkflowDb::open(&db_path) {
        if let Err(e) = db.update_blueprint_status(b_id, "complete") {
            tracing::warn!("Failed to update blueprint status in DB: {e}");
        }
    }

    eprintln!("Blueprint {b_id} complete!");
    eprintln!("  {} sections shipped", sections.len());
    eprintln!("  Report: {}", report_path.display());
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────

pub fn get_stage_runner(stage: Stage) -> Box<dyn StageRunner> {
    match stage {
        Stage::Define => Box::new(DefineStage),
        Stage::Explore => Box::new(ExploreStage),
        Stage::Architect => Box::new(ArchitectStage),
        Stage::Build => Box::new(BuildStage),
        Stage::Validate => Box::new(ValidateStage),
        Stage::SealSection => Box::new(SealSectionStage),
        Stage::Advance => Box::new(AdvanceStage),
    }
}

/// Parse the current workflow position from STATUS.md.
fn parse_position_from_state(fin_dir: &FinDir) -> anyhow::Result<super::WorkflowPosition> {
    let state = fin_dir
        .read_state()
        .ok_or_else(|| anyhow::anyhow!("STATUS.md not found. Run `fin init` first."))?;

    let mut blueprint_id = String::new();
    let mut section_id: Option<String> = None;
    let mut task_id: Option<String> = None;
    let mut stage = Stage::Define;

    for line in state.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("**Active Blueprint:**") {
            let rest = rest.trim();
            if rest != "None" {
                // Extract blueprint ID (e.g., "B001 — Title" → "B001")
                blueprint_id = rest.split_whitespace().next().unwrap_or("").to_string();
            }
        }

        if let Some(rest) = line.strip_prefix("**Active Section:**") {
            let rest = rest.trim();
            if !rest.is_empty() && rest != "None" {
                section_id = Some(rest.split_whitespace().next().unwrap_or("").to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("**Active Task:**") {
            let rest = rest.trim();
            if !rest.is_empty() && rest != "None" {
                task_id = Some(rest.split_whitespace().next().unwrap_or("").to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("**Stage:**") {
            let rest = rest.trim().to_lowercase();
            if let Some(s) = Stage::from_str(&rest) {
                stage = s;
            }
        }
    }

    if blueprint_id.is_empty() {
        anyhow::bail!("No active blueprint. Run `fin blueprint new <name>` first.");
    }

    Ok(super::WorkflowPosition {
        blueprint_id,
        section_id,
        task_id,
        stage,
    })
}
