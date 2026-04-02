// Fin — System Prompt Builder
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::llm::types::ToolSchema;
use crate::skills;
use std::path::Path;

/// Optional context for agent specialization and delegation.
pub struct AgentPromptContext {
    /// For parent agents: markdown summary of available sub-agents
    pub available_agents: Option<String>,
    /// For sub-agents: the agent definition's markdown body (role/instructions)
    pub agent_role: Option<String>,
}

/// Build the system prompt for the agent.
pub fn build_system_prompt(
    tools: &[ToolSchema],
    cwd: &Path,
    agent_context: Option<&AgentPromptContext>,
) -> String {
    let mut prompt = String::new();

    // Identity — use agent role if this is a sub-agent, otherwise default
    if let Some(ctx) = agent_context {
        if let Some(role) = &ctx.agent_role {
            prompt.push_str(role);
            prompt.push_str("\n\n");
        } else {
            prompt.push_str("You are Fin, an AI coding agent. You help users with software engineering tasks by reading files, writing code, running commands, and managing git repositories.\n\n");
        }
    } else {
        prompt.push_str("You are Fin, an AI coding agent. You help users with software engineering tasks by reading files, writing code, running commands, and managing git repositories.\n\n");
    }

    // Guidelines
    prompt.push_str("# Guidelines\n");
    prompt.push_str("- Read files before modifying them. Never guess at code you haven't seen.\n");
    prompt.push_str("- Keep changes minimal and focused. Don't refactor beyond what was asked.\n");
    prompt.push_str("- Run builds and tests after making changes to verify correctness.\n");
    prompt.push_str("- Use git to commit changes with clear, conventional commit messages.\n");
    prompt.push_str("- Be concise in responses. Lead with the action, not the reasoning.\n");
    prompt.push_str("- If a tool call fails, diagnose the error before retrying.\n");
    prompt.push_str("- Never introduce security vulnerabilities (injection, XSS, etc.).\n");
    prompt.push_str("- Prefer editing existing files over creating new ones.\n\n");

    // Load skills from ~/.config/fin/skills/
    if let Ok(paths) = crate::config::paths::FinPaths::resolve() {
        let loaded_skills = skills::load_skills(&paths.skills_dir);
        if !loaded_skills.is_empty() {
            prompt.push_str("# Skills\n\n");
            for skill in &loaded_skills {
                prompt.push_str(&format!("## {}\n\n{}\n\n", skill.name, skill.content));
            }
        }
    }

    // Load project context files
    if let Some(context) = load_context_files(cwd) {
        prompt.push_str("# Project Context\n\n");
        prompt.push_str(&context);
        prompt.push_str("\n\n");
    }

    // Tools
    if !tools.is_empty() {
        prompt.push_str("# Available Tools\n\n");
        for tool in tools {
            prompt.push_str(&format!("## {}\n{}\n\n", tool.name, tool.description));

            if let Some(props) = tool.parameters.get("properties") {
                if let Some(obj) = props.as_object() {
                    prompt.push_str("Parameters:\n");
                    for (name, schema) in obj {
                        let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("any");
                        let desc = schema
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        let required = tool
                            .parameters
                            .get("required")
                            .and_then(|r| r.as_array())
                            .map(|arr| arr.iter().any(|v| v.as_str() == Some(name)))
                            .unwrap_or(false);
                        let req_marker = if required { " (required)" } else { "" };
                        prompt.push_str(&format!("- `{name}` ({type_str}{req_marker}): {desc}\n"));
                    }
                    prompt.push('\n');
                }
            }
        }
    }

    // Tool usage guidelines
    prompt.push_str("# Tool Usage\n");
    prompt.push_str("- Use `read` to view files before editing them.\n");
    prompt.push_str(
        "- Use `edit` for targeted changes (string replacement). Use `write` only for new files.\n",
    );
    prompt.push_str("- Use `bash` for running commands, builds, tests, and system operations.\n");
    prompt.push_str("- Use `grep` to search code. Use `glob` to find files by pattern.\n");
    prompt.push_str("- Use `git` for version control operations.\n");
    prompt.push_str("- When multiple tools are needed, call them in sequence — one per turn.\n\n");

    // Available agents for delegation
    if let Some(ctx) = agent_context {
        if let Some(agents_summary) = &ctx.available_agents {
            if !agents_summary.is_empty() {
                prompt.push_str("# Available Agents for Delegation\n\n");
                prompt.push_str(agents_summary);
                prompt.push_str("\n\n");
            }
        }
    }

    // Extension prompt additions
    let cwd_for_ext = cwd.to_path_buf();
    let ext_ctx = crate::extensions::api::ExtensionContext {
        cwd: cwd_for_ext,
        session_id: String::new(),
    };
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    let ext_additions = ext_registry.prompt_additions(&ext_ctx);
    if !ext_additions.is_empty() {
        prompt.push_str("# Extension Context\n\n");
        prompt.push_str(&ext_additions);
        prompt.push_str("\n\n");
    }

    // Environment
    prompt.push_str("# Environment\n");
    prompt.push_str(&format!("- Working directory: {}\n", cwd.display()));
    prompt.push_str(&format!(
        "- Date: {}\n",
        chrono::Utc::now().format("%Y-%m-%d")
    ));
    prompt.push_str(&format!("- Platform: {}\n", std::env::consts::OS));

    // Git info
    if let Some(git_info) = detect_git_info(cwd) {
        prompt.push_str(&format!("- Git branch: {}\n", git_info));
    }

    prompt
}

/// Load context files: CLAUDE.md, CONTRIBUTING.md, .fin/ state, etc.
fn load_context_files(cwd: &Path) -> Option<String> {
    let mut context = String::new();

    // Priority order for context files
    let context_files = [
        "CLAUDE.md",
        ".claude/CLAUDE.md",
        "CONTRIBUTING.md",
        "AGENTS.md",
        ".fin/STATE.md",
        ".fin/DECISIONS.md",
    ];

    for filename in &context_files {
        let path = cwd.join(filename);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let content = if content.len() > 8000 {
                    format!("{}...\n(truncated)", &content[..8000])
                } else {
                    content
                };
                context.push_str(&format!("## {filename}\n\n{content}\n\n"));
            }
        }
    }

    // Load codebase map if present (written by `fin map`)
    let map_path = cwd.join(".fin/CODEBASE_MAP.md");
    if map_path.exists() {
        if let Ok(map_content) = std::fs::read_to_string(&map_path) {
            // Staleness check: compare git-head in map vs current HEAD
            let stale_warning = check_map_staleness(cwd, &map_content);

            let capped = if map_content.len() > 5000 {
                format!(
                    "{}...\n(map truncated — run `fin map` to refresh)",
                    &map_content[..5000]
                )
            } else {
                map_content
            };

            context.push_str("## Codebase Map (.fin/CODEBASE_MAP.md)\n\n");
            if let Some(warning) = stale_warning {
                context.push_str(&format!("**{warning}**\n\n"));
            }
            context.push_str(&capped);
            context.push_str("\n\n");
        }
    }

    // Project type detection — gives the agent context about the stack
    if let Some(project_info) = detect_project_type(cwd) {
        context.push_str(&format!("## Project\n\n{project_info}\n\n"));
    }

    if context.is_empty() {
        None
    } else {
        Some(context)
    }
}

/// Detect project type from manifest files and give the agent stack context.
fn detect_project_type(cwd: &Path) -> Option<String> {
    let mut info = Vec::new();

    if cwd.join("Cargo.toml").exists() {
        let mut desc = "Rust project (Cargo)".to_string();
        if let Ok(content) = std::fs::read_to_string(cwd.join("Cargo.toml")) {
            if let Some(name) = content
                .lines()
                .find(|l| l.starts_with("name"))
                .and_then(|l| l.split('=').nth(1))
            {
                desc = format!("Rust project: {}", name.trim().trim_matches('"'));
            }
        }
        info.push(desc);
    }

    if cwd.join("package.json").exists() {
        let mut desc = "Node.js project".to_string();
        if let Ok(content) = std::fs::read_to_string(cwd.join("package.json")) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let name = pkg["name"].as_str().unwrap_or("unknown");
                desc = format!("Node.js project: {name}");
                // Detect framework
                let deps_str = format!(
                    "{}{}",
                    pkg.get("dependencies")
                        .map(|d| d.to_string())
                        .unwrap_or_default(),
                    pkg.get("devDependencies")
                        .map(|d| d.to_string())
                        .unwrap_or_default(),
                );
                if deps_str.contains("next") {
                    desc.push_str(" (Next.js)");
                } else if deps_str.contains("react") {
                    desc.push_str(" (React)");
                } else if deps_str.contains("vue") {
                    desc.push_str(" (Vue)");
                } else if deps_str.contains("express") {
                    desc.push_str(" (Express)");
                }
            }
        }
        info.push(desc);
    }

    if cwd.join("go.mod").exists() {
        info.push("Go project".to_string());
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        info.push("Python project".to_string());
    }
    if cwd.join("Gemfile").exists() {
        info.push("Ruby project".to_string());
    }
    if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        info.push("Java/JVM project".to_string());
    }

    if info.is_empty() {
        None
    } else {
        Some(info.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_prompt_contains_identity() {
        let prompt = build_system_prompt(&[], Path::new("/tmp"), None);
        assert!(prompt.contains("Fin"));
        assert!(prompt.contains("AI coding agent"));
    }

    #[test]
    fn test_build_system_prompt_contains_guidelines() {
        let prompt = build_system_prompt(&[], Path::new("/tmp"), None);
        assert!(prompt.contains("Guidelines"));
        assert!(prompt.contains("Read files before modifying"));
    }

    #[test]
    fn test_build_system_prompt_contains_environment() {
        let prompt = build_system_prompt(&[], Path::new("/tmp"), None);
        assert!(prompt.contains("Environment"));
        assert!(prompt.contains("/tmp"));
    }

    #[test]
    fn test_build_system_prompt_with_tools() {
        let tools = vec![crate::llm::types::ToolSchema {
            name: "test_tool".into(),
            description: "A test tool".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }];
        let prompt = build_system_prompt(&tools, Path::new("/tmp"), None);
        assert!(prompt.contains("test_tool"));
        assert!(prompt.contains("A test tool"));
    }

    #[test]
    fn test_build_system_prompt_with_agent_role() {
        let ctx = AgentPromptContext {
            available_agents: None,
            agent_role: Some("You are a code reviewer.".into()),
        };
        let prompt = build_system_prompt(&[], Path::new("/tmp"), Some(&ctx));
        assert!(prompt.contains("code reviewer"));
        // Should NOT contain default identity when role is set
        assert!(!prompt.starts_with("You are Fin"));
    }

    #[test]
    fn test_build_system_prompt_with_agents() {
        let ctx = AgentPromptContext {
            available_agents: Some("| Agent | Description |\n|-------|-------------|".into()),
            agent_role: None,
        };
        let prompt = build_system_prompt(&[], Path::new("/tmp"), Some(&ctx));
        assert!(prompt.contains("Available Agents"));
    }

    #[test]
    fn test_detect_project_type_rust() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"myapp\"").unwrap();
        let info = detect_project_type(tmp.path()).unwrap();
        assert!(info.contains("Rust"));
        assert!(info.contains("myapp"));
    }

    #[test]
    fn test_detect_project_type_node() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"webapp","dependencies":{"next":"14"}}"#,
        )
        .unwrap();
        let info = detect_project_type(tmp.path()).unwrap();
        assert!(info.contains("Node.js"));
        assert!(info.contains("Next.js"));
    }

    #[test]
    fn test_detect_project_type_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_project_type(tmp.path()).is_none());
    }

    #[test]
    fn test_load_context_files_with_contributing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CONTRIBUTING.md"),
            "# Contributing\nRules here.",
        )
        .unwrap();
        let ctx = load_context_files(tmp.path()).unwrap();
        assert!(ctx.contains("CONTRIBUTING.md"));
        assert!(ctx.contains("Rules here"));
    }
}

/// Check if CODEBASE_MAP.md is stale by comparing the embedded git-head with current HEAD.
/// Returns a warning string if stale, None if current or unable to determine.
fn check_map_staleness(cwd: &Path, map_content: &str) -> Option<String> {
    // Extract git-head from map comment: <!-- git-head: abc1234 -->
    let map_head = map_content
        .lines()
        .find(|l| l.contains("git-head:"))?
        .split("git-head:")
        .nth(1)?
        .trim()
        .trim_end_matches("-->")
        .trim()
        .to_string();

    if map_head.is_empty() {
        return None;
    }

    // Get current HEAD
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let current_head = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if current_head != map_head {
        Some(format!(
            "CODEBASE_MAP may be stale (mapped at {map_head}, current HEAD is {current_head}). Run `fin map` to refresh."
        ))
    } else {
        None
    }
}

/// Detect current git branch.
fn detect_git_info(cwd: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}
