// Fin — Onboarding Wizard
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::config::auth::AuthStore;
use crate::config::paths::FinPaths;
use std::io::{self, Write};

/// Run the first-time setup wizard.
pub async fn run_wizard() -> anyhow::Result<()> {
    println!();
    println!("  ╭─────────────────────────────╮");
    println!("  │     fin — Setup Wizard       │");
    println!("  │  AI Coding Agent v{}     │", env!("CARGO_PKG_VERSION"));
    println!("  ╰─────────────────────────────╯");
    println!();

    let paths = FinPaths::resolve()?;
    let mut auth = AuthStore::load(&paths.auth_file).unwrap_or_default();

    // Step 1: LLM Provider
    println!("Step 1: Configure an LLM provider\n");
    println!("  1. Anthropic (Claude)");
    println!("  2. OpenAI (GPT-4)");
    println!("  3. Google (Gemini)");
    println!("  4. Ollama (Local models — qwen, llama, deepseek, etc.)");
    println!("  5. Skip for now");
    println!();

    let choice = prompt_input("Choose provider [1-5]")?;

    match choice.trim() {
        "1" => {
            println!("\nGet your API key at: https://console.anthropic.com/settings/keys\n");
            let key = prompt_secret("Anthropic API key")?;
            if !key.is_empty() {
                auth.set_api_key("anthropic", key);
                println!("  ✓ Anthropic configured");
            }
        }
        "2" => {
            println!("\nGet your API key at: https://platform.openai.com/api-keys\n");
            let key = prompt_secret("OpenAI API key")?;
            if !key.is_empty() {
                auth.set_api_key("openai", key);
                println!("  ✓ OpenAI configured");
            }
        }
        "3" => {
            println!("\nGet your API key at: https://aistudio.google.com/apikey\n");
            let key = prompt_secret("Google API key")?;
            if !key.is_empty() {
                auth.set_api_key("google", key);
                println!("  ✓ Google configured");
            }
        }
        "4" => {
            configure_ollama(&mut auth).await?;
        }
        _ => {
            println!("\n  Skipped. Set an API key later via environment variable:");
            println!("    export ANTHROPIC_API_KEY=sk-ant-...");
            println!("    export OPENAI_API_KEY=sk-...");
            println!("    export GOOGLE_API_KEY=...");
            println!("  Or start Ollama locally: ollama serve");
        }
    }

    // Step 2: Optional search
    println!("\nStep 2: Web search (optional)\n");
    println!("  1. Brave Search (https://brave.com/search/api/)");
    println!("  2. Tavily (https://tavily.com/)");
    println!("  3. Skip");
    println!();

    let choice = prompt_input("Choose search provider [1-3]")?;
    match choice.trim() {
        "1" => {
            let key = prompt_secret("Brave API key")?;
            if !key.is_empty() {
                auth.set_api_key("brave", key);
                println!("  ✓ Brave Search configured");
            }
        }
        "2" => {
            let key = prompt_secret("Tavily API key")?;
            if !key.is_empty() {
                auth.set_api_key("tavily", key);
                println!("  ✓ Tavily configured");
            }
        }
        _ => {}
    }

    // Save
    auth.save(&paths.auth_file)?;

    println!("\n  ✓ Configuration saved to {}", paths.auth_file.display());
    println!("\n  Run `fin` to start the interactive agent.");
    println!("  Run `fin -p \"your prompt\"` for single-shot mode.");
    println!();

    Ok(())
}

/// Interactive Ollama setup — detect, list models, optional custom host.
async fn configure_ollama(auth: &mut AuthStore) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let available = crate::llm::ollama::OllamaProvider::is_available(&client).await;

    if available {
        println!("\n  ✓ Ollama detected at {}", std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "localhost:11434".into()));

        let models = crate::llm::ollama::OllamaProvider::discover_models(&client).await;
        if models.is_empty() {
            println!("  No models found. Pull one with: ollama pull qwen3:8b");
        } else {
            println!("  Available models:");
            for m in &models {
                let size = if m.details.parameter_size.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", m.details.parameter_size)
                };
                println!("    - {}{size}", m.name);
            }
            println!("\n  Use with: fin --model ollama/{}", models[0].name);
        }
    } else {
        println!("\n  Ollama not detected at localhost:11434.");
        println!("  Install: https://ollama.com");
        println!("  Start:   ollama serve");
    }

    println!("\n  Custom endpoint? (leave blank for default localhost:11434)");
    let host = prompt_input("Ollama host")?;
    if !host.is_empty() {
        // Store custom host as a preference (not as an API key)
        // User should set OLLAMA_HOST env var for custom endpoints
        println!("  Set OLLAMA_HOST={host} in your shell profile.");
    }

    // Optional API key for remote OpenAI-compatible endpoints
    println!("\n  API key? (leave blank if none needed — most local setups don't)");
    let key = prompt_secret("Ollama API key (optional)")?;
    if !key.is_empty() {
        auth.set_api_key("ollama", key);
        println!("  ✓ Ollama API key saved");
    } else {
        println!("  ✓ Ollama configured (no API key needed for local)");
    }

    Ok(())
}

/// Set an API key for a specific provider (hidden input).
pub async fn cmd_set_key(provider: &str) -> anyhow::Result<()> {
    let known = ["anthropic", "openai", "google", "mistral", "brave", "tavily", "ollama"];
    if !known.contains(&provider) {
        println!(
            "  Unknown provider: {provider}\n  Supported: {}",
            known.join(", ")
        );
        return Ok(());
    }

    let paths = FinPaths::resolve()?;
    let mut auth = AuthStore::load(&paths.auth_file).unwrap_or_default();

    let key = prompt_secret(&format!("API key for {provider}"))?;
    if key.is_empty() {
        println!("  No key entered, nothing changed.");
        return Ok(());
    }

    auth.set_api_key(provider, key);
    auth.save(&paths.auth_file)?;
    println!("  ✓ {provider} key saved");
    Ok(())
}

/// List all configured API keys (masked).
pub async fn cmd_list_keys() -> anyhow::Result<()> {
    let auth = AuthStore::default();

    let all_providers = [
        ("anthropic", "ANTHROPIC_API_KEY"),
        ("openai", "OPENAI_API_KEY"),
        ("google", "GOOGLE_API_KEY"),
        ("mistral", "MISTRAL_API_KEY"),
        ("brave", ""),
        ("tavily", ""),
    ];

    println!("\n  Configured API keys:\n");

    let mut found = false;
    for (name, env_var) in &all_providers {
        if let Some(masked) = auth.get_masked_key(name) {
            let source = if !env_var.is_empty() && std::env::var(env_var).is_ok() {
                "env"
            } else {
                "stored"
            };
            println!("  {name:<12} {masked:<20} ({source})");
            found = true;
        }
    }

    // Check Ollama separately (doesn't need an API key)
    if auth.get_masked_key("ollama").is_some() {
        println!("  {:<12} {:<20} (stored)", "ollama", auth.get_masked_key("ollama").unwrap());
        found = true;
    } else if std::env::var("OLLAMA_HOST").is_ok() {
        println!("  {:<12} {:<20} (env)", "ollama", "OLLAMA_HOST set");
        found = true;
    }

    if !found {
        println!("  No API keys configured.");
        println!("  Run `fin config set-key <provider>` to add one.");
        println!("  Or start Ollama locally: ollama serve");
    }

    println!();
    Ok(())
}

/// Remove a stored API key.
pub async fn cmd_remove_key(provider: &str) -> anyhow::Result<()> {
    let paths = FinPaths::resolve()?;
    let mut auth = AuthStore::load(&paths.auth_file).unwrap_or_default();

    if auth.get_api_key(provider).is_none() {
        println!("  No key found for {provider}.");
        return Ok(());
    }

    auth.remove_api_key(provider);
    auth.save(&paths.auth_file)?;
    println!("  ✓ {provider} key removed");

    // Warn if env var is still set
    let env_key = match provider {
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        "google" => Some("GOOGLE_API_KEY"),
        "mistral" => Some("MISTRAL_API_KEY"),
        _ => None,
    };
    if let Some(var) = env_key {
        if std::env::var(var).is_ok() {
            println!("  Note: {var} is still set in your environment.");
        }
    }

    Ok(())
}

/// Check if onboarding should run.
pub fn should_run_onboarding(auth: &AuthStore) -> bool {
    !auth.has_any_provider()
}

/// Prompt for secret input (hidden, no echo).
fn prompt_secret(label: &str) -> anyhow::Result<String> {
    let key = rpassword::prompt_password(format!("  {label}: "))?;
    Ok(key.trim().to_string())
}

fn prompt_input(label: &str) -> anyhow::Result<String> {
    print!("  {label}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
