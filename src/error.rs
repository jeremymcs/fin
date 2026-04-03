// Fin + Core Error Types

#[allow(dead_code)]
#[derive(thiserror::Error, Debug)]
pub enum FinError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Extension error: {0}")]
    Extension(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Cancelled")]
    Cancelled,
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("Provider {provider} returned {status}: {body}")]
    ApiError {
        provider: String,
        status: u16,
        body: String,
    },

    #[allow(dead_code)]
    #[error("Stream error: {0}")]
    Stream(String),

    #[allow(dead_code)]
    #[error("No API key configured for provider: {0}")]
    NoApiKey(String),

    #[allow(dead_code)]
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[allow(dead_code)]
    #[error("Rate limited by {provider}, retry after {retry_after_secs}s")]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },

    #[allow(dead_code)]
    #[error("Context window exceeded: {used} tokens used, {max} max")]
    ContextOverflow { used: u64, max: u64 },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

#[allow(dead_code)]
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid parameters for tool {tool}: {reason}")]
    InvalidParams { tool: String, reason: String },

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool timed out after {0}s")]
    Timeout(u64),

    #[error("Tool blocked: {0}")]
    Blocked(String),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, FinError>;
