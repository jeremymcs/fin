// Fin + AI Coding Agent Entry Point

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

    // File layer — best-effort only. Never panic if filesystem is restricted.
    let file_appender = init_file_appender();

    // Stderr layer — only in non-TUI modes (would corrupt TUI display)
    let stderr_layer = if is_tui {
        None
    } else {
        Some(fmt::layer().with_target(false).with_writer(std::io::stderr))
    };

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer);

    if let Some(file_appender) = file_appender {
        registry
            .with(
                fmt::layer()
                    .with_writer(file_appender)
                    .with_ansi(false)
                    .with_target(true),
            )
            .init();
    } else {
        registry.init();
    }

    // Route to appropriate mode
    match args.run().await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!("{e:#}");
            std::process::exit(1);
        }
    }
}

/// Initialize rolling file logs in a non-fatal way.
/// If the path is not writable, we fall back to stderr-only logging.
fn init_file_appender() -> Option<tracing_appender::rolling::RollingFileAppender> {
    let log_dir = crate::config::paths::FinPaths::resolve()
        .map(|p| p.data_dir.join("logs"))
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
                .join("fin")
                .join("logs")
        });

    if let Err(err) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "fin: warning: file logging disabled (failed to create {}: {err})",
            log_dir.display()
        );
        return None;
    }

    match std::panic::catch_unwind(|| tracing_appender::rolling::daily(&log_dir, "fin.log")) {
        Ok(appender) => Some(appender),
        Err(_) => {
            eprintln!(
                "fin: warning: file logging disabled (failed to initialize log file at {})",
                log_dir.display()
            );
            None
        }
    }
}
