// Fin + Built-in Tool Implementations

pub mod bash;
pub mod edit;
pub mod git;
pub mod glob;
pub mod grep;
pub mod read;
pub mod write;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::llm::types::{Content, ToolSchema};

/// Result of a tool execution.
pub struct ToolResult {
    pub content: Vec<Content>,
    pub is_error: bool,
}

/// Trait all tools implement.
#[async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;

    async fn execute(
        &self,
        id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult>;
}

/// Tool registry — holds all available tools.
pub struct ToolRegistry {
    tools: Vec<Box<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn AgentTool>) {
        self.tools.push(tool);
    }

    /// Register all built-in tools.
    pub fn with_defaults(cwd: &std::path::Path) -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(bash::BashTool::new(cwd)));
        registry.register(Box::new(read::ReadTool));
        registry.register(Box::new(write::WriteTool));
        registry.register(Box::new(edit::EditTool));
        registry.register(Box::new(grep::GrepTool::new(cwd)));
        registry.register(Box::new(glob::GlobTool::new(cwd)));
        registry.register(Box::new(git::GitTool::new(cwd)));
        registry
    }

    pub async fn execute(
        &self,
        name: &str,
        id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {name}"))?;

        tool.execute(id, params, cancel).await
    }

    /// Get JSON schemas for all tools (for LLM context).
    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.tools
            .iter()
            .map(|t| ToolSchema {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters_schema(),
            })
            .collect()
    }

    /// Build a registry with only the named built-in tools registered.
    /// `allowed` contains lowercase tool names like ["read", "grep", "glob"].
    pub fn filtered_defaults(cwd: &std::path::Path, allowed: &[String]) -> Self {
        let mut registry = Self::new();

        let all_tools: Vec<Box<dyn AgentTool>> = vec![
            Box::new(bash::BashTool::new(cwd)),
            Box::new(read::ReadTool),
            Box::new(write::WriteTool),
            Box::new(edit::EditTool),
            Box::new(grep::GrepTool::new(cwd)),
            Box::new(glob::GlobTool::new(cwd)),
            Box::new(git::GitTool::new(cwd)),
        ];

        for tool in all_tools {
            if allowed.iter().any(|a| a == tool.name()) {
                registry.register(tool);
            }
        }

        registry
    }
}
