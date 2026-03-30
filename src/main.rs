// Fin — AI Coding Agent Entry Point
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

mod agent;
mod agents;
mod cli;
mod config;
mod db;
mod error;
mod extensions;
mod io;
mod llm;
mod onboarding;
mod sessions;
mod skills;
mod tools;
mod workflow;
mod worktree;

#[cfg(feature = "tui")]
mod tui;

use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments first so we know which mode we're in
    let args = cli::Cli::parse();

    // Determine if we're in TUI mode (stderr would corrupt the display)
    let is_tui = !args.print && args.prompt.is_none() && args.command.is_none();

    // Initialize tracing
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("fin=info"));

    // File layer — always active (detailed with timestamps)
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
        .join("fin")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, "fin.log");
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true);

    // Stderr layer — only in non-TUI modes (would corrupt TUI display)
    let stderr_layer = if is_tui {
        None
    } else {
        Some(fmt::layer().with_target(false).with_writer(std::io::stderr))
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    // Route to appropriate mode
    match args.run().await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!("{e:#}");
            std::process::exit(1);
        }
    }
}
