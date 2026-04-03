// Fin + Read Tool (File Reading)

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct ReadTool;

#[async_trait]
impl AgentTool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read a file from the filesystem. Returns content with line numbers. Supports offset/limit for large files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default: 2000)"
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
        let file_path = params["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;

        let offset = params["offset"].as_u64().unwrap_or(1) as usize;
        let limit = params["limit"].as_u64().unwrap_or(2000) as usize;

        let content = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                return Ok(ToolResult {
                    content: vec![Content::Text {
                        text: format!("Error reading file: {e}"),
                    }],
                    is_error: true,
                });
            }
        };

        // Format with line numbers (cat -n style)
        let lines: Vec<String> = content
            .lines()
            .enumerate()
            .skip(offset.saturating_sub(1))
            .take(limit)
            .map(|(i, line)| format!("{}\t{}", i + 1, line))
            .collect();

        let text = lines.join("\n");

        Ok(ToolResult {
            content: vec![Content::Text { text }],
            is_error: false,
        })
    }
}
