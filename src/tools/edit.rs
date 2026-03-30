// Fin — Edit Tool (String Replacement)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolResult};
use crate::llm::types::Content;

pub struct EditTool;

#[async_trait]
impl AgentTool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing an exact string match. The old_string must be unique in the file unless replace_all is true."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["file_path", "old_string", "new_string"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
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

        let old_string = params["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'old_string' parameter"))?;

        let new_string = params["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'new_string' parameter"))?;

        let replace_all = params["replace_all"].as_bool().unwrap_or(false);

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    content: vec![Content::Text {
                        text: format!("Error reading file: {e}"),
                    }],
                    is_error: true,
                });
            }
        };

        if !replace_all {
            // Check uniqueness
            let count = content.matches(old_string).count();
            if count == 0 {
                return Ok(ToolResult {
                    content: vec![Content::Text {
                        text: "old_string not found in file".into(),
                    }],
                    is_error: true,
                });
            }
            if count > 1 {
                return Ok(ToolResult {
                    content: vec![Content::Text {
                        text: format!(
                            "old_string is not unique — found {count} occurrences. Use replace_all or provide more context."
                        ),
                    }],
                    is_error: true,
                });
            }
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        std::fs::write(file_path, &new_content)?;

        Ok(ToolResult {
            content: vec![Content::Text {
                text: format!("Edited {file_path}"),
            }],
            is_error: false,
        })
    }
}
