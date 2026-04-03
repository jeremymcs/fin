// Fin + Web Search Extension

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::extensions::api::*;
use crate::llm::types::Content;
use crate::tools::{AgentTool, ToolResult};

pub struct WebSearchExtension;

impl Extension for WebSearchExtension {
    fn manifest(&self) -> ExtensionManifest {
        ExtensionManifest {
            id: "web-search".into(),
            name: "Web Search".into(),
            version: "0.1.0".into(),
            description: "Search the web using Brave Search or Tavily API".into(),
            tier: ExtensionTier::Bundled,
        }
    }

    fn tools(&self) -> Vec<Box<dyn AgentTool>> {
        vec![Box::new(WebSearchTool)]
    }
}

struct WebSearchTool;

#[async_trait]
impl AgentTool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for current information. Requires BRAVE_API_KEY or TAVILY_API_KEY."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 5)"
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
        let max_results = params["max_results"].as_u64().unwrap_or(5);

        // Try Brave Search first, then Tavily
        if let Ok(key) = std::env::var("BRAVE_API_KEY") {
            return brave_search(query, max_results, &key).await;
        }

        if let Ok(key) = std::env::var("TAVILY_API_KEY") {
            return tavily_search(query, max_results, &key).await;
        }

        Ok(ToolResult {
            content: vec![Content::Text {
                text: "No search API key configured. Set BRAVE_API_KEY or TAVILY_API_KEY.".into(),
            }],
            is_error: true,
        })
    }
}

async fn brave_search(query: &str, max_results: u64, api_key: &str) -> anyhow::Result<ToolResult> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &max_results.to_string())])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(ToolResult {
            content: vec![Content::Text {
                text: format!("Brave Search error: {}", resp.status()),
            }],
            is_error: true,
        });
    }

    let body: serde_json::Value = resp.json().await?;
    let mut results = String::new();

    if let Some(web) = body["web"]["results"].as_array() {
        for (i, result) in web.iter().enumerate() {
            let title = result["title"].as_str().unwrap_or("");
            let url = result["url"].as_str().unwrap_or("");
            let desc = result["description"].as_str().unwrap_or("");
            results.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                title,
                url,
                desc
            ));
        }
    }

    Ok(ToolResult {
        content: vec![Content::Text {
            text: if results.is_empty() {
                "No results found.".into()
            } else {
                results
            },
        }],
        is_error: false,
    })
}

async fn tavily_search(query: &str, max_results: u64, api_key: &str) -> anyhow::Result<ToolResult> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.tavily.com/search")
        .json(&json!({
            "api_key": api_key,
            "query": query,
            "max_results": max_results,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(ToolResult {
            content: vec![Content::Text {
                text: format!("Tavily error: {}", resp.status()),
            }],
            is_error: true,
        });
    }

    let body: serde_json::Value = resp.json().await?;
    let mut results = String::new();

    if let Some(items) = body["results"].as_array() {
        for (i, result) in items.iter().enumerate() {
            let title = result["title"].as_str().unwrap_or("");
            let url = result["url"].as_str().unwrap_or("");
            let content = result["content"].as_str().unwrap_or("");
            results.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                title,
                url,
                content
            ));
        }
    }

    Ok(ToolResult {
        content: vec![Content::Text {
            text: if results.is_empty() {
                "No results found.".into()
            } else {
                results
            },
        }],
        is_error: false,
    })
}
