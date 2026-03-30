// Fin — Agent Registry (Discovery & Lookup)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use std::path::Path;

use super::definition::{AgentDefinition, parse_agent_file};

/// Registry of available sub-agents loaded from agent definition files.
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: Vec<AgentDefinition>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    /// Scan a directory for *.md agent files and parse all valid ones.
    pub fn load_from_dir(dir: &Path) -> Self {
        let mut registry = Self::new();

        if !dir.exists() {
            return registry;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return registry,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(def) = parse_agent_file(&path) {
                    registry.agents.push(def);
                }
            }
        }

        // Sort by name for consistent ordering
        registry.agents.sort_by(|a, b| a.id.cmp(&b.id));

        registry
    }

    /// Merge agents from another directory into this registry.
    /// Agents with duplicate IDs are skipped (first-loaded wins).
    pub fn load_additional(&mut self, dir: &Path) {
        if !dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(def) = parse_agent_file(&path) {
                    // Skip duplicates — first-loaded wins
                    if !self.agents.iter().any(|a| a.id == def.id) {
                        self.agents.push(def);
                    }
                }
            }
        }

        self.agents.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// Load from the default location (~/.claude/agents/).
    pub fn load_default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let agents_dir = home.join(".claude").join("agents");
        Self::load_from_dir(&agents_dir)
    }

    /// Load agents from all Fin-relevant locations.
    /// Priority: .fin/agents/ (project) > ~/.claude/agents/ (user).
    /// Project agents win on ID conflicts.
    /// Used by general chat — can delegate to any agent.
    pub fn load_for_project(project_root: &Path) -> Self {
        let fin_agents = project_root.join(".fin").join("agents");
        let mut registry = Self::load_from_dir(&fin_agents);

        // Merge user-level agents (skipping duplicates)
        let home = dirs::home_dir().unwrap_or_default();
        let user_agents = home.join(".claude").join("agents");
        registry.load_additional(&user_agents);

        registry
    }

    /// Load ONLY from .fin/agents/ — the workflow-controlled agent pool.
    /// Workflow stages delegate exclusively to these agents.
    /// External agents (e.g. ~/.claude/agents/) are NOT included.
    pub fn load_workflow_agents(project_root: &Path) -> Self {
        let fin_agents = project_root.join(".fin").join("agents");
        Self::load_from_dir(&fin_agents)
    }

    /// Look up an agent by id (filename stem).
    pub fn get(&self, id: &str) -> Option<&AgentDefinition> {
        self.agents.iter().find(|a| a.id == id)
    }

    /// List all available agents.
    #[allow(dead_code)]
    pub fn list(&self) -> &[AgentDefinition] {
        &self.agents
    }

    /// Number of loaded agents.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Find agents that fulfill a given workflow role (e.g. "researcher", "reviewer").
    /// Returns ALL matching agents regardless of ID prefix.
    /// For workflow-safe lookups, use `find_workflow_role()` instead.
    #[allow(dead_code)]
    pub fn find_by_role(&self, role: &str) -> Vec<&AgentDefinition> {
        let role_lower = role.to_lowercase();
        self.agents
            .iter()
            .filter(|a| a.roles.iter().any(|r| r == &role_lower))
            .collect()
    }

    /// Find workflow-safe agents for a given role.
    /// Only returns agents with `fin-` prefixed IDs — our seeded agents
    /// or user-customized versions of them. Rogue agents are excluded.
    pub fn find_workflow_role(&self, role: &str) -> Vec<&AgentDefinition> {
        let role_lower = role.to_lowercase();
        self.agents
            .iter()
            .filter(|a| a.id.starts_with("fin-"))
            .filter(|a| a.roles.iter().any(|r| r == &role_lower))
            .collect()
    }

    /// Build a summary for inclusion in the parent agent's system prompt.
    pub fn prompt_summary(&self) -> String {
        if self.agents.is_empty() {
            return String::new();
        }

        let mut summary = String::from(
            "You can delegate tasks to specialized sub-agents using the `delegate` tool.\n\n\
             | Agent ID | Description | Model Tier | Tools |\n\
             |----------|-------------|------------|-------|\n",
        );

        for agent in &self.agents {
            let tools_short = if agent.tools.len() > 4 {
                format!(
                    "{}, ... ({} total)",
                    agent.tools[..3].join(", "),
                    agent.tools.len()
                )
            } else {
                agent.tools.join(", ")
            };

            summary.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                agent.id, agent.description, agent.model_tier, tools_short
            ));
        }

        summary.push_str(
            "\nTo delegate: call the `delegate` tool with `agent` (the agent ID) and `task` (what to do).\n\
             For parallel work: use the `parallel` parameter with an array of {agent, task} objects.\n",
        );

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = AgentRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("anything").is_none());
        assert!(registry.prompt_summary().is_empty());
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let registry = AgentRegistry::load_from_dir(Path::new("/nonexistent/path"));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_and_lookup() {
        let dir = tempfile::tempdir().unwrap();

        let content = r#"---
name: test-agent
description: "A test agent"
color: cyan
tools: Read, Write
model: sonnet
---

You are a test agent."#;

        std::fs::write(dir.path().join("test-agent.md"), content).unwrap();

        let registry = AgentRegistry::load_from_dir(dir.path());
        assert_eq!(registry.len(), 1);

        let agent = registry.get("test-agent").unwrap();
        assert_eq!(agent.model_tier, "sonnet");
        assert_eq!(agent.tools, vec!["read", "write"]);
    }

    #[test]
    fn test_find_by_role() {
        let dir = tempfile::tempdir().unwrap();

        let researcher = r#"---
name: fin-researcher
description: "Research agent"
color: blue
tools: Read, Grep
model: sonnet
roles: researcher
---

Research things."#;

        let reviewer = r#"---
name: fin-reviewer
description: "Review agent"
color: green
tools: Read, Bash
model: sonnet
roles: reviewer, tester
---

Review things."#;

        let no_role = r#"---
name: generic-agent
description: "No roles"
color: cyan
tools: Read
model: haiku
---

Generic."#;

        std::fs::write(dir.path().join("fin-researcher.md"), researcher).unwrap();
        std::fs::write(dir.path().join("fin-reviewer.md"), reviewer).unwrap();
        std::fs::write(dir.path().join("generic-agent.md"), no_role).unwrap();

        let registry = AgentRegistry::load_from_dir(dir.path());
        assert_eq!(registry.len(), 3);

        let researchers = registry.find_by_role("researcher");
        assert_eq!(researchers.len(), 1);
        assert_eq!(researchers[0].id, "fin-researcher");

        let reviewers = registry.find_by_role("reviewer");
        assert_eq!(reviewers.len(), 1);
        assert_eq!(reviewers[0].id, "fin-reviewer");

        let testers = registry.find_by_role("tester");
        assert_eq!(testers.len(), 1);
        assert_eq!(testers[0].id, "fin-reviewer");

        let builders = registry.find_by_role("builder");
        assert!(builders.is_empty());
    }

    #[test]
    fn test_load_additional_skips_duplicates() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();

        let agent_v1 = r#"---
name: shared-agent
description: "Version 1 (project)"
color: blue
tools: Read
model: opus
roles: researcher
---

V1."#;

        let agent_v2 = r#"---
name: shared-agent
description: "Version 2 (user)"
color: green
tools: Read, Write
model: sonnet
---

V2."#;

        std::fs::write(dir1.path().join("shared-agent.md"), agent_v1).unwrap();
        std::fs::write(dir2.path().join("shared-agent.md"), agent_v2).unwrap();

        // Load project first, then user
        let mut registry = AgentRegistry::load_from_dir(dir1.path());
        registry.load_additional(dir2.path());

        // Should have only one agent — project version wins
        assert_eq!(registry.len(), 1);
        let agent = registry.get("shared-agent").unwrap();
        assert_eq!(agent.model_tier, "opus"); // V1, not V2
        assert_eq!(agent.description, "Version 1 (project)");
    }

    #[test]
    fn test_load_for_project() {
        let project = tempfile::tempdir().unwrap();
        let fin_agents = project.path().join(".fin").join("agents");
        std::fs::create_dir_all(&fin_agents).unwrap();

        let agent = r#"---
name: fin-builder
description: "Builder"
color: yellow
tools: Read, Write, Bash
model: sonnet
roles: builder
---

Build."#;

        std::fs::write(fin_agents.join("fin-builder.md"), agent).unwrap();

        let registry = AgentRegistry::load_for_project(project.path());
        // Should have at least the project agent (user agents may vary)
        assert!(registry.get("fin-builder").is_some());
        let builder = registry.get("fin-builder").unwrap();
        assert_eq!(builder.roles, vec!["builder"]);
    }

    #[test]
    fn test_load_workflow_agents_excludes_external() {
        let project = tempfile::tempdir().unwrap();

        // Create .fin/agents/ with a workflow agent
        let fin_agents = project.path().join(".fin").join("agents");
        std::fs::create_dir_all(&fin_agents).unwrap();

        let fin_agent = r#"---
name: fin-researcher
description: "Workflow researcher"
color: blue
tools: Read, Grep
model: sonnet
roles: researcher
---

Research."#;

        std::fs::write(fin_agents.join("fin-researcher.md"), fin_agent).unwrap();

        // load_workflow_agents only sees .fin/agents/
        let workflow = AgentRegistry::load_workflow_agents(project.path());
        assert_eq!(workflow.len(), 1);
        assert_eq!(
            workflow.get("fin-researcher").unwrap().description,
            "Workflow researcher"
        );

        // Any external agents (e.g. gsd-* in ~/.claude/agents/) are NOT loaded
        // We can't test the user's home dir, but we can verify only .fin/agents/ is read
        // by checking the count matches exactly what we put there
        assert_eq!(workflow.list().len(), 1);
    }

    #[test]
    fn test_find_workflow_role_rejects_non_fin_prefix() {
        let dir = tempfile::tempdir().unwrap();

        let fin_agent = r#"---
name: fin-researcher
description: "Legit workflow agent"
color: blue
tools: Read, Grep
model: sonnet
roles: researcher
---

Legit."#;

        let rogue_agent = r#"---
name: rogue-researcher
description: "Rogue agent trying to infiltrate workflow"
color: red
tools: Read, Write, Bash
model: opus
roles: researcher
---

Rogue."#;

        std::fs::write(dir.path().join("fin-researcher.md"), fin_agent).unwrap();
        std::fs::write(dir.path().join("rogue-researcher.md"), rogue_agent).unwrap();

        let registry = AgentRegistry::load_from_dir(dir.path());
        assert_eq!(registry.len(), 2);

        // find_by_role returns both
        let all = registry.find_by_role("researcher");
        assert_eq!(all.len(), 2);

        // find_workflow_role only returns fin-* prefixed
        let safe = registry.find_workflow_role("researcher");
        assert_eq!(safe.len(), 1);
        assert_eq!(safe[0].id, "fin-researcher");
    }
}
