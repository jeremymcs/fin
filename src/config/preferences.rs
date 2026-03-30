// Fin — Preferences
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preferences {
    /// Workflow mode
    #[serde(default)]
    pub mode: WorkflowMode,

    /// Default LLM model
    #[serde(default)]
    pub default_model: Option<String>,

    /// Git preferences
    #[serde(default)]
    pub git: GitPreferences,

    /// Model overrides per stage
    #[serde(default)]
    pub models: ModelPreferences,

    /// Workflow automation preferences
    #[serde(default)]
    pub workflow: WorkflowPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    #[default]
    Solo,
    Team,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitPreferences {
    #[serde(default)]
    pub auto_push: bool,
    #[serde(default)]
    pub main_branch: Option<String>,
    #[serde(default)]
    pub merge_strategy: Option<String>,
    #[serde(default)]
    pub isolation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelPreferences {
    pub explore: Option<String>,
    pub architect: Option<String>,
    pub build: Option<String>,
    pub validate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPreferences {
    /// Auto-advance to next stage after completion
    #[serde(default)]
    pub auto_advance: bool,
    /// Auto-commit .fin/ artifacts after stage completion
    #[serde(default = "default_true")]
    pub auto_commit_artifacts: bool,
    /// Auto-create section branch on build stage entry
    #[serde(default = "default_true")]
    pub auto_branch: bool,
    /// Auto-squash merge on section completion
    #[serde(default = "default_true")]
    pub auto_squash: bool,
}

impl Default for WorkflowPreferences {
    fn default() -> Self {
        Self {
            auto_advance: false,
            auto_commit_artifacts: true,
            auto_branch: true,
            auto_squash: true,
        }
    }
}

fn default_true() -> bool {
    true
}

impl Preferences {
    /// Load preferences from TOML file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let prefs: Self = toml::from_str(&content)?;
        Ok(prefs)
    }

    /// Save preferences to TOML file.
    #[allow(dead_code)] // Used by `fin config` command
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Merge project-level preferences over global ones.
    pub fn merge(&mut self, project: &Preferences) {
        if project.default_model.is_some() {
            self.default_model.clone_from(&project.default_model);
        }
        if project.git.auto_push {
            self.git.auto_push = true;
        }
        if project.git.main_branch.is_some() {
            self.git.main_branch.clone_from(&project.git.main_branch);
        }
    }

    /// Load resolved preferences: global (~/.config/fin/preferences.toml)
    /// merged with project-level (.fin/preferences.toml) if present.
    pub fn resolve(cwd: &Path) -> Self {
        let global = crate::config::paths::FinPaths::resolve()
            .ok()
            .and_then(|p| Self::load(&p.preferences_file).ok())
            .unwrap_or_default();

        let project_file = cwd.join(".fin").join("preferences.toml");
        let mut prefs = global;
        if let Ok(project) = Self::load(&project_file) {
            prefs.merge(&project);
        }
        prefs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workflow_mode_is_solo() {
        let prefs = Preferences::default();
        assert!(matches!(prefs.mode, WorkflowMode::Solo));
    }

    #[test]
    fn default_workflow_prefs() {
        let wf = WorkflowPreferences::default();
        assert!(!wf.auto_advance);
        assert!(wf.auto_commit_artifacts);
        assert!(wf.auto_branch);
        assert!(wf.auto_squash);
    }

    #[test]
    fn default_git_prefs() {
        let git = GitPreferences::default();
        assert!(!git.auto_push);
        assert!(git.main_branch.is_none());
        assert!(git.merge_strategy.is_none());
    }

    #[test]
    fn toml_roundtrip() {
        let mut prefs = Preferences::default();
        prefs.default_model = Some("claude-opus-4-6".into());
        prefs.git.main_branch = Some("develop".into());
        prefs.workflow.auto_advance = true;

        let toml_str = toml::to_string_pretty(&prefs).unwrap();
        let parsed: Preferences = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.default_model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(parsed.git.main_branch.as_deref(), Some("develop"));
        assert!(parsed.workflow.auto_advance);
    }

    #[test]
    fn toml_empty_string_parses_to_defaults() {
        let parsed: Preferences = toml::from_str("").unwrap();
        assert!(parsed.default_model.is_none());
        assert!(matches!(parsed.mode, WorkflowMode::Solo));
    }

    #[test]
    fn merge_overrides_model() {
        let mut global = Preferences::default();
        global.default_model = Some("old-model".into());

        let mut project = Preferences::default();
        project.default_model = Some("project-model".into());

        global.merge(&project);
        assert_eq!(global.default_model.as_deref(), Some("project-model"));
    }

    #[test]
    fn merge_overrides_git_branch() {
        let mut global = Preferences::default();
        let mut project = Preferences::default();
        project.git.main_branch = Some("develop".into());

        global.merge(&project);
        assert_eq!(global.git.main_branch.as_deref(), Some("develop"));
    }

    #[test]
    fn merge_preserves_global_when_project_empty() {
        let mut global = Preferences::default();
        global.default_model = Some("global-model".into());
        global.git.main_branch = Some("main".into());

        let project = Preferences::default();
        global.merge(&project);

        assert_eq!(global.default_model.as_deref(), Some("global-model"));
        assert_eq!(global.git.main_branch.as_deref(), Some("main"));
    }

    #[test]
    fn merge_auto_push_only_sets_true() {
        let mut global = Preferences::default();
        assert!(!global.git.auto_push);

        let mut project = Preferences::default();
        project.git.auto_push = true;
        global.merge(&project);
        assert!(global.git.auto_push);

        // Merging with false doesn't revert
        let project2 = Preferences::default();
        global.merge(&project2);
        assert!(global.git.auto_push);
    }
}
