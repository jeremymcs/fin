// Fin — Workflow System
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

#![allow(dead_code)]

pub mod auto_loop;
pub mod commands;
pub mod continue_protocol;
pub mod crud;
pub mod dispatch;
pub mod git;
pub mod markdown;
pub mod phases;
pub mod prompts;
pub mod state;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Define,
    Explore,
    Architect,
    Build,
    Validate,
    SealSection,
    Advance,
}

const ALL_STAGES: [Stage; 7] = [
    Stage::Define,
    Stage::Explore,
    Stage::Architect,
    Stage::Build,
    Stage::Validate,
    Stage::SealSection,
    Stage::Advance,
];

impl Stage {
    /// Returns the next stage in the ordered sequence, or None if at the end.
    pub fn next(self) -> Option<Stage> {
        match self {
            Stage::Define => Some(Stage::Explore),
            Stage::Explore => Some(Stage::Architect),
            Stage::Architect => Some(Stage::Build),
            Stage::Build => Some(Stage::Validate),
            Stage::Validate => Some(Stage::SealSection),
            Stage::SealSection => Some(Stage::Advance),
            Stage::Advance => None,
        }
    }

    /// Returns the lowercase string label for this stage.
    pub fn label(&self) -> &str {
        match self {
            Stage::Define => "define",
            Stage::Explore => "explore",
            Stage::Architect => "architect",
            Stage::Build => "build",
            Stage::Validate => "validate",
            Stage::SealSection => "seal-section",
            Stage::Advance => "advance",
        }
    }

    /// Parses a stage from a case-insensitive string.
    pub fn from_str(s: &str) -> Option<Stage> {
        match s.to_lowercase().as_str() {
            "define" => Some(Stage::Define),
            "explore" => Some(Stage::Explore),
            "architect" => Some(Stage::Architect),
            "build" => Some(Stage::Build),
            "validate" => Some(Stage::Validate),
            "seal-section" | "seal_section" => Some(Stage::SealSection),
            "advance" => Some(Stage::Advance),
            _ => None,
        }
    }

    /// Returns all 7 stages in order.
    pub fn all() -> &'static [Stage] {
        &ALL_STAGES
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPosition {
    pub blueprint_id: String,
    pub section_id: Option<String>,
    pub task_id: Option<String>,
    pub stage: Stage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_sequence() {
        assert_eq!(Stage::Define.next(), Some(Stage::Explore));
        assert_eq!(Stage::Explore.next(), Some(Stage::Architect));
        assert_eq!(Stage::Architect.next(), Some(Stage::Build));
        assert_eq!(Stage::Build.next(), Some(Stage::Validate));
        assert_eq!(Stage::Validate.next(), Some(Stage::SealSection));
        assert_eq!(Stage::SealSection.next(), Some(Stage::Advance));
        assert_eq!(Stage::Advance.next(), None);
    }

    #[test]
    fn stage_from_str_case_insensitive() {
        assert_eq!(Stage::from_str("DEFINE"), Some(Stage::Define));
        assert_eq!(Stage::from_str("Architect"), Some(Stage::Architect));
        assert_eq!(Stage::from_str("build"), Some(Stage::Build));
        assert_eq!(Stage::from_str("unknown"), None);
    }

    #[test]
    fn stage_all_returns_seven() {
        assert_eq!(Stage::all().len(), 7);
        assert_eq!(Stage::all()[0], Stage::Define);
        assert_eq!(Stage::all()[6], Stage::Advance);
    }

    #[test]
    fn stage_display() {
        assert_eq!(format!("{}", Stage::Build), "build");
        assert_eq!(format!("{}", Stage::SealSection), "seal-section");
    }

    #[test]
    fn stage_serde_roundtrip() {
        let stage = Stage::Build;
        let json = serde_json::to_string(&stage).unwrap();
        assert_eq!(json, "\"build\"");
        let parsed: Stage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, stage);
    }

    #[test]
    fn workflow_position_serde() {
        let pos = WorkflowPosition {
            blueprint_id: "B001".to_string(),
            section_id: Some("S01".to_string()),
            task_id: None,
            stage: Stage::Architect,
        };
        let json = serde_json::to_string(&pos).unwrap();
        let parsed: WorkflowPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.blueprint_id, "B001");
        assert_eq!(parsed.section_id, Some("S01".to_string()));
        assert_eq!(parsed.task_id, None);
        assert_eq!(parsed.stage, Stage::Architect);
    }
}
