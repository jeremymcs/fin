// Fin + .fin/ Directory Manager

use std::fs;
use std::path::{Path, PathBuf};

use super::markdown;

/// Manages the `.fin/` directory structure for a project.
pub struct FinDir {
    root: PathBuf,
}

impl FinDir {
    /// Create a new FinDir rooted at `project_root/.fin/`.
    pub fn new(project_root: &Path) -> Self {
        Self {
            root: project_root.join(".fin"),
        }
    }

    /// Whether the `.fin/` directory exists on disk.
    pub fn exists(&self) -> bool {
        self.root.exists()
    }

    /// The `.fin/` directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // ── Init ────────────────────────────────────────────────────────

    /// Create the `.fin/` directory with initial files:
    /// - STATUS.md
    /// - LEDGER.md
    /// - blueprints/
    /// - agents/ (seeded with default Fin agent definitions)
    pub fn init(&self) -> anyhow::Result<()> {
        fs::create_dir_all(self.root.join("blueprints"))?;
        fs::write(self.status_path(), markdown::initial_status())?;
        fs::write(self.ledger_path(), markdown::initial_ledger())?;
        // Initialize SQLite database (creates tables if needed)
        if let Err(e) = crate::db::project::ProjectDb::open(&self.db_path()) {
            tracing::warn!("Failed to initialize project database: {e}");
        }
        // Seed default agent definitions
        let agents_dir = self.root.join("agents");
        if let Err(e) = crate::agents::defaults::seed_default_agents(&agents_dir) {
            tracing::warn!("Failed to seed default agents: {e}");
        }
        Ok(())
    }

    /// Path to the project-level agents directory.
    pub fn agents_dir(&self) -> PathBuf {
        self.root.join("agents")
    }

    // ── Path accessors ──────────────────────────────────────────────

    pub fn status_path(&self) -> PathBuf {
        self.root.join("STATUS.md")
    }

    pub fn ledger_path(&self) -> PathBuf {
        self.root.join("LEDGER.md")
    }

    pub fn map_path(&self) -> PathBuf {
        self.root.join("CODEBASE_MAP.md")
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("fin.db")
    }

    // ── Blueprint paths ─────────────────────────────────────────────

    pub fn blueprint_dir(&self, id: &str) -> PathBuf {
        self.root.join("blueprints").join(id)
    }

    pub fn blueprint_vision(&self, id: &str) -> PathBuf {
        self.blueprint_dir(id).join(format!("{id}-VISION.md"))
    }

    pub fn blueprint_brief(&self, id: &str) -> PathBuf {
        self.blueprint_dir(id).join(format!("{id}-BRIEF.md"))
    }

    pub fn blueprint_findings(&self, id: &str) -> PathBuf {
        self.blueprint_dir(id).join(format!("{id}-FINDINGS.md"))
    }

    pub fn blueprint_report(&self, id: &str) -> PathBuf {
        self.blueprint_dir(id).join(format!("{id}-REPORT.md"))
    }

    // ── Section paths ─────────────────────────────────────────────

    pub fn section_dir(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.blueprint_dir(b_id).join("sections").join(s_id)
    }

    pub fn section_spec(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id).join(format!("{s_id}-SPEC.md"))
    }

    pub fn section_brief(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id)
            .join(format!("{s_id}-BRIEF.md"))
    }

    pub fn section_findings(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id)
            .join(format!("{s_id}-FINDINGS.md"))
    }

    pub fn section_report(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id)
            .join(format!("{s_id}-REPORT.md"))
    }

    pub fn section_acceptance(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id)
            .join(format!("{s_id}-ACCEPTANCE.md"))
    }

    pub fn section_handoff(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id).join("handoff.md")
    }

    // ── Task paths ──────────────────────────────────────────────────

    pub fn tasks_dir(&self, b_id: &str, s_id: &str) -> PathBuf {
        self.section_dir(b_id, s_id).join("tasks")
    }

    pub fn task_spec(&self, b_id: &str, s_id: &str, t_id: &str) -> PathBuf {
        self.tasks_dir(b_id, s_id).join(format!("{t_id}-SPEC.md"))
    }

    pub fn task_report(&self, b_id: &str, s_id: &str, t_id: &str) -> PathBuf {
        self.tasks_dir(b_id, s_id).join(format!("{t_id}-REPORT.md"))
    }

    /// Directory for task-specific status markers (.executed, .validated).
    pub fn task_dir(&self, b_id: &str, s_id: &str, t_id: &str) -> PathBuf {
        self.tasks_dir(b_id, s_id).join(t_id)
    }

    /// Check if a task status marker exists (e.g., "executed", "validated").
    pub fn has_task_marker(&self, b_id: &str, s_id: &str, t_id: &str, marker: &str) -> bool {
        self.task_dir(b_id, s_id, t_id)
            .join(format!(".{marker}"))
            .exists()
    }

    // ── Directory creation ──────────────────────────────────────────

    /// Create the directory for a blueprint (including the sections subdir).
    pub fn create_blueprint(&self, id: &str) -> anyhow::Result<()> {
        fs::create_dir_all(self.blueprint_dir(id).join("sections"))?;
        Ok(())
    }

    /// Create the directory for a section (including the tasks subdir).
    pub fn create_section(&self, b_id: &str, s_id: &str) -> anyhow::Result<()> {
        fs::create_dir_all(self.tasks_dir(b_id, s_id))?;
        Ok(())
    }

    // ── Enumeration ─────────────────────────────────────────────────

    /// List blueprint directory names, sorted.
    pub fn list_blueprints(&self) -> Vec<String> {
        Self::sorted_dir_names(&self.root.join("blueprints"))
    }

    /// List section directory names within a blueprint, sorted.
    pub fn list_sections(&self, b_id: &str) -> Vec<String> {
        Self::sorted_dir_names(&self.blueprint_dir(b_id).join("sections"))
    }

    /// List task file stems within a section's tasks/ dir, sorted.
    /// Returns unique task IDs extracted from filenames like "T01-SPEC.md".
    pub fn list_tasks(&self, b_id: &str, s_id: &str) -> Vec<String> {
        let dir = self.tasks_dir(b_id, s_id);
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };

        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                // Extract task ID: "T01-SPEC.md" -> "T01", or dir "T01" -> "T01"
                let id = name.split('-').next().unwrap_or("");
                // Only accept valid task IDs (T followed by digits)
                if id.starts_with('T')
                    && id.len() > 1
                    && id[1..].chars().all(|c| c.is_ascii_digit())
                {
                    Some(id.to_string())
                } else {
                    None
                }
            })
            .collect();

        ids.sort();
        ids.dedup();
        ids
    }

    // ── Generic read/write ──────────────────────────────────────────

    /// Read a file relative to the `.fin/` root. Returns None if missing.
    pub fn read_file(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.root.join(relative)).ok()
    }

    /// Write a file relative to the `.fin/` root, creating parent dirs.
    pub fn write_file(&self, relative: &str, content: &str) -> anyhow::Result<()> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(())
    }

    // ── STATUS.md helpers ────────────────────────────────────────────

    pub fn read_state(&self) -> Option<String> {
        fs::read_to_string(self.status_path()).ok()
    }

    pub fn write_state(&self, content: &str) -> anyhow::Result<()> {
        fs::write(self.status_path(), content)?;
        Ok(())
    }

    // ── LEDGER.md append ─────────────────────────────────────────────

    /// Append a row to the decisions table in LEDGER.md.
    pub fn append_decision(
        &self,
        id: &str,
        scope: &str,
        title: &str,
        choice: &str,
        rationale: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let row = format!("| {id} | {now} | {scope} | {title} | {choice} | {rationale} | Yes |\n");

        let mut content =
            fs::read_to_string(self.ledger_path()).unwrap_or_else(|_| markdown::initial_ledger());
        content.push_str(&row);
        fs::write(self.ledger_path(), content)?;
        Ok(())
    }

    // ── Blueprint status ──────────────────────────────────────────

    /// Determine the current blueprint status from STATUS.md.
    pub fn active_blueprint_status(&self) -> BlueprintStatus {
        let state_md = match self.read_state() {
            Some(s) => s,
            None => return BlueprintStatus::Idle,
        };

        let mut blueprint_raw = String::new();
        let mut stage = String::new();
        let mut section: Option<String> = None;
        let mut task: Option<String> = None;

        for line in state_md.lines() {
            let line = line.trim();

            if let Some(rest) = line.strip_prefix("**Active Blueprint:**") {
                blueprint_raw = rest.trim().to_string();
            }
            if let Some(rest) = line.strip_prefix("**Stage:**") {
                stage = rest.trim().to_string();
            }
            if let Some(rest) = line.strip_prefix("**Active Section:**") {
                let rest = rest.trim();
                if !rest.is_empty() && rest != "None" {
                    section = Some(rest.split_whitespace().next().unwrap_or("").to_string());
                }
            }
            if let Some(rest) = line.strip_prefix("**Active Task:**") {
                let rest = rest.trim();
                if !rest.is_empty() && rest != "None" {
                    task = Some(rest.split_whitespace().next().unwrap_or("").to_string());
                }
            }
        }

        if blueprint_raw.is_empty() || blueprint_raw == "None" {
            return BlueprintStatus::Idle;
        }

        // Check for COMPLETE marker
        if blueprint_raw.contains("COMPLETE") {
            let id = blueprint_raw
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            return BlueprintStatus::Complete(id);
        }

        let id = blueprint_raw
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        if id.is_empty() {
            return BlueprintStatus::Idle;
        }

        BlueprintStatus::InProgress {
            id,
            stage,
            section,
            task,
        }
    }

    // ── Health check ────────────────────────────────────────────────

    /// Validate .fin/ state consistency and fix issues.
    /// Returns a report of what was found and fixed.
    pub fn blueprint_health_check(&self) -> HealthReport {
        let mut report = HealthReport::default();

        let status = self.active_blueprint_status();
        let b_id = match &status {
            BlueprintStatus::InProgress { id, .. } => id.clone(),
            _ => {
                report.summary = "No active blueprint to check.".into();
                return report;
            }
        };

        // Check 1: Blueprint directory exists
        if !self.blueprint_dir(&b_id).exists() {
            report.issues.push(HealthIssue {
                severity: Severity::Error,
                description: format!("Blueprint dir for {b_id} does not exist"),
                fixed: true,
            });
            // Fix: reset to idle
            let _ = self.write_state(&super::markdown::initial_status());
            report
                .fixed
                .push(format!("Reset STATUS.md to Idle (missing {b_id} dir)"));
            report.summary = format!("Blueprint {b_id} dir missing — reset to Idle.");
            return report;
        }

        // Check 2: Stage vs artifacts consistency
        if let BlueprintStatus::InProgress { ref stage, .. } = status {
            let stage_lower = stage.to_lowercase();

            // If past define but no BRIEF.md, define wasn't completed
            if matches!(
                stage_lower.as_str(),
                "explore" | "architect" | "build" | "validate"
            ) && !self.blueprint_brief(&b_id).exists()
            {
                report.issues.push(HealthIssue {
                    severity: Severity::Warning,
                    description: format!("Stage is '{stage}' but no BRIEF.md exists for {b_id}"),
                    fixed: true,
                });
                let state = super::markdown::status_template(
                    &format!("{b_id} — (active)"),
                    None,
                    None,
                    "define",
                    "Brief missing — restarting from define stage.",
                );
                let _ = self.write_state(&state);
                report
                    .fixed
                    .push("Reset stage to 'define' (missing BRIEF.md)".into());
            }

            // If past explore but no FINDINGS.md
            if matches!(stage_lower.as_str(), "architect" | "build" | "validate")
                && !self.blueprint_findings(&b_id).exists()
                && self.blueprint_brief(&b_id).exists()
            {
                report.issues.push(HealthIssue {
                    severity: Severity::Warning,
                    description: format!("Stage is '{stage}' but no FINDINGS.md exists for {b_id}"),
                    fixed: true,
                });
                let state = super::markdown::status_template(
                    &format!("{b_id} — (active)"),
                    None,
                    None,
                    "explore",
                    "Findings missing — restarting from explore stage.",
                );
                let _ = self.write_state(&state);
                report
                    .fixed
                    .push("Reset stage to 'explore' (missing FINDINGS.md)".into());
            }
        }

        // Check 3: Section references in STATUS.md point to existing dirs
        if let BlueprintStatus::InProgress {
            section: Some(s_id),
            ..
        } = &status
        {
            if !self.section_dir(&b_id, s_id).exists() {
                report.issues.push(HealthIssue {
                    severity: Severity::Warning,
                    description: format!(
                        "STATUS.md references section {s_id} but dir doesn't exist"
                    ),
                    fixed: true,
                });
                // Clear section/task references
                let state = super::markdown::status_template(
                    &format!("{b_id} — (active)"),
                    None,
                    None,
                    "architect",
                    &format!("Section {s_id} missing — re-run architect."),
                );
                let _ = self.write_state(&state);
                report
                    .fixed
                    .push(format!("Cleared stale section reference {s_id}"));
            }
        }

        // Check 4: Task references point to existing specs
        if let BlueprintStatus::InProgress {
            ref section,
            ref task,
            ..
        } = status
        {
            if let (Some(s_id), Some(t_id)) = (section, task) {
                if self.section_dir(&b_id, s_id).exists()
                    && !self.task_spec(&b_id, s_id, t_id).exists()
                {
                    report.issues.push(HealthIssue {
                        severity: Severity::Warning,
                        description: format!("STATUS.md references task {t_id} but no SPEC exists"),
                        fixed: true,
                    });
                    // Clear task reference, keep section
                    let state = super::markdown::status_template(
                        &format!("{b_id} — (active)"),
                        Some(&format!("{s_id} — (active)")),
                        None,
                        "architect",
                        &format!("Task {t_id} spec missing — re-run architect for {s_id}."),
                    );
                    let _ = self.write_state(&state);
                    report
                        .fixed
                        .push(format!("Cleared stale task reference {t_id}"));
                }
            }
        }

        // Check 5: Orphaned task markers (markers without SPEC files)
        let sections = self.list_sections(&b_id);
        for s_id in &sections {
            let tasks = self.list_tasks(&b_id, s_id);
            for t_id in &tasks {
                let has_spec = self.task_spec(&b_id, s_id, t_id).exists();
                let task_dir = self.task_dir(&b_id, s_id, t_id);
                let has_executed = task_dir.join(".executed").exists();
                let has_validated = task_dir.join(".validated").exists();

                if !has_spec && (has_executed || has_validated) {
                    report.issues.push(HealthIssue {
                        severity: Severity::Warning,
                        description: format!("Task {t_id} in {s_id} has markers but no SPEC"),
                        fixed: true,
                    });
                    // Remove orphaned markers
                    let _ = fs::remove_file(task_dir.join(".executed"));
                    let _ = fs::remove_file(task_dir.join(".validated"));
                    report
                        .fixed
                        .push(format!("Removed orphaned markers for {t_id}"));
                }
            }
        }

        // Build summary
        if report.issues.is_empty() {
            report.summary = format!("Blueprint {b_id} state is healthy.");
        } else {
            let fixed_count = report.fixed.len();
            let total = report.issues.len();
            report.summary = format!("Found {total} issue(s), fixed {fixed_count}.");
        }

        report
    }

    // ── Progress snapshot (for workflow events) ─────────────────────

    /// Structured progress snapshot for a blueprint — used by workflow events.
    pub fn progress_snapshot(&self, b_id: &str) -> BlueprintProgressSnapshot {
        let sections = self.list_sections(b_id);
        let mut sections_done: u32 = 0;
        let mut tasks_total: u32 = 0;
        let mut tasks_done: u32 = 0;

        for s_id in &sections {
            let has_report = self.section_report(b_id, s_id).exists();
            if has_report {
                sections_done += 1;
            }
            let tasks = self.list_tasks(b_id, s_id);
            tasks_total += tasks.len() as u32;
            for t_id in &tasks {
                if self.task_report(b_id, s_id, t_id).exists() {
                    tasks_done += 1;
                }
            }
        }

        BlueprintProgressSnapshot {
            sections_total: sections.len() as u32,
            sections_done,
            tasks_total,
            tasks_done,
        }
    }

    // ── Progress summary ────────────────────────────────────────────

    /// Build a human-readable progress summary for the active blueprint.
    pub fn blueprint_progress_summary(&self) -> String {
        let status = self.active_blueprint_status();
        let b_id = match &status {
            BlueprintStatus::InProgress { id, .. } => id.clone(),
            BlueprintStatus::Complete(id) => return format!("Blueprint {id} is complete."),
            BlueprintStatus::Idle => return "No active blueprint.".into(),
        };

        let mut lines = Vec::new();

        // Blueprint-level artifacts
        let has_brief = self.blueprint_brief(&b_id).exists();
        let has_findings = self.blueprint_findings(&b_id).exists();
        let has_vision = self.blueprint_vision(&b_id).exists();

        lines.push(format!("Blueprint {b_id}:"));
        lines.push(format!(
            "  VISION.md:   {}",
            if has_vision { "done" } else { "pending" }
        ));
        lines.push(format!(
            "  BRIEF.md:    {}",
            if has_brief { "done" } else { "pending" }
        ));
        lines.push(format!(
            "  FINDINGS.md: {}",
            if has_findings { "done" } else { "pending" }
        ));

        // Sections
        let sections = self.list_sections(&b_id);
        if sections.is_empty() {
            lines.push("  Sections: none yet".into());
        } else {
            lines.push(format!("  Sections: {}", sections.len()));
            for s_id in &sections {
                let has_spec = self.section_spec(&b_id, s_id).exists();
                let has_report = self.section_report(&b_id, s_id).exists();
                let tasks = self.list_tasks(&b_id, s_id);
                let tasks_done = tasks
                    .iter()
                    .filter(|t| self.task_report(&b_id, s_id, t).exists())
                    .count();
                let status_str = if has_report {
                    "complete".to_string()
                } else if has_spec {
                    format!("tasks {}/{}", tasks_done, tasks.len())
                } else {
                    "needs planning".to_string()
                };
                lines.push(format!("    {s_id}: {status_str}"));
            }
        }

        // Current position
        if let BlueprintStatus::InProgress {
            stage,
            section,
            task,
            ..
        } = &status
        {
            lines.push(String::new());
            lines.push(format!("Current: stage={stage}"));
            if let Some(s) = section {
                lines.push(format!("  section={s}"));
            }
            if let Some(t) = task {
                lines.push(format!("  task={t}"));
            }
        }

        lines.join("\n")
    }

    // ── Blueprint listing ────────────────────────────────────────────

    /// List all blueprints with their status (active, complete, or stage info).
    pub fn list_blueprints_display(&self) -> String {
        let blueprints = self.list_blueprints();
        if blueprints.is_empty() {
            return "No blueprints yet. Use /blueprint <name> to create one.".into();
        }

        let status = self.active_blueprint_status();
        let active_id = match &status {
            BlueprintStatus::InProgress { id, .. } => Some(id.as_str()),
            _ => None,
        };

        let mut lines = vec!["Blueprints:".to_string()];
        for b_id in &blueprints {
            let vision_path = self.blueprint_vision(b_id);
            let title = if vision_path.exists() {
                fs::read_to_string(&vision_path)
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

            let has_report = self.blueprint_report(b_id).exists();
            let marker = if Some(b_id.as_str()) == active_id {
                " (active)"
            } else if has_report {
                " (complete)"
            } else {
                ""
            };

            lines.push(format!("  {b_id}: {title}{marker}"));
        }

        lines.join("\n")
    }

    // ── Private helpers ─────────────────────────────────────────────

    /// Read a directory and return sorted entry names that are directories.
    fn sorted_dir_names(dir: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };

        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();

        names.sort();
        names
    }
}

// ── Progress snapshot ──────────────────────────────────────────────

/// Structured progress counts for workflow event emission.
#[derive(Debug, Clone, Default)]
pub struct BlueprintProgressSnapshot {
    pub sections_total: u32,
    pub sections_done: u32,
    pub tasks_total: u32,
    pub tasks_done: u32,
}

// ── Blueprint status types ─────────────────────────────────────────

/// Status of the active blueprint parsed from STATUS.md.
#[derive(Debug, Clone, PartialEq)]
pub enum BlueprintStatus {
    /// No active blueprint (None or idle).
    Idle,
    /// Blueprint in progress with current position.
    InProgress {
        id: String,
        stage: String,
        section: Option<String>,
        task: Option<String>,
    },
    /// Blueprint marked complete.
    Complete(String),
}

/// Severity level for health check issues.
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Warning,
    Error,
}

/// A single issue found during health check.
#[derive(Debug, Clone)]
pub struct HealthIssue {
    pub severity: Severity,
    pub description: String,
    pub fixed: bool,
}

/// Result of a blueprint health check.
#[derive(Debug, Clone, Default)]
pub struct HealthReport {
    pub issues: Vec<HealthIssue>,
    pub fixed: Vec<String>,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FinDir) {
        let tmp = TempDir::new().unwrap();
        let fin = FinDir::new(tmp.path());
        (tmp, fin)
    }

    #[test]
    fn test_init_creates_structure() {
        let (_tmp, fin) = setup();
        assert!(!fin.exists());
        fin.init().unwrap();
        assert!(fin.exists());
        assert!(fin.status_path().exists());
        assert!(fin.ledger_path().exists());
        assert!(fin.root().join("blueprints").is_dir());
        // Verify agent seeding
        let agents_dir = fin.agents_dir();
        assert!(agents_dir.is_dir());
        assert!(agents_dir.join("fin-researcher.md").exists());
        assert!(agents_dir.join("fin-planner.md").exists());
        assert!(agents_dir.join("fin-builder.md").exists());
        assert!(agents_dir.join("fin-reviewer.md").exists());
    }

    #[test]
    fn test_init_state_content() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let state = fin.read_state().unwrap();
        assert!(state.contains("**Active Blueprint:** None"));
        assert!(state.contains("**Stage:** Idle"));
    }

    #[test]
    fn test_init_decisions_content() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let dec = fs::read_to_string(fin.ledger_path()).unwrap();
        assert!(dec.contains("| # | When | Scope |"));
    }

    #[test]
    fn test_blueprint_paths() {
        let (_tmp, fin) = setup();
        assert!(fin.blueprint_dir("B001").ends_with(".fin/blueprints/B001"));
        assert!(
            fin.blueprint_vision("B001")
                .ends_with("B001/B001-VISION.md")
        );
        assert!(fin.blueprint_brief("B001").ends_with("B001/B001-BRIEF.md"));
        assert!(
            fin.blueprint_findings("B001")
                .ends_with("B001/B001-FINDINGS.md")
        );
        assert!(
            fin.blueprint_report("B001")
                .ends_with("B001/B001-REPORT.md")
        );
    }

    #[test]
    fn test_section_paths() {
        let (_tmp, fin) = setup();
        assert!(
            fin.section_dir("B001", "S01")
                .ends_with("B001/sections/S01")
        );
        assert!(fin.section_spec("B001", "S01").ends_with("S01/S01-SPEC.md"));
        assert!(
            fin.section_handoff("B001", "S01")
                .ends_with("S01/handoff.md")
        );
    }

    #[test]
    fn test_task_paths() {
        let (_tmp, fin) = setup();
        assert!(fin.tasks_dir("B001", "S01").ends_with("S01/tasks"));
        assert!(
            fin.task_spec("B001", "S01", "T01")
                .ends_with("tasks/T01-SPEC.md")
        );
        assert!(
            fin.task_report("B001", "S01", "T01")
                .ends_with("tasks/T01-REPORT.md")
        );
    }

    #[test]
    fn test_create_blueprint_and_list() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fin.create_blueprint("B002").unwrap();

        let blueprints = fin.list_blueprints();
        assert_eq!(blueprints, vec!["B001", "B002"]);
    }

    #[test]
    fn test_create_section_and_list() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fin.create_section("B001", "S02").unwrap();
        fin.create_section("B001", "S01").unwrap();

        let sections = fin.list_sections("B001");
        assert_eq!(sections, vec!["S01", "S02"]);
    }

    #[test]
    fn test_list_tasks() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fin.create_section("B001", "S01").unwrap();

        // Write some task files
        let tasks_dir = fin.tasks_dir("B001", "S01");
        fs::write(tasks_dir.join("T01-SPEC.md"), "spec").unwrap();
        fs::write(tasks_dir.join("T01-REPORT.md"), "report").unwrap();
        fs::write(tasks_dir.join("T02-SPEC.md"), "spec2").unwrap();

        let tasks = fin.list_tasks("B001", "S01");
        assert_eq!(tasks, vec!["T01", "T02"]);
    }

    #[test]
    fn test_read_write_file() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();

        fin.write_file("test/nested/file.md", "hello").unwrap();
        assert_eq!(fin.read_file("test/nested/file.md").unwrap(), "hello");
    }

    #[test]
    fn test_read_missing_file() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        assert!(fin.read_file("nonexistent.md").is_none());
    }

    #[test]
    fn test_write_and_read_state() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.write_state("new state").unwrap();
        assert_eq!(fin.read_state().unwrap(), "new state");
    }

    #[test]
    fn test_append_decision() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.append_decision("D001", "B001", "Use Rust", "Rust", "Performance")
            .unwrap();
        fin.append_decision("D002", "B001/S01", "Use Axum", "Axum", "Ergonomics")
            .unwrap();

        let content = fs::read_to_string(fin.ledger_path()).unwrap();
        assert!(content.contains("| D001 |"));
        assert!(content.contains("| D002 |"));
        assert!(content.contains("| Use Rust |"));
        assert!(content.contains("| Axum |"));
    }

    #[test]
    fn test_db_path() {
        let (_tmp, fin) = setup();
        assert!(fin.db_path().ends_with(".fin/fin.db"));
    }

    #[test]
    fn test_list_blueprints_empty() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        assert!(fin.list_blueprints().is_empty());
    }

    #[test]
    fn test_list_sections_no_blueprint() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        assert!(fin.list_sections("B999").is_empty());
    }

    #[test]
    fn test_list_tasks_no_section() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        assert!(fin.list_tasks("B001", "S01").is_empty());
    }

    // ── BlueprintStatus tests ──────────────────────────────────────

    #[test]
    fn test_status_idle_no_fin_dir() {
        let tmp = TempDir::new().unwrap();
        let fin = FinDir::new(tmp.path());
        assert_eq!(fin.active_blueprint_status(), BlueprintStatus::Idle);
    }

    #[test]
    fn test_status_idle_none() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        assert_eq!(fin.active_blueprint_status(), BlueprintStatus::Idle);
    }

    #[test]
    fn test_status_in_progress() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            Some("S01 — (active)"),
            Some("T02 — (active)"),
            "build",
            "Building T02.",
        );
        fin.write_state(&state).unwrap();

        match fin.active_blueprint_status() {
            BlueprintStatus::InProgress {
                id,
                stage,
                section,
                task,
            } => {
                assert_eq!(id, "B001");
                assert_eq!(stage, "build");
                assert_eq!(section, Some("S01".into()));
                assert_eq!(task, Some("T02".into()));
            }
            other => panic!("Expected InProgress, got {:?}", other),
        }
    }

    #[test]
    fn test_status_complete() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — COMPLETE",
            None,
            None,
            "idle",
            "Done.",
        );
        fin.write_state(&state).unwrap();

        match fin.active_blueprint_status() {
            BlueprintStatus::Complete(id) => assert_eq!(id, "B001"),
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_status_in_progress_no_section() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            None,
            None,
            "define",
            "Start defining.",
        );
        fin.write_state(&state).unwrap();

        match fin.active_blueprint_status() {
            BlueprintStatus::InProgress {
                id,
                stage,
                section,
                task,
            } => {
                assert_eq!(id, "B001");
                assert_eq!(stage, "define");
                assert_eq!(section, None);
                assert_eq!(task, None);
            }
            other => panic!("Expected InProgress, got {:?}", other),
        }
    }

    // ── Health check tests ─────────────────────────────────────────

    #[test]
    fn test_health_check_no_active_blueprint() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let report = fin.blueprint_health_check();
        assert!(report.issues.is_empty());
        assert!(report.summary.contains("No active blueprint"));
    }

    #[test]
    fn test_health_check_missing_blueprint_dir() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        // Set STATUS to active blueprint but don't create the dir
        let state =
            crate::workflow::markdown::status_template("B001 — MVP", None, None, "define", "Go.");
        fin.write_state(&state).unwrap();

        let report = fin.blueprint_health_check();
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, Severity::Error);
        // Should have reset to idle
        assert_eq!(fin.active_blueprint_status(), BlueprintStatus::Idle);
    }

    #[test]
    fn test_health_check_stage_without_brief() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        // Stage says explore but no BRIEF.md
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            None,
            None,
            "explore",
            "Exploring.",
        );
        fin.write_state(&state).unwrap();

        let report = fin.blueprint_health_check();
        assert!(!report.issues.is_empty());
        assert!(report.fixed.iter().any(|f| f.contains("define")));
    }

    #[test]
    fn test_health_check_stale_section_reference() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fs::write(fin.blueprint_brief("B001"), "brief").unwrap();
        fs::write(fin.blueprint_findings("B001"), "findings").unwrap();
        // Reference section that doesn't exist
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            Some("S99 — (active)"),
            None,
            "build",
            "Building.",
        );
        fin.write_state(&state).unwrap();

        let report = fin.blueprint_health_check();
        assert!(report.fixed.iter().any(|f| f.contains("S99")));
    }

    #[test]
    fn test_health_check_orphaned_markers() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fin.create_section("B001", "S01").unwrap();
        // Write a task marker file without a SPEC
        let tasks_dir = fin.tasks_dir("B001", "S01");
        fs::write(tasks_dir.join("T01-REPORT.md"), "report").unwrap();
        let task_dir = fin.task_dir("B001", "S01", "T01");
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(task_dir.join(".executed"), "").unwrap();

        let state =
            crate::workflow::markdown::status_template("B001 — MVP", None, None, "define", "Go.");
        fin.write_state(&state).unwrap();

        let report = fin.blueprint_health_check();
        assert!(report.fixed.iter().any(|f| f.contains("orphaned")));
        // Marker should be removed
        assert!(!task_dir.join(".executed").exists());
    }

    #[test]
    fn test_health_check_healthy_state() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fs::write(fin.blueprint_vision("B001"), "vision").unwrap();
        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            None,
            None,
            "define",
            "Start.",
        );
        fin.write_state(&state).unwrap();

        let report = fin.blueprint_health_check();
        assert!(report.issues.is_empty());
        assert!(report.summary.contains("healthy"));
    }

    // ── Progress summary tests ─────────────────────────────────────

    #[test]
    fn test_progress_summary_idle() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        let summary = fin.blueprint_progress_summary();
        assert!(summary.contains("No active blueprint"));
    }

    #[test]
    fn test_progress_summary_in_progress() {
        let (_tmp, fin) = setup();
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fs::write(fin.blueprint_vision("B001"), "vision").unwrap();
        fs::write(fin.blueprint_brief("B001"), "brief").unwrap();
        fin.create_section("B001", "S01").unwrap();
        fs::write(fin.section_spec("B001", "S01"), "spec").unwrap();

        let state = crate::workflow::markdown::status_template(
            "B001 — MVP",
            Some("S01 — (active)"),
            None,
            "build",
            "Building.",
        );
        fin.write_state(&state).unwrap();

        let summary = fin.blueprint_progress_summary();
        assert!(summary.contains("B001"));
        assert!(summary.contains("BRIEF.md:    done"));
        assert!(summary.contains("FINDINGS.md: pending"));
        assert!(summary.contains("S01"));
        assert!(summary.contains("stage=build"));
    }
}
