// Fin + Agent Definition Parser (Claude Code agent format)

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Parsed agent definition from a ~/.claude/agents/*.md file.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    /// Filename stem, e.g. "backend-architect"
    pub id: String,
    /// Human-readable name from frontmatter
    #[allow(dead_code)]
    pub name: String,
    /// One-line description
    pub description: String,
    /// UI color hint
    #[allow(dead_code)]
    pub color: String,
    /// Model tier: "opus", "sonnet", "haiku"
    pub model_tier: String,
    /// Normalized lowercase tool names: ["read", "grep", "glob"]
    pub tools: Vec<String>,
    /// Memory mode (e.g. "user") if enabled
    #[allow(dead_code)]
    pub memory: Option<String>,
    /// Permission mode (e.g. "acceptEdits")
    #[allow(dead_code)]
    pub permission_mode: Option<String>,
    /// Workflow roles this agent can fulfill (e.g. "researcher", "reviewer").
    /// Stages use roles to find compatible agents for delegation.
    pub roles: Vec<String>,
    /// Markdown body — the agent's system prompt
    pub system_prompt: String,
    /// Original file path
    #[allow(dead_code)]
    pub source_path: PathBuf,
}

/// Raw YAML frontmatter for serde deserialization.
#[derive(Deserialize)]
struct AgentFrontmatter {
    name: String,
    description: String,
    #[serde(default = "default_color")]
    color: String,
    tools: String,
    #[serde(default = "default_model")]
    model: String,
    memory: Option<String>,
    #[serde(rename = "permissionMode")]
    permission_mode: Option<String>,
    /// Comma-separated workflow roles (e.g. "researcher, reviewer").
    #[serde(default)]
    roles: Option<String>,
}

fn default_model() -> String {
    "sonnet".into()
}

fn default_color() -> String {
    "cyan".into()
}

/// Parse a single agent file. Returns None if parsing fails.
pub fn parse_agent_file(path: &Path) -> Option<AgentDefinition> {
    let content = std::fs::read_to_string(path).ok()?;
    let (yaml_str, body) = split_frontmatter(&content)?;

    let fm: AgentFrontmatter = serde_yaml::from_str(yaml_str).ok()?;

    let tools: Vec<String> = fm
        .tools
        .split(',')
        .map(|s| normalize_tool_name(s.trim()))
        .filter(|s| !s.is_empty())
        .collect();

    let id = path.file_stem()?.to_str()?.to_string();

    let roles: Vec<String> = fm
        .roles
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    Some(AgentDefinition {
        id,
        name: fm.name,
        description: fm.description,
        color: fm.color,
        model_tier: fm.model.to_lowercase(),
        tools,
        memory: fm.memory,
        permission_mode: fm.permission_mode,
        roles,
        system_prompt: body.trim().to_string(),
        source_path: path.to_path_buf(),
    })
}

/// Split YAML frontmatter from markdown body.
/// Expects `---\n<yaml>\n---\n<body>`.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing ---
    let after_first = &trimmed[3..].trim_start_matches(['\r', '\n']);
    let end_idx = after_first.find("\n---")?;

    let yaml = &after_first[..end_idx];
    let body = &after_first[end_idx + 4..]; // skip "\n---"

    Some((yaml, body))
}

/// Normalize tool names from Title Case to internal lowercase names.
fn normalize_tool_name(name: &str) -> String {
    // Pass through MCP tool patterns unchanged
    if name.starts_with("mcp__") {
        return name.to_string();
    }

    match name {
        "Bash" => "bash".to_string(),
        "Read" => "read".to_string(),
        "Write" => "write".to_string(),
        "Edit" => "edit".to_string(),
        "Grep" => "grep".to_string(),
        "Glob" => "glob".to_string(),
        "Git" => "git".to_string(),
        "WebSearch" => "web_search".to_string(),
        "WebFetch" => "web_fetch".to_string(),
        "Task" => "task".to_string(),
        "Delegate" => "delegate".to_string(),
        "TodoWrite" => "todo_write".to_string(),
        "NotebookEdit" => "notebook_edit".to_string(),
        // Fallback: lowercase the input
        other => other.to_lowercase().replace(' ', "_"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nname: test\n---\nBody here";
        let (yaml, body) = split_frontmatter(content).unwrap();
        assert_eq!(yaml, "name: test");
        assert_eq!(body.trim(), "Body here");
    }

    #[test]
    fn test_normalize_tool_name() {
        assert_eq!(normalize_tool_name("Bash"), "bash");
        assert_eq!(normalize_tool_name("WebSearch"), "web_search");
        assert_eq!(
            normalize_tool_name("mcp__context7__resolve"),
            "mcp__context7__resolve"
        );
        assert_eq!(normalize_tool_name("UnknownTool"), "unknowntool");
        assert_eq!(normalize_tool_name("Web Fetch"), "web_fetch");
    }

    #[test]
    fn test_parse_agent_content() {
        let content = r#"---
name: test-agent
description: "A test agent"
color: cyan
tools: Read, Write, Bash
model: opus
---

You are a test agent.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-agent.md");
        std::fs::write(&path, content).unwrap();

        let def = parse_agent_file(&path).unwrap();
        assert_eq!(def.id, "test-agent");
        assert_eq!(def.name, "test-agent");
        assert_eq!(def.model_tier, "opus");
        assert_eq!(def.tools, vec!["read", "write", "bash"]);
        assert!(def.roles.is_empty());
        assert_eq!(def.system_prompt, "You are a test agent.");
    }

    #[test]
    fn test_parse_agent_with_roles() {
        let content = r#"---
name: fin-researcher
description: "Research agent"
color: blue
tools: Read, Grep, Glob
model: sonnet
roles: researcher, explorer
---

You are a research agent.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fin-researcher.md");
        std::fs::write(&path, content).unwrap();

        let def = parse_agent_file(&path).unwrap();
        assert_eq!(def.id, "fin-researcher");
        assert_eq!(def.roles, vec!["researcher", "explorer"]);
    }

    #[test]
    fn test_parse_agent_single_role() {
        let content = r#"---
name: fin-reviewer
description: "Review agent"
color: green
tools: Read, Bash
roles: reviewer
---

You review code.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fin-reviewer.md");
        std::fs::write(&path, content).unwrap();

        let def = parse_agent_file(&path).unwrap();
        assert_eq!(def.roles, vec!["reviewer"]);
    }
}
