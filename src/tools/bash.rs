// Fin + Bash Tool (Shell Command Execution)

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct BashTool {
    cwd: PathBuf,
}

impl BashTool {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
        }
    }
}

#[async_trait]
impl AgentTool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command and return its output. Use for system commands, builds, tests, and terminal operations."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)"
                }
            }
        })
    }

    async fn execute(
        &self,
        _id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        let command = params["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

        let timeout_ms = params["timeout"].as_u64().unwrap_or(120_000);

        let output = tokio::select! {
            result = tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&self.cwd)
                .output() => {
                result?
            }
            _ = cancel.cancelled() => {
                return Ok(ToolResult {
                    content: vec![Content::Text { text: "Command cancelled".into() }],
                    is_error: true,
                });
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)) => {
                return Ok(ToolResult {
                    content: vec![Content::Text { text: format!("Command timed out after {timeout_ms}ms") }],
                    is_error: true,
                });
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut text = String::new();
        if !stdout.is_empty() {
            text.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("stderr:\n");
            text.push_str(&stderr);
        }

        if text.is_empty() {
            text = format!("(exit code: {})", output.status.code().unwrap_or(-1));
        }

        Ok(ToolResult {
            content: vec![Content::Text { text }],
            is_error: !output.status.success(),
        })
    }
}
