// Fin — Print Mode (Single-Shot Agent Execution)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use std::sync::Arc;

use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::{AgentRegistry, DelegateTool};
use crate::config::auth::AuthStore;
use crate::io::print_io::PrintIO;
use crate::llm::models::{default_models, resolve_model};
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::*;
use crate::tools::ToolRegistry;

/// Run a prompt through the full agent loop with tools.
pub async fn run(prompt: &str, model_id: Option<&str>) -> anyhow::Result<()> {
    if prompt.is_empty() {
        anyhow::bail!("No prompt provided. Use: fin -p \"your prompt\"");
    }

    // Resolve model
    let model = match model_id {
        Some(id) => resolve_model(id).ok_or_else(|| anyhow::anyhow!("Model not found: {id}"))?,
        None => pick_default_model()?,
    };

    eprintln!(
        "\x1b[2m[{} via {}]\x1b[0m",
        model.display_name, model.provider
    );

    // Verify auth
    let auth = AuthStore::default();
    if auth.get_api_key(&model.provider).is_none() {
        let env_hint = match model.provider.as_str() {
            "anthropic" => "ANTHROPIC_API_KEY",
            "openai" => "OPENAI_ACCESS_TOKEN, OPENAI_BEARER_TOKEN, or OPENAI_API_KEY",
            "google" => "GOOGLE_API_KEY or GEMINI_API_KEY",
            _ => "API key",
        };
        anyhow::bail!(
            "No API key for provider '{}'. Set {} environment variable.",
            model.provider,
            env_hint
        );
    }

    // Working directory
    let cwd = std::env::current_dir()?;

    // Build provider registry (shared with delegate tool)
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));

    // Load agent registry
    let agent_registry = Arc::new(AgentRegistry::load_default());

    // Build tools (with extensions and delegate tool)
    let mut tool_registry = ToolRegistry::with_defaults(&cwd);

    // Register extension tools (web_search, context7) and fire lifecycle
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }
    ext_registry.on_session_start(&crate::extensions::api::ExtensionContext {
        cwd: cwd.clone(),
        session_id: String::new(),
    });

    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.clone(),
            0,
        )));
    }

    // Build system prompt with agent context
    let agent_context = if !agent_registry.is_empty() {
        Some(AgentPromptContext {
            available_agents: Some(agent_registry.prompt_summary()),
            agent_role: None,
        })
    } else {
        None
    };
    let system_prompt = build_system_prompt(&tool_registry.schemas(), &cwd, agent_context.as_ref());

    // Build agent state
    let mut state = AgentState::new(model.clone(), cwd);
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.messages.push(Message::new_user(prompt));

    // Create IO adapter
    let io = PrintIO::new(true, true);

    // Get provider
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    // Run agent loop
    let cancel = tokio_util::sync::CancellationToken::new();
    run_agent_loop(&mut state, provider, &io, cancel).await?;

    // Persist session
    if let Ok(paths) = crate::config::paths::FinPaths::resolve() {
        if let Ok(store) = crate::db::session::SessionStore::new(&paths.sessions_dir) {
            for msg in &state.messages {
                if let Err(e) = store.append(&state.session_id, msg) {
                    tracing::warn!("Session persist failed: {e}");
                }
            }
        }
    }

    Ok(())
}

/// Pick the best default model based on available API keys.
/// Public so headless and HTTP modes can reuse it.
pub fn pick_model(model_id: Option<&str>) -> anyhow::Result<crate::llm::models::ModelConfig> {
    if let Some(id) = model_id {
        return crate::llm::models::resolve_model(id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {id}"));
    }
    pick_default_model()
}

/// Resume a session with a specific prompt (used by `fin -c "prompt"`).
pub async fn run_with_prompt_and_session(
    session_id: &str,
    messages: Vec<Message>,
    prompt: &str,
    model_id: Option<&str>,
) -> anyhow::Result<()> {
    let model = pick_model(model_id)?;
    eprintln!(
        "\x1b[2m[{} via {} — resuming session]\x1b[0m",
        model.display_name, model.provider
    );

    let cwd = std::env::current_dir()?;
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let agent_registry = Arc::new(AgentRegistry::load_default());

    let mut tool_registry = ToolRegistry::with_defaults(&cwd);
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }
    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.clone(),
            0,
        )));
    }

    let agent_context = if !agent_registry.is_empty() {
        Some(AgentPromptContext {
            available_agents: Some(agent_registry.prompt_summary()),
            agent_role: None,
        })
    } else {
        None
    };
    let system_prompt = build_system_prompt(&tool_registry.schemas(), &cwd, agent_context.as_ref());

    let mut state = AgentState::new(model.clone(), cwd);
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.session_id = session_id.to_string();
    state.messages = messages;
    state.messages.push(Message::new_user(prompt));

    let io = PrintIO::new(true, true);
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let cancel = tokio_util::sync::CancellationToken::new();
    run_agent_loop(&mut state, provider, &io, cancel).await?;

    // Persist session
    if let Ok(paths) = crate::config::paths::FinPaths::resolve() {
        if let Ok(store) = crate::db::session::SessionStore::new(&paths.sessions_dir) {
            for msg in &state.messages {
                if let Err(e) = store.append(&state.session_id, msg) {
                    tracing::warn!("Session persist failed: {e}");
                }
            }
        }
    }

    Ok(())
}

/// Resume a session by loading existing messages and waiting for the next prompt via stdin.
pub async fn run_with_session(session_id: &str, messages: Vec<Message>) -> anyhow::Result<()> {
    let model = pick_default_model()?;
    eprintln!(
        "\x1b[2m[{} via {}]\x1b[0m",
        model.display_name, model.provider
    );

    let cwd = std::env::current_dir()?;
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let agent_registry = Arc::new(AgentRegistry::load_default());

    let mut tool_registry = ToolRegistry::with_defaults(&cwd);
    let ext_registry = crate::extensions::ExtensionRegistry::with_defaults();
    for tool in ext_registry.tools() {
        tool_registry.register(tool);
    }
    if !agent_registry.is_empty() {
        tool_registry.register(Box::new(DelegateTool::new(
            Arc::clone(&agent_registry),
            Arc::clone(&provider_registry),
            cwd.clone(),
            0,
        )));
    }

    let agent_context = if !agent_registry.is_empty() {
        Some(AgentPromptContext {
            available_agents: Some(agent_registry.prompt_summary()),
            agent_role: None,
        })
    } else {
        None
    };
    let system_prompt = build_system_prompt(&tool_registry.schemas(), &cwd, agent_context.as_ref());

    let mut state = AgentState::new(model.clone(), cwd);
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.session_id = session_id.to_string();
    state.messages = messages;

    // Show conversation summary
    let user_msgs: Vec<&str> = state
        .messages
        .iter()
        .filter(|m| m.role == Role::User)
        .filter_map(|m| m.content.first())
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    if let Some(last) = user_msgs.last() {
        let preview = if last.len() > 80 { &last[..80] } else { last };
        eprintln!("\x1b[2mLast prompt: {preview}\x1b[0m");
    }

    // Read next prompt from stdin
    eprintln!("Enter your next prompt:");
    let mut prompt = String::new();
    std::io::stdin().read_line(&mut prompt)?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("No prompt provided.");
    }
    state.messages.push(Message::new_user(prompt));

    let io = PrintIO::new(true, true);
    let provider = provider_registry
        .get(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", model.provider))?;

    let cancel = tokio_util::sync::CancellationToken::new();
    run_agent_loop(&mut state, provider, &io, cancel).await?;

    // Persist session
    if let Ok(paths) = crate::config::paths::FinPaths::resolve() {
        if let Ok(store) = crate::db::session::SessionStore::new(&paths.sessions_dir) {
            for msg in &state.messages {
                if let Err(e) = store.append(&state.session_id, msg) {
                    tracing::warn!("Session persist failed: {e}");
                }
            }
        }
    }

    Ok(())
}

pub fn pick_default_model() -> anyhow::Result<crate::llm::models::ModelConfig> {
    let models = default_models();
    let auth = AuthStore::default(); // Loads from auth.json + env vars

    // Check preferences for a configured default model
    let cwd = std::env::current_dir().unwrap_or_default();
    let prefs = crate::config::preferences::Preferences::resolve(&cwd);
    if let Some(ref model_id) = prefs.default_model {
        if let Some(m) = resolve_model(model_id) {
            // Verify the provider has auth configured
            if auth.get_api_key(&m.provider).is_some() {
                return Ok(m);
            }
            tracing::warn!(
                "Preferred model '{model_id}' configured but no API key for provider '{}'",
                m.provider
            );
        }
    }

    // Check each provider — AuthStore.get_api_key checks env vars AND stored keys
    if auth.get_api_key("anthropic").is_some() {
        if let Some(m) = models.iter().find(|m| m.id == "claude-sonnet-4-6") {
            return Ok(m.clone());
        }
    }

    if auth.get_api_key("openai").is_some() {
        if let Some(m) = models.iter().find(|m| m.id == "gpt-4.1") {
            return Ok(m.clone());
        }
    }

    if auth.get_api_key("google").is_some() || std::env::var("GEMINI_API_KEY").is_ok() {
        if let Some(m) = models.iter().find(|m| m.id == "gemini-2.5-flash") {
            return Ok(m.clone());
        }
    }

    // Cloud providers (Vertex, Bedrock)
    if std::env::var("GOOGLE_CLOUD_PROJECT").is_ok()
        || std::env::var("CLOUDSDK_CORE_PROJECT").is_ok()
    {
        if let Some(m) = models.iter().find(|m| m.id == "claude-sonnet-4@20250514") {
            return Ok(m.clone());
        }
    }

    if std::env::var("AWS_ACCESS_KEY_ID").is_ok() || std::env::var("AWS_PROFILE").is_ok() {
        if let Some(m) = models
            .iter()
            .find(|m| m.id == "anthropic.claude-sonnet-4-20250514-v1:0")
        {
            return Ok(m.clone());
        }
    }

    anyhow::bail!(
        "No API key found. Set one of:\n  \
         ANTHROPIC_API_KEY, OPENAI_ACCESS_TOKEN (or OPENAI_API_KEY), GOOGLE_API_KEY\n  \
         GOOGLE_CLOUD_PROJECT (Vertex AI), AWS_ACCESS_KEY_ID (Bedrock)\n  \
         Or run `fin config` to store a key."
    )
}
