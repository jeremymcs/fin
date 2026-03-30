// Fin — Continue/Resume Protocol
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use serde::{Deserialize, Serialize};

use super::state::FinDir;

/// State captured when a task is interrupted and needs to be resumed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueState {
    pub blueprint_id: String,
    pub section_id: String,
    pub task_id: String,
    pub step: u32,
    pub total_steps: u32,
    pub completed_work: String,
    pub remaining_work: String,
    pub decisions_made: String,
    pub context: String,
    pub next_action: String,
    pub saved_at: String,
}

/// Write a handoff.md file for a section, filling in all fields from the
/// ContinueState into the YAML frontmatter and markdown body.
pub fn write_continue(
    fin_dir: &FinDir,
    b_id: &str,
    s_id: &str,
    state: &ContinueState,
) -> anyhow::Result<()> {
    let content = format!(
        "---\n\
         blueprint_id: {blueprint}\n\
         section_id: {section}\n\
         task_id: {task}\n\
         step: {step}\n\
         total_steps: {total}\n\
         saved_at: \"{saved_at}\"\n\
         ---\n\
         \n\
         # Continue: {task} (Step {step}/{total})\n\
         \n\
         ## Completed Work\n\
         \n\
         {completed}\n\
         \n\
         ## Remaining Work\n\
         \n\
         {remaining}\n\
         \n\
         ## Decisions Made\n\
         \n\
         {decisions}\n\
         \n\
         ## Context\n\
         \n\
         {context}\n\
         \n\
         ## Next Action\n\
         \n\
         {next_action}\n",
        blueprint = state.blueprint_id,
        section = state.section_id,
        task = state.task_id,
        step = state.step,
        total = state.total_steps,
        saved_at = state.saved_at,
        completed = state.completed_work,
        remaining = state.remaining_work,
        decisions = state.decisions_made,
        context = state.context,
        next_action = state.next_action,
    );

    let path = fin_dir.section_handoff(b_id, s_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(())
}

/// Read and parse a handoff.md file, returning None if it does not exist
/// or cannot be parsed.
pub fn read_continue(fin_dir: &FinDir, b_id: &str, s_id: &str) -> Option<ContinueState> {
    let path = fin_dir.section_handoff(b_id, s_id);
    let content = std::fs::read_to_string(&path).ok()?;

    let (yaml_str, body) = split_frontmatter(&content)?;

    let fm: ContinueFrontmatter = serde_yaml::from_str(yaml_str).ok()?;

    let completed_work = extract_section(body, "## Completed Work");
    let remaining_work = extract_section(body, "## Remaining Work");
    let decisions_made = extract_section(body, "## Decisions Made");
    let context = extract_section(body, "## Context");
    let next_action = extract_section(body, "## Next Action");

    Some(ContinueState {
        blueprint_id: fm.blueprint_id,
        section_id: fm.section_id,
        task_id: fm.task_id,
        step: fm.step,
        total_steps: fm.total_steps,
        completed_work,
        remaining_work,
        decisions_made,
        context,
        next_action,
        saved_at: fm.saved_at.unwrap_or_default(),
    })
}

/// Delete the handoff.md file for a section.
pub fn remove_continue(fin_dir: &FinDir, b_id: &str, s_id: &str) -> anyhow::Result<()> {
    let path = fin_dir.section_handoff(b_id, s_id);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Check whether a handoff.md exists for the given section.
pub fn has_continue(fin_dir: &FinDir, b_id: &str, s_id: &str) -> bool {
    fin_dir.section_handoff(b_id, s_id).exists()
}

// ── Private helpers ─────────────────────────────────────────────────

/// YAML frontmatter shape for handoff.md deserialization.
#[derive(Deserialize)]
struct ContinueFrontmatter {
    blueprint_id: String,
    section_id: String,
    task_id: String,
    step: u32,
    total_steps: u32,
    saved_at: Option<String>,
}

/// Split YAML frontmatter from markdown body.
/// Expects `---\n<yaml>\n---\n<body>`.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..].trim_start_matches(['\r', '\n']);
    let end_idx = after_first.find("\n---")?;

    let yaml = &after_first[..end_idx];
    let body = &after_first[end_idx + 4..]; // skip "\n---"

    Some((yaml, body))
}

/// Extract the text content under a markdown heading, up to the next
/// heading of the same level or end of document.
fn extract_section(body: &str, heading: &str) -> String {
    let Some(start) = body.find(heading) else {
        return String::new();
    };

    let after_heading = &body[start + heading.len()..];
    let end = after_heading.find("\n## ").unwrap_or(after_heading.len());

    after_heading[..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FinDir) {
        let tmp = TempDir::new().unwrap();
        let fin = FinDir::new(tmp.path());
        fin.init().unwrap();
        fin.create_blueprint("B001").unwrap();
        fin.create_section("B001", "S01").unwrap();
        (tmp, fin)
    }

    fn sample_state() -> ContinueState {
        ContinueState {
            blueprint_id: "B001".into(),
            section_id: "S01".into(),
            task_id: "T01".into(),
            step: 3,
            total_steps: 7,
            completed_work: "Implemented the parser.".into(),
            remaining_work: "Write tests and docs.".into(),
            decisions_made: "Used serde_yaml for parsing.".into(),
            context: "The parser handles YAML frontmatter.".into(),
            next_action: "Write unit tests for edge cases.".into(),
            saved_at: "2026-03-30T12:00:00Z".into(),
        }
    }

    #[test]
    fn test_write_and_read_continue() {
        let (_tmp, fin) = setup();
        let state = sample_state();

        write_continue(&fin, "B001", "S01", &state).unwrap();
        assert!(has_continue(&fin, "B001", "S01"));

        let loaded = read_continue(&fin, "B001", "S01").unwrap();
        assert_eq!(loaded.blueprint_id, "B001");
        assert_eq!(loaded.section_id, "S01");
        assert_eq!(loaded.task_id, "T01");
        assert_eq!(loaded.step, 3);
        assert_eq!(loaded.total_steps, 7);
        assert_eq!(loaded.completed_work, "Implemented the parser.");
        assert_eq!(loaded.remaining_work, "Write tests and docs.");
        assert_eq!(loaded.decisions_made, "Used serde_yaml for parsing.");
        assert_eq!(loaded.context, "The parser handles YAML frontmatter.");
        assert_eq!(loaded.next_action, "Write unit tests for edge cases.");
        assert_eq!(loaded.saved_at, "2026-03-30T12:00:00Z");
    }

    #[test]
    fn test_has_continue_false() {
        let (_tmp, fin) = setup();
        assert!(!has_continue(&fin, "B001", "S01"));
    }

    #[test]
    fn test_remove_continue() {
        let (_tmp, fin) = setup();
        let state = sample_state();

        write_continue(&fin, "B001", "S01", &state).unwrap();
        assert!(has_continue(&fin, "B001", "S01"));

        remove_continue(&fin, "B001", "S01").unwrap();
        assert!(!has_continue(&fin, "B001", "S01"));
    }

    #[test]
    fn test_remove_continue_nonexistent() {
        let (_tmp, fin) = setup();
        remove_continue(&fin, "B001", "S01").unwrap();
    }

    #[test]
    fn test_read_continue_nonexistent() {
        let (_tmp, fin) = setup();
        assert!(read_continue(&fin, "B001", "S01").is_none());
    }

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nkey: value\n---\nBody text";
        let (yaml, body) = split_frontmatter(content).unwrap();
        assert_eq!(yaml, "key: value");
        assert_eq!(body.trim(), "Body text");
    }

    #[test]
    fn test_split_frontmatter_no_markers() {
        assert!(split_frontmatter("no frontmatter here").is_none());
    }

    #[test]
    fn test_extract_section() {
        let body = "\n## First\n\nContent one.\n\n## Second\n\nContent two.\n";
        assert_eq!(extract_section(body, "## First"), "Content one.");
        assert_eq!(extract_section(body, "## Second"), "Content two.");
        assert_eq!(extract_section(body, "## Missing"), "");
    }

    #[test]
    fn test_roundtrip_with_multiline_content() {
        let (_tmp, fin) = setup();
        let state = ContinueState {
            blueprint_id: "B001".into(),
            section_id: "S01".into(),
            task_id: "T02".into(),
            step: 1,
            total_steps: 3,
            completed_work: "Line one.\nLine two.".into(),
            remaining_work: "- Item A\n- Item B".into(),
            decisions_made: "None yet.".into(),
            context: "Multi\nline\ncontext.".into(),
            next_action: "Start step 2.".into(),
            saved_at: "2026-03-30T15:00:00Z".into(),
        };

        write_continue(&fin, "B001", "S01", &state).unwrap();
        let loaded = read_continue(&fin, "B001", "S01").unwrap();
        assert_eq!(loaded.completed_work, "Line one.\nLine two.");
        assert_eq!(loaded.remaining_work, "- Item A\n- Item B");
        assert_eq!(loaded.next_action, "Start step 2.");
    }
}
