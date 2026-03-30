// Fin — Glob Tool (File Pattern Matching)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct GlobTool {
    cwd: PathBuf,
}

impl GlobTool {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
        }
    }
}

#[async_trait]
impl AgentTool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns paths sorted by modification time."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: cwd)"
                }
            }
        })
    }

    async fn execute(
        &self,
        _id: &str,
        params: serde_json::Value,
        _cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'pattern' parameter"))?;

        let search_path = params["path"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.cwd.clone());

        // Use globset for matching
        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| anyhow::anyhow!("Invalid glob pattern: {e}"))?
            .compile_matcher();

        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        fn walk_dir(
            dir: &Path,
            base: &Path,
            glob: &globset::GlobMatcher,
            results: &mut Vec<(PathBuf, std::time::SystemTime)>,
        ) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };

            for entry in entries.flatten() {
                let path = entry.path();

                // Skip hidden dirs and common ignores
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') || name == "node_modules" || name == "target" {
                        continue;
                    }
                }

                if path.is_dir() {
                    walk_dir(&path, base, glob, results);
                } else {
                    let relative = path.strip_prefix(base).unwrap_or(&path);
                    if glob.is_match(relative) {
                        let mtime = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        results.push((path, mtime));
                    }
                }
            }
        }

        walk_dir(&search_path, &search_path, &glob, &mut matches);

        // Sort by modification time (newest first)
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        let text = if matches.is_empty() {
            "No files matched.".to_string()
        } else {
            matches
                .iter()
                .map(|(p, _)| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult {
            content: vec![Content::Text { text }],
            is_error: false,
        })
    }
}
