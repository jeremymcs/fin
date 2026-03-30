// Fin — Grep Tool (Native ripgrep engine — no subprocess)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct GrepTool {
    cwd: PathBuf,
}

impl GrepTool {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
        }
    }
}

#[async_trait]
impl AgentTool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. Powered by ripgrep for fast, recursive search."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (default: cwd)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., '*.rs')"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default: false)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode (default: files_with_matches)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines before and after match"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 250)"
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

        let max_results = params["max_results"].as_u64().unwrap_or(250) as usize;
        let case_insensitive = params["case_insensitive"].as_bool().unwrap_or(false);
        let output_mode = params["output_mode"]
            .as_str()
            .unwrap_or("files_with_matches");
        let context_lines = params["context_lines"].as_u64().unwrap_or(0) as usize;
        let glob_pattern = params["glob"].as_str().map(String::from);

        // Build regex
        let regex_str = if case_insensitive {
            format!("(?i){pattern}")
        } else {
            pattern.to_string()
        };
        let matcher = RegexMatcher::new_line_matcher(&regex_str)?;

        // Walk files (respects .gitignore)
        let mut walker_builder = ignore::WalkBuilder::new(&search_path);
        walker_builder
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true);

        if let Some(ref glob) = glob_pattern {
            let mut overrides = ignore::overrides::OverrideBuilder::new(&search_path);
            overrides.add(glob)?;
            walker_builder.overrides(overrides.build()?);
        }

        // Collect results
        let mut content_lines: Vec<String> = Vec::new();
        let mut matched_files: Vec<PathBuf> = Vec::new();
        let mut file_counts: Vec<(PathBuf, usize)> = Vec::new();
        let mut total_hits: usize = 0;

        for entry in walker_builder.build().flatten() {
            if total_hits >= max_results && output_mode == "content" {
                break;
            }
            if matched_files.len() >= max_results && output_mode != "content" {
                break;
            }

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Build searcher with context if needed
            let mut searcher = if context_lines > 0 {
                let mut builder = grep_searcher::SearcherBuilder::new();
                builder.before_context(context_lines);
                builder.after_context(context_lines);
                builder.build()
            } else {
                Searcher::new()
            };

            let file_path = path.to_path_buf();
            let mut file_hit_count: usize = 0;

            if let Err(e) = searcher.search_path(
                &matcher,
                path,
                UTF8(|line_num, line| {
                    file_hit_count += 1;
                    total_hits += 1;

                    if output_mode == "content" && total_hits <= max_results {
                        content_lines.push(format!(
                            "{}:{}:{}",
                            file_path.display(),
                            line_num,
                            line.trim_end()
                        ));
                    }

                    Ok(total_hits < max_results || output_mode != "content")
                }),
            ) {
                tracing::debug!("Grep search error on {}: {e}", path.display());
            }

            if file_hit_count > 0 {
                matched_files.push(path.to_path_buf());
                file_counts.push((path.to_path_buf(), file_hit_count));
            }
        }

        // Format output
        let text = match output_mode {
            "content" => {
                if content_lines.is_empty() {
                    "No matches found.".to_string()
                } else {
                    content_lines.join("\n")
                }
            }
            "count" => {
                if file_counts.is_empty() {
                    "No matches found.".to_string()
                } else {
                    file_counts.sort_by(|a, b| a.0.cmp(&b.0));
                    file_counts
                        .iter()
                        .map(|(p, c)| format!("{}:{c}", p.display()))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            _ => {
                if matched_files.is_empty() {
                    "No matches found.".to_string()
                } else {
                    matched_files.sort();
                    matched_files
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
        };

        Ok(ToolResult {
            content: vec![Content::Text { text }],
            is_error: false,
        })
    }
}
