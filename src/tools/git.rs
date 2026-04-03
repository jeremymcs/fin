// Fin + Git Tool (Version Control Operations)

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct GitTool {
    cwd: PathBuf,
}

impl GitTool {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
        }
    }

    async fn run_git(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("git error: {stderr}"))
        }
    }
}

#[async_trait]
impl AgentTool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Execute git operations: status, diff, log, add, commit, branch, checkout."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["subcommand"],
            "properties": {
                "subcommand": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "add", "commit", "branch", "checkout", "push", "stash"],
                    "description": "Git subcommand to execute"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional arguments for the subcommand"
                },
                "message": {
                    "type": "string",
                    "description": "Commit message (for 'commit' subcommand)"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files to add (for 'add' subcommand)"
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
        let subcommand = params["subcommand"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'subcommand' parameter"))?;

        let result = match subcommand {
            "status" => self.run_git(&["status", "--short"]).await,

            "diff" => {
                let args = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut cmd_args = vec!["diff"];
                cmd_args.extend(args);
                self.run_git(&cmd_args).await
            }

            "log" => {
                let mut cmd_args = vec!["log", "--oneline", "-20"];
                let extra = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                cmd_args.extend(extra);
                self.run_git(&cmd_args).await
            }

            "add" => {
                let files = params["files"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                if files.is_empty() {
                    Err(anyhow::anyhow!("No files specified for 'add'"))
                } else {
                    let mut cmd_args = vec!["add"];
                    cmd_args.extend(files);
                    self.run_git(&cmd_args).await
                }
            }

            "commit" => {
                let message = params["message"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'message' for commit"))?;

                self.run_git(&["commit", "-m", message]).await
            }

            "branch" => {
                let args = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut cmd_args = vec!["branch"];
                cmd_args.extend(args);
                self.run_git(&cmd_args).await
            }

            "checkout" => {
                let args = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut cmd_args = vec!["checkout"];
                cmd_args.extend(args);
                self.run_git(&cmd_args).await
            }

            "push" => {
                // Push requires explicit confirmation in the agent loop
                let args = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut cmd_args = vec!["push"];
                cmd_args.extend(args);
                self.run_git(&cmd_args).await
            }

            "stash" => {
                let args = params["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut cmd_args = vec!["stash"];
                cmd_args.extend(args);
                self.run_git(&cmd_args).await
            }

            _ => Err(anyhow::anyhow!("Unknown git subcommand: {subcommand}")),
        };

        match result {
            Ok(output) => Ok(ToolResult {
                content: vec![Content::Text {
                    text: if output.is_empty() {
                        "(no output)".into()
                    } else {
                        output
                    },
                }],
                is_error: false,
            }),
            Err(e) => Ok(ToolResult {
                content: vec![Content::Text {
                    text: e.to_string(),
                }],
                is_error: true,
            }),
        }
    }
}
