// Fin — Dispatch Table (State → Next Unit Resolution)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use super::Stage;
use super::state::FinDir;

/// A dispatchable unit of work — one fresh context window.
#[derive(Debug, Clone)]
pub struct DispatchUnit {
    pub unit_type: UnitType,
    pub blueprint_id: String,
    pub section_id: Option<String>,
    pub task_id: Option<String>,
    pub stage: Stage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnitType {
    DefineBlueprint,
    ExploreBlueprint,
    ArchitectBlueprint,
    DefineSection,
    ExploreSection,
    ArchitectSection,
    BuildTask,
    ValidateTask,
    SealSection,
    AdvanceTask,
    AdvanceSection,
}

/// Terminal condition — nothing left to dispatch.
#[derive(Debug, Clone)]
pub enum DispatchResult {
    /// Run this unit next.
    Unit(DispatchUnit),
    /// All work complete.
    Complete(String),
    /// Blocked — needs user input or external action.
    Blocked(String),
}

/// Derive the next unit to run from .fin/ filesystem state.
///
/// Rules are evaluated in order — first match wins.
/// State is derived fresh each call (no cached state, no token rot).
pub fn dispatch(fin_dir: &FinDir) -> DispatchResult {
    let state_md = match fin_dir.read_state() {
        Some(s) => s,
        None => return DispatchResult::Blocked("No STATUS.md found. Run `fin init`.".into()),
    };

    let pos = match parse_state(&state_md) {
        Some(p) => p,
        None => {
            return DispatchResult::Blocked(
                "No active blueprint. Run `fin blueprint new <name>`.".into(),
            );
        }
    };

    // Rule 1: Blueprint has no sections → needs planning
    let sections = fin_dir.list_sections(&pos.blueprint_id);
    if sections.is_empty() {
        return dispatch_blueprint_stage(fin_dir, &pos);
    }

    // Rule 2: Find the first incomplete section
    for section_id in &sections {
        let result = dispatch_section(fin_dir, &pos.blueprint_id, section_id);
        match result {
            SectionStatus::NeedsWork(unit) => return DispatchResult::Unit(unit),
            SectionStatus::Complete => continue,
        }
    }

    // Rule 3: All sections complete → blueprint done
    DispatchResult::Complete(format!(
        "Blueprint {} complete — all sections done.",
        pos.blueprint_id
    ))
}

// ── Blueprint-level dispatch ──────────────────────────────────────

fn dispatch_blueprint_stage(fin_dir: &FinDir, pos: &StatePosition) -> DispatchResult {
    let b = &pos.blueprint_id;

    // Check what artifacts exist to determine stage
    let has_brief = fin_dir.blueprint_brief(b).exists();
    let has_findings = fin_dir.blueprint_findings(b).exists();
    let has_vision = fin_dir.blueprint_vision(b).exists();

    if !has_brief {
        return DispatchResult::Unit(DispatchUnit {
            unit_type: UnitType::DefineBlueprint,
            blueprint_id: b.clone(),
            section_id: None,
            task_id: None,
            stage: Stage::Define,
        });
    }

    if !has_findings {
        return DispatchResult::Unit(DispatchUnit {
            unit_type: UnitType::ExploreBlueprint,
            blueprint_id: b.clone(),
            section_id: None,
            task_id: None,
            stage: Stage::Explore,
        });
    }

    if !has_vision || fin_dir.list_sections(b).is_empty() {
        return DispatchResult::Unit(DispatchUnit {
            unit_type: UnitType::ArchitectBlueprint,
            blueprint_id: b.clone(),
            section_id: None,
            task_id: None,
            stage: Stage::Architect,
        });
    }

    // Vision exists but sections exist — fall through to section dispatch
    DispatchResult::Blocked(format!(
        "Blueprint {b} has a vision but dispatch couldn't resolve next unit."
    ))
}

// ── Section-level dispatch ──────────────────────────────────────────

enum SectionStatus {
    NeedsWork(DispatchUnit),
    Complete,
}

fn dispatch_section(fin_dir: &FinDir, b_id: &str, s_id: &str) -> SectionStatus {
    // Check if section has a report (= complete)
    let report_path = fin_dir.section_report(b_id, s_id);
    if report_path.exists() {
        return SectionStatus::Complete;
    }

    // Check if section has a spec
    let spec_path = fin_dir.section_spec(b_id, s_id);
    if !spec_path.exists() {
        // Needs planning — check if brief exists first
        let brief_path = fin_dir.section_brief(b_id, s_id);
        if !brief_path.exists() {
            return SectionStatus::NeedsWork(DispatchUnit {
                unit_type: UnitType::ArchitectSection,
                blueprint_id: b_id.into(),
                section_id: Some(s_id.into()),
                task_id: None,
                stage: Stage::Architect,
            });
        }
        return SectionStatus::NeedsWork(DispatchUnit {
            unit_type: UnitType::ArchitectSection,
            blueprint_id: b_id.into(),
            section_id: Some(s_id.into()),
            task_id: None,
            stage: Stage::Architect,
        });
    }

    // Section has a spec — find the first incomplete task
    let tasks = fin_dir.list_tasks(b_id, s_id);
    if tasks.is_empty() {
        // Spec exists but no task dirs — plan stage should create them
        return SectionStatus::NeedsWork(DispatchUnit {
            unit_type: UnitType::ArchitectSection,
            blueprint_id: b_id.into(),
            section_id: Some(s_id.into()),
            task_id: None,
            stage: Stage::Architect,
        });
    }

    for t_id in &tasks {
        let result = dispatch_task(fin_dir, b_id, s_id, t_id);
        match result {
            TaskStatus::NeedsWork(unit) => return SectionStatus::NeedsWork(unit),
            TaskStatus::Complete => continue,
        }
    }

    // All tasks complete — section needs completion (validation + report)
    SectionStatus::NeedsWork(DispatchUnit {
        unit_type: UnitType::SealSection,
        blueprint_id: b_id.into(),
        section_id: Some(s_id.into()),
        task_id: None,
        stage: Stage::SealSection,
    })
}

// ── Task-level dispatch ───────────────────────────────────────────

enum TaskStatus {
    NeedsWork(DispatchUnit),
    Complete,
}

fn dispatch_task(fin_dir: &FinDir, b_id: &str, s_id: &str, t_id: &str) -> TaskStatus {
    // Task is complete if it has a report
    let report_path = fin_dir.task_report(b_id, s_id, t_id);
    if report_path.exists() {
        return TaskStatus::Complete;
    }

    let spec_path = fin_dir.task_spec(b_id, s_id, t_id);
    if !spec_path.exists() {
        // No spec for this task — section planning incomplete
        return TaskStatus::NeedsWork(DispatchUnit {
            unit_type: UnitType::ArchitectSection,
            blueprint_id: b_id.into(),
            section_id: Some(s_id.into()),
            task_id: None,
            stage: Stage::Architect,
        });
    }

    // Has spec, no report — check status markers to determine stage
    let is_executed = fin_dir.has_task_marker(b_id, s_id, t_id, "executed");
    let is_validated = fin_dir.has_task_marker(b_id, s_id, t_id, "validated");

    if is_validated {
        // Validated — task is complete (executor wrote report during execution)
        return TaskStatus::Complete;
    }

    if is_executed {
        // Executed but not validated → needs validate
        return TaskStatus::NeedsWork(DispatchUnit {
            unit_type: UnitType::ValidateTask,
            blueprint_id: b_id.into(),
            section_id: Some(s_id.into()),
            task_id: Some(t_id.into()),
            stage: Stage::Validate,
        });
    }

    // Not yet executed → build
    TaskStatus::NeedsWork(DispatchUnit {
        unit_type: UnitType::BuildTask,
        blueprint_id: b_id.into(),
        section_id: Some(s_id.into()),
        task_id: Some(t_id.into()),
        stage: Stage::Build,
    })
}

// ── State parsing ─────────────────────────────────────────────────

struct StatePosition {
    blueprint_id: String,
}

fn parse_state(state_md: &str) -> Option<StatePosition> {
    for line in state_md.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("**Active Blueprint:**") {
            let rest = rest.trim();
            if rest != "None" && !rest.is_empty() {
                let b_id = rest.split_whitespace().next()?.to_string();
                return Some(StatePosition { blueprint_id: b_id });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FinDir) {
        let tmp = TempDir::new().unwrap();
        let fin = FinDir::new(tmp.path());
        fin.init().unwrap();
        (tmp, fin)
    }

    #[test]
    fn test_dispatch_no_blueprint() {
        let (_tmp, fin) = setup();
        // STATUS.md exists but has no active blueprint
        fin.write_state("**Active Blueprint:** None").unwrap();
        match dispatch(&fin) {
            DispatchResult::Blocked(msg) => assert!(msg.contains("No active blueprint")),
            other => panic!("Expected Blocked, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_fresh_blueprint_needs_discuss() {
        let (_tmp, fin) = setup();
        fin.create_blueprint("B001").unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            None,
            None,
            "discuss",
            "Start discussing.",
        );
        fin.write_state(&state).unwrap();

        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::DefineBlueprint);
                assert_eq!(unit.blueprint_id, "B001");
                assert_eq!(unit.stage, Stage::Define);
            }
            other => panic!("Expected Unit, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_with_brief_needs_research() {
        let (_tmp, fin) = setup();
        fin.create_blueprint("B001").unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            None,
            None,
            "research",
            "Research next.",
        );
        fin.write_state(&state).unwrap();
        // Write a brief file so discuss is "done"
        std::fs::write(fin.blueprint_brief("B001"), "# Brief\nDecisions here.").unwrap();

        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::ExploreBlueprint);
                assert_eq!(unit.stage, Stage::Explore);
            }
            other => panic!("Expected Unit, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_all_sections_complete() {
        let (_tmp, fin) = setup();
        fin.create_blueprint("B001").unwrap();
        let state =
            crate::workflow::markdown::status_template("B001 — MVP", None, None, "idle", "Done.");
        fin.write_state(&state).unwrap();
        std::fs::write(fin.blueprint_brief("B001"), "# Brief").unwrap();
        std::fs::write(fin.blueprint_findings("B001"), "# Findings").unwrap();
        std::fs::write(fin.blueprint_vision("B001"), "# Vision").unwrap();

        // Create a section with a report (= complete)
        fin.create_section("B001", "S01").unwrap();
        let report_path = fin.section_report("B001", "S01");
        if let Some(parent) = report_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&report_path, "# S01 Report\nDone.").unwrap();

        match dispatch(&fin) {
            DispatchResult::Complete(msg) => assert!(msg.contains("complete")),
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_section_needs_plan() {
        let (_tmp, fin) = setup();
        fin.create_blueprint("B001").unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            Some("S01"),
            None,
            "plan",
            "Plan it.",
        );
        fin.write_state(&state).unwrap();
        std::fs::write(fin.blueprint_brief("B001"), "# Brief").unwrap();
        std::fs::write(fin.blueprint_findings("B001"), "# Findings").unwrap();
        std::fs::write(fin.blueprint_vision("B001"), "# Vision").unwrap();
        fin.create_section("B001", "S01").unwrap();

        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::ArchitectSection);
                assert_eq!(unit.section_id, Some("S01".into()));
            }
            other => panic!("Expected Unit(ArchitectSection), got {:?}", other),
        }
    }

    /// Helper: set up a blueprint with sections and a task that has a spec
    fn setup_with_task() -> (TempDir, FinDir) {
        let (tmp, fin) = setup();
        fin.create_blueprint("B001").unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            Some("S01 — (active)"),
            Some("T01 — (active)"),
            "execute",
            "Go.",
        );
        fin.write_state(&state).unwrap();
        std::fs::write(fin.blueprint_brief("B001"), "# Brief").unwrap();
        std::fs::write(fin.blueprint_findings("B001"), "# Findings").unwrap();
        std::fs::write(fin.blueprint_vision("B001"), "# Vision").unwrap();
        fin.create_section("B001", "S01").unwrap();
        // Write a task spec
        let spec_path = fin.task_spec("B001", "S01", "T01");
        std::fs::write(&spec_path, "# T01 Spec\n\nDo the thing.").unwrap();
        // Write section spec so dispatch doesn't fall back to ArchitectSection
        let section_spec = fin.section_spec("B001", "S01");
        std::fs::write(&section_spec, "# S01 Spec\n\n- T01: Do the thing.").unwrap();
        (tmp, fin)
    }

    #[test]
    fn test_dispatch_task_needs_execute() {
        let (_tmp, fin) = setup_with_task();
        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::BuildTask);
                assert_eq!(unit.task_id, Some("T01".into()));
                assert_eq!(unit.stage, Stage::Build);
            }
            other => panic!("Expected BuildTask, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_task_executed_needs_verify() {
        let (_tmp, fin) = setup_with_task();
        // Write .executed marker
        let task_dir = fin.task_dir("B001", "S01", "T01");
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(task_dir.join(".executed"), "").unwrap();

        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::ValidateTask);
                assert_eq!(unit.task_id, Some("T01".into()));
                assert_eq!(unit.stage, Stage::Validate);
            }
            other => panic!("Expected ValidateTask, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_task_validated_is_complete() {
        let (_tmp, fin) = setup_with_task();
        let task_dir = fin.task_dir("B001", "S01", "T01");
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(task_dir.join(".executed"), "").unwrap();
        std::fs::write(task_dir.join(".validated"), "").unwrap();

        // Validated task is complete — executor wrote report during execution.
        // With only one task complete, section needs SealSection.
        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::SealSection);
                assert_eq!(unit.section_id, Some("S01".into()));
                assert_eq!(unit.stage, Stage::SealSection);
            }
            other => panic!("Expected SealSection, got {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_task_with_report_is_complete() {
        let (_tmp, fin) = setup_with_task();
        // Write report — task is done (even without markers, report = complete)
        std::fs::write(fin.task_report("B001", "S01", "T01"), "# T01 Report\nDone.").unwrap();

        // With only one task complete, section needs SealSection
        match dispatch(&fin) {
            DispatchResult::Unit(unit) => {
                assert_eq!(unit.unit_type, UnitType::SealSection);
                assert_eq!(unit.section_id, Some("S01".into()));
            }
            other => panic!("Expected SealSection, got {:?}", other),
        }
    }
}
