// Fin — Markdown Template Generators
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

/// Initial STATUS.md content for a fresh .fin/ directory.
pub fn initial_status() -> String {
    "# Fin State\n\
     **Active Blueprint:** None\n\
     **Stage:** Idle\n\
     \n\
     ## Next Action\n\
     Run `fin blueprint new <name>` to start a blueprint.\n"
        .to_string()
}

/// Generate a STATUS.md with current workflow position.
pub fn status_template(
    blueprint: &str,
    section: Option<&str>,
    task: Option<&str>,
    stage: &str,
    next_action: &str,
) -> String {
    let mut out = format!(
        "# Fin State\n\
         **Active Blueprint:** {blueprint}\n"
    );

    if let Some(s) = section {
        out.push_str(&format!("**Active Section:** {s}\n"));
    }
    if let Some(t) = task {
        out.push_str(&format!("**Active Task:** {t}\n"));
    }

    out.push_str(&format!(
        "**Stage:** {stage}\n\
         \n\
         ## Next Action\n\
         {next_action}\n"
    ));
    out
}

/// Initial LEDGER.md with table header.
pub fn initial_ledger() -> String {
    "# Decisions Register\n\
     \n\
     | # | When | Scope | Decision | Choice | Rationale | Revisable? |\n\
     |---|------|-------|----------|--------|-----------|------------|\n"
        .to_string()
}

/// Blueprint vision template (B001-VISION.md).
pub fn blueprint_vision(id: &str, title: &str, vision: &str) -> String {
    format!(
        "# {id}: {title}\n\
         \n\
         **Vision:** {vision}\n\
         \n\
         ## Sections\n\
         \n\
         _No sections defined yet._\n\
         \n\
         ## Boundary Map\n\
         \n\
         _Define external interfaces, APIs, and constraints here._\n"
    )
}

/// Blueprint brief template (B001-BRIEF.md).
pub fn blueprint_brief(id: &str, title: &str) -> String {
    format!(
        "# {id}: {title} — Brief\n\
         \n\
         ## Project Background\n\
         \n\
         _Describe the project context, goals, and constraints._\n\
         \n\
         ## Technical Landscape\n\
         \n\
         _Describe the existing codebase, architecture, and tech stack._\n\
         \n\
         ## Key Stakeholders\n\
         \n\
         _List stakeholders and their concerns._\n\
         \n\
         ## Assumptions\n\
         \n\
         _List assumptions made during planning._\n"
    )
}

/// Blueprint findings template (B001-FINDINGS.md).
pub fn blueprint_findings(id: &str, title: &str) -> String {
    format!(
        "# {id}: {title} — Findings\n\
         \n\
         ## Questions\n\
         \n\
         _List open questions to investigate._\n\
         \n\
         ## Findings\n\
         \n\
         _Document research findings here._\n\
         \n\
         ## References\n\
         \n\
         _Links to relevant documentation, articles, and resources._\n"
    )
}

/// Blueprint report template (B001-REPORT.md).
pub fn blueprint_report(id: &str, title: &str) -> String {
    format!(
        "# {id}: {title} — Report\n\
         \n\
         ## Outcome\n\
         \n\
         _Describe the final outcome of this blueprint._\n\
         \n\
         ## Sections Completed\n\
         \n\
         _List all sections and their status._\n\
         \n\
         ## Key Decisions\n\
         \n\
         _Summarize important decisions made during this blueprint._\n\
         \n\
         ## Lessons Learned\n\
         \n\
         _What went well, what could be improved._\n"
    )
}

/// Section spec template (S01-SPEC.md).
pub fn section_spec(s_id: &str, title: &str, goal: &str) -> String {
    format!(
        "# {s_id}: {title}\n\
         \n\
         **Goal:** {goal}\n\
         \n\
         **Demo:** _Describe what a successful demo looks like._\n\
         \n\
         ## Acceptance Gates\n\
         \n\
         - _List acceptance criteria_\n\
         \n\
         ## Tasks\n\
         \n\
         _Tasks will be added during planning._\n\
         \n\
         ## Files Likely Touched\n\
         \n\
         _List files expected to be created or modified._\n"
    )
}

/// Task spec template (T01-SPEC.md).
pub fn task_spec(t_id: &str, title: &str, section_id: &str, blueprint_id: &str) -> String {
    format!(
        "# {t_id}: {title}\n\
         \n\
         **Section:** {section_id}\n\
         **Blueprint:** {blueprint_id}\n\
         \n\
         ## Goal\n\
         \n\
         _Describe what this task accomplishes._\n\
         \n\
         ## Acceptance Gates\n\
         \n\
         ### Truths\n\
         \n\
         _Invariants that must hold._\n\
         \n\
         ### Artifacts\n\
         \n\
         _Files, configs, or outputs this task produces._\n\
         \n\
         ### Key Links\n\
         \n\
         _References to related docs, issues, or PRs._\n\
         \n\
         ## Steps\n\
         \n\
         1. _Step 1_\n\
         2. _Step 2_\n\
         \n\
         ## Context\n\
         \n\
         _Any additional context or notes._\n"
    )
}

/// Task report template (T01-REPORT.md).
pub fn task_report(t_id: &str, parent_section: &str, blueprint: &str, one_liner: &str) -> String {
    format!(
        "---\n\
         id: {t_id}\n\
         parent: {parent_section}\n\
         blueprint: {blueprint}\n\
         provides: []\n\
         key_files: []\n\
         duration: \"\"\n\
         verification_result: \"\"\n\
         completed_at: \"\"\n\
         ---\n\
         \n\
         # {t_id}: {one_liner}\n\
         \n\
         ## What Happened\n\
         \n\
         _Describe what was actually done._\n\
         \n\
         ## Deviations\n\
         \n\
         _Note any deviations from the plan._\n\
         \n\
         ## Files Created/Modified\n\
         \n\
         _List files affected by this task._\n"
    )
}

/// Section report template (S01-REPORT.md).
pub fn section_report(s_id: &str, title: &str) -> String {
    format!(
        "# {s_id}: {title} — Report\n\
         \n\
         ## Outcome\n\
         \n\
         _Describe the result of this section._\n\
         \n\
         ## Tasks Completed\n\
         \n\
         _List all tasks and their outcomes._\n\
         \n\
         ## Demo Result\n\
         \n\
         _Describe the demo result._\n\
         \n\
         ## Files Changed\n\
         \n\
         _Aggregate list of files created or modified._\n"
    )
}

/// Acceptance template (S01-ACCEPTANCE.md).
pub fn acceptance_template(s_id: &str, title: &str) -> String {
    format!(
        "# {s_id}: {title} — Acceptance\n\
         \n\
         ## Acceptance Criteria\n\
         \n\
         - [ ] _Criterion 1_\n\
         - [ ] _Criterion 2_\n\
         \n\
         ## Test Steps\n\
         \n\
         1. _Step 1_\n\
         2. _Step 2_\n\
         \n\
         ## Result\n\
         \n\
         **Status:** Pending\n\
         \n\
         ## Notes\n\
         \n\
         _Any observations during testing._\n"
    )
}

/// Handoff/resume template (handoff.md) with YAML frontmatter.
pub fn handoff_template(
    blueprint: &str,
    section: &str,
    task: &str,
    step: u32,
    total_steps: u32,
) -> String {
    format!(
        "---\n\
         blueprint_id: {blueprint}\n\
         section_id: {section}\n\
         task_id: {task}\n\
         step: {step}\n\
         total_steps: {total_steps}\n\
         saved_at: \"\"\n\
         ---\n\
         \n\
         # Handoff: {task} (Step {step}/{total_steps})\n\
         \n\
         ## Completed Work\n\
         \n\
         _Summary of work done so far._\n\
         \n\
         ## Remaining Work\n\
         \n\
         _What still needs to be done._\n\
         \n\
         ## Decisions Made\n\
         \n\
         _Decisions made during this session._\n\
         \n\
         ## Context\n\
         \n\
         _Important context for the next session._\n\
         \n\
         ## Next Action\n\
         \n\
         _Exact next step to take._\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_status_contains_idle() {
        let s = initial_status();
        assert!(s.contains("**Stage:** Idle"));
        assert!(s.contains("**Active Blueprint:** None"));
    }

    #[test]
    fn test_status_template_full() {
        let s = status_template("B001", Some("S01"), Some("T01"), "Building", "Continue T01");
        assert!(s.contains("**Active Blueprint:** B001"));
        assert!(s.contains("**Active Section:** S01"));
        assert!(s.contains("**Active Task:** T01"));
        assert!(s.contains("**Stage:** Building"));
    }

    #[test]
    fn test_status_template_no_section() {
        let s = status_template("B001", None, None, "Architecting", "Create sections");
        assert!(!s.contains("Active Section"));
        assert!(!s.contains("Active Task"));
    }

    #[test]
    fn test_initial_ledger_has_table() {
        let d = initial_ledger();
        assert!(d.contains("| # | When | Scope |"));
        assert!(d.contains("|---|"));
    }

    #[test]
    fn test_blueprint_vision() {
        let r = blueprint_vision("B001", "Setup", "Get the project bootstrapped");
        assert!(r.contains("# B001: Setup"));
        assert!(r.contains("**Vision:** Get the project bootstrapped"));
        assert!(r.contains("## Sections"));
        assert!(r.contains("## Boundary Map"));
    }

    #[test]
    fn test_section_spec() {
        let p = section_spec("S01", "Core API", "Build the core API endpoints");
        assert!(p.contains("# S01: Core API"));
        assert!(p.contains("**Goal:** Build the core API endpoints"));
        assert!(p.contains("## Tasks"));
    }

    #[test]
    fn test_task_spec() {
        let p = task_spec("T01", "Add routes", "S01", "B001");
        assert!(p.contains("# T01: Add routes"));
        assert!(p.contains("**Section:** S01"));
        assert!(p.contains("**Blueprint:** B001"));
        assert!(p.contains("## Steps"));
    }

    #[test]
    fn test_task_report_frontmatter() {
        let s = task_report("T01", "S01", "B001", "Added routes");
        assert!(s.starts_with("---\n"));
        assert!(s.contains("id: T01"));
        assert!(s.contains("parent: S01"));
        assert!(s.contains("blueprint: B001"));
    }

    #[test]
    fn test_handoff_template_frontmatter() {
        let c = handoff_template("B001", "S01", "T01", 3, 7);
        assert!(c.contains("step: 3"));
        assert!(c.contains("total_steps: 7"));
        assert!(c.contains("Step 3/7"));
    }

    #[test]
    fn test_acceptance_template() {
        let u = acceptance_template("S01", "Core API");
        assert!(u.contains("# S01: Core API — Acceptance"));
        assert!(u.contains("**Status:** Pending"));
    }

    #[test]
    fn test_blueprint_brief() {
        let c = blueprint_brief("B001", "Setup");
        assert!(c.contains("# B001: Setup — Brief"));
        assert!(c.contains("## Project Background"));
    }

    #[test]
    fn test_blueprint_findings() {
        let r = blueprint_findings("B001", "Setup");
        assert!(r.contains("# B001: Setup — Findings"));
        assert!(r.contains("## Questions"));
    }

    #[test]
    fn test_blueprint_report() {
        let s = blueprint_report("B001", "Setup");
        assert!(s.contains("# B001: Setup — Report"));
        assert!(s.contains("## Lessons Learned"));
    }

    #[test]
    fn test_section_report() {
        let s = section_report("S01", "Core API");
        assert!(s.contains("# S01: Core API — Report"));
        assert!(s.contains("## Demo Result"));
    }
}
