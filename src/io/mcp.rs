// Fin — MCP Server (Model Context Protocol — JSON-RPC 2.0 over stdio)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::tools::ToolRegistry;

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: serde_json::Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

pub async fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let tool_registry = ToolRegistry::with_defaults(&cwd);

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    -32700,
                    &format!("Parse error: {e}"),
                );
                let json = serde_json::to_string(&resp)?;
                stdout.write_all(json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };

        let id = request.id.unwrap_or(serde_json::Value::Null);

        let response = match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "fin",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),

            "notifications/initialized" => {
                continue; // No response needed for notifications
            }

            "tools/list" => {
                let tools: Vec<serde_json::Value> = tool_registry
                    .schemas()
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.parameters
                        })
                    })
                    .collect();

                JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
            }

            "tools/call" => {
                let tool_name = request
                    .params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let arguments = request
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                let cancel = tokio_util::sync::CancellationToken::new();
                match tool_registry
                    .execute(tool_name, "mcp", arguments, cancel)
                    .await
                {
                    Ok(result) => {
                        let content: Vec<serde_json::Value> = result.content.iter().map(|c| {
                            match c {
                                crate::llm::types::Content::Text { text } => {
                                    serde_json::json!({"type": "text", "text": text})
                                }
                                _ => serde_json::json!({"type": "text", "text": "(non-text content)"})
                            }
                        }).collect();

                        JsonRpcResponse::success(
                            id,
                            serde_json::json!({
                                "content": content,
                                "isError": result.is_error
                            }),
                        )
                    }
                    Err(e) => JsonRpcResponse::success(
                        id,
                        serde_json::json!({
                            "content": [{"type": "text", "text": format!("Error: {e}")}],
                            "isError": true
                        }),
                    ),
                }
            }

            _ => {
                JsonRpcResponse::error(id, -32601, &format!("Method not found: {}", request.method))
            }
        };

        let json = serde_json::to_string(&response)?;
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}
