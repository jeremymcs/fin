// Fin — LLM Provider Layer
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

pub mod anthropic;
pub mod bedrock;
pub mod google;
pub mod models;
pub mod ollama;
pub mod openai;
pub mod provider;
pub mod stream;
pub mod types;
pub mod vertex;

use models::ModelConfig;

/// List available models, optionally filtered by search term.
/// Includes cloud models and locally discovered Ollama models.
pub fn list_models(search: Option<&str>) {
    let mut models = models::default_models();

    // Try to discover local Ollama models (non-blocking with short timeout)
    let local_models = discover_ollama_models_sync();
    if !local_models.is_empty() {
        for m in &local_models {
            models.push(m.to_model_config());
        }
    }

    let filtered: Vec<&ModelConfig> = match search {
        Some(term) => {
            let term = term.to_lowercase();
            models
                .iter()
                .filter(|m| {
                    m.id.to_lowercase().contains(&term)
                        || m.display_name.to_lowercase().contains(&term)
                        || m.provider.to_lowercase().contains(&term)
                })
                .collect()
        }
        None => models.iter().collect(),
    };

    if filtered.is_empty() {
        println!("No models found.");
        return;
    }

    let aliases = models::model_aliases();

    println!(
        "{:<35} {:<12} {:<10} {:<14} {}",
        "Model", "Provider", "Context", "$/1M in/out", "Alias"
    );
    println!("{}", "-".repeat(85));
    for m in filtered {
        let cost = if m.cost.input_per_million == 0.0 && m.cost.output_per_million == 0.0 {
            "free (local)".to_string()
        } else {
            format!("${:.2}/${:.2}", m.cost.input_per_million, m.cost.output_per_million)
        };
        // Collect all aliases that point to this model
        let model_aliases: Vec<&str> = aliases
            .iter()
            .filter(|(_, full_id)| *full_id == m.id)
            .map(|(alias, _)| *alias)
            .collect();
        let alias_str = if model_aliases.is_empty() {
            String::new()
        } else {
            model_aliases.join(", ")
        };
        println!(
            "{:<35} {:<12} {:<10} {:<14} {}",
            m.display_name,
            m.provider,
            m.context_window,
            cost,
            alias_str,
        );
    }
}

/// Blocking Ollama model discovery (used by CLI commands).
fn discover_ollama_models_sync() -> Vec<ollama::OllamaModel> {
    let client = reqwest::Client::new();
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // Already in a tokio runtime — use spawn_blocking to avoid nested block_on
            return std::thread::scope(|_| {
                let client = client.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                handle.spawn(async move {
                    let models = ollama::OllamaProvider::discover_models(&client).await;
                    let _ = tx.send(models);
                });
                rx.recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap_or_default()
            });
        }
        Err(_) => tokio::runtime::Runtime::new().ok(),
    };

    match rt {
        Some(rt) => rt.block_on(ollama::OllamaProvider::discover_models(&client)),
        None => Vec::new(),
    }
}
