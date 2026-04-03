// Fin + Context7 Extension (Library Documentation Lookup)

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::extensions::api::*;
use crate::llm::types::Content;
use crate::tools::{AgentTool, ToolResult};

const C7_API: &str = "https://api.context7.com/v1";

pub struct Context7Extension;

impl Extension for Context7Extension {
    fn manifest(&self) -> ExtensionManifest {
        ExtensionManifest {
            id: "context7".into(),
            name: "Context7".into(),
            version: "0.1.0".into(),
            description: "Look up library documentation via Context7 API".into(),
            tier: ExtensionTier::Bundled,
        }
    }

    fn tools(&self) -> Vec<Box<dyn AgentTool>> {
        vec![Box::new(ResolveLibraryTool), Box::new(GetLibraryDocsTool)]
    }
}

struct ResolveLibraryTool;

#[async_trait]
impl AgentTool for ResolveLibraryTool {
    fn name(&self) -> &str {
        "resolve_library"
    }

    fn description(&self) -> &str {
        "Search the Context7 library catalogue to find documentation for a library or framework."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Library name to search for (e.g., 'react', 'tokio', 'axum')"
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
        let query = params["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{C7_API}/libraries"))
            .query(&[("q", query)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(ToolResult {
                content: vec![Content::Text {
                    text: format!("Context7 error: {}", resp.status()),
                }],
                is_error: true,
            });
        }

        let body: serde_json::Value = resp.json().await?;
        let mut results = String::new();

        if let Some(libs) = body["libraries"].as_array() {
            for lib in libs.iter().take(5) {
                let id = lib["id"].as_str().unwrap_or("");
                let name = lib["name"].as_str().unwrap_or("");
                let desc = lib["description"].as_str().unwrap_or("");
                results.push_str(&format!("- {name} (id: {id})\n  {desc}\n\n"));
            }
        }

        Ok(ToolResult {
            content: vec![Content::Text {
                text: if results.is_empty() {
                    "No libraries found.".into()
                } else {
                    results
                },
            }],
            is_error: false,
        })
    }
}

struct GetLibraryDocsTool;

#[async_trait]
impl AgentTool for GetLibraryDocsTool {
    fn name(&self) -> &str {
        "get_library_docs"
    }

    fn description(&self) -> &str {
        "Get documentation for a specific library from Context7. Use resolve_library first to find the library ID."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["library_id", "topic"],
            "properties": {
                "library_id": {
                    "type": "string",
                    "description": "Library ID from resolve_library results"
                },
                "topic": {
                    "type": "string",
                    "description": "Specific topic or function to look up"
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
        let library_id = params["library_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'library_id'"))?;
        let topic = params["topic"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'topic'"))?;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{C7_API}/libraries/{library_id}/docs"))
            .query(&[("q", topic)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(ToolResult {
                content: vec![Content::Text {
                    text: format!("Context7 error: {}", resp.status()),
                }],
                is_error: true,
            });
        }

        let body: serde_json::Value = resp.json().await?;
        let text = body["content"]
            .as_str()
            .unwrap_or("No documentation found.")
            .to_string();

        Ok(ToolResult {
            content: vec![Content::Text { text }],
            is_error: false,
        })
    }
}
