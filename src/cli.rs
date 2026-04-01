// Fin — CLI Argument Parsing & Mode Routing
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "fin",
    version,
    about = "AI coding agent — You give it a prompt. It builds the thing."
)]
pub struct Cli {
    /// Prompt to execute (interactive if omitted)
    pub prompt: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    /// Output mode
    #[arg(long, value_enum, default_value = "interactive")]
    pub mode: Mode,

    /// LLM model override (e.g., claude-sonnet-4-6, gpt-4.1)
    #[arg(long)]
    pub model: Option<String>,

    /// Resume most recent session
    #[arg(short, long)]
    pub r#continue: bool,

    /// Single-shot print mode (no session persistence)
    #[arg(short, long)]
    pub print: bool,

    /// Launch HTTP API server
    #[arg(long)]
    pub web: bool,

    /// Server bind port
    #[arg(long, default_value = "3000")]
    pub port: u16,

    /// Server bind address
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Worktree isolation (optional name)
    #[arg(short, long)]
    pub worktree: Option<Option<String>>,

    /// Restrict available tools (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub tools: Option<Vec<String>>,

    /// Disable session persistence
    #[arg(long)]
    pub no_session: bool,

    /// Load additional extension
    #[arg(long)]
    pub extension: Option<Vec<String>>,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run headless (non-interactive)
    Headless {
        /// The prompt or action to execute
        prompt: String,

        /// Timeout in milliseconds
        #[arg(long, default_value = "300000")]
        timeout: u64,

        /// Stream JSONL events to stdout
        #[arg(long)]
        json: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        output_format: OutputFormat,

        /// Auto-respond to extension UI requests
        #[arg(long)]
        auto: bool,

        /// Pre-supply answers JSON file path
        #[arg(long)]
        answers: Option<String>,
    },

    /// Start MCP server (stdio transport)
    Mcp,

    /// Start HTTP API server
    Serve {
        /// Port to bind
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Address to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// List and manage sessions
    Sessions {
        #[command(subcommand)]
        action: Option<SessionAction>,
    },

    /// Manage worktrees
    Worktree {
        #[command(subcommand)]
        action: WorktreeAction,
    },

    /// List available models
    Models {
        /// Filter by search term
        search: Option<String>,
    },

    // ── Workflow Commands ──────────────────────────────────────
    /// Initialize .fin/ workflow directory
    Init,

    /// Show current workflow status
    Status,

    /// Advance to next logical step
    Next,

    /// Create and manage blueprints
    Blueprint {
        #[command(subcommand)]
        action: BlueprintAction,
    },

    /// Run a specific workflow stage (define, explore, architect, build, validate)
    Stage {
        /// Stage name
        name: String,
        /// Auto-mode (skip interactive questions)
        #[arg(long)]
        auto: bool,
    },

    /// Run autonomously — dispatch → build → validate → repeat until done
    Auto,

    /// Create PR, final review, merge
    Ship,

    /// Resume from handoff.md
    Resume,

    /// Write handoff.md and pause
    Pause,

    /// Map the codebase — generate .fin/CODEBASE_MAP.md for agent reference
    Map,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set an API key for a provider (input is hidden)
    SetKey {
        /// Provider name (anthropic, openai, google, mistral, brave, tavily)
        provider: String,
    },
    /// List configured API keys (masked)
    ListKeys,
    /// Remove a stored API key
    RemoveKey {
        /// Provider name to remove
        provider: String,
    },
}

#[derive(Subcommand)]
pub enum BlueprintAction {
    /// Create a new blueprint
    New {
        /// Blueprint name/title
        name: String,
    },
    /// Complete current blueprint
    Complete,
    /// List blueprints
    List,
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List recent sessions
    List,
    /// Resume a specific session
    Resume { id: String },
}

#[derive(Subcommand)]
pub enum WorktreeAction {
    /// List active worktrees
    List,
    /// Create a new worktree
    Create { name: String },
    /// Merge worktree back to main
    Merge { name: String },
    /// Remove a worktree
    Remove { name: String },
    /// Clean up stale worktrees
    Clean,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum Mode {
    /// Full interactive TUI
    #[default]
    Interactive,
    /// Plain text output
    Text,
    /// JSON output
    Json,
    /// Streaming JSON (JSONL)
    StreamJson,
    /// RPC protocol (stdin/stdout JSONL)
    Rpc,
    /// MCP protocol
    Mcp,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        // Handle subcommands first
        if let Some(cmd) = self.command {
            return match cmd {
                Command::Headless {
                    prompt,
                    timeout,
                    json,
                    output_format: _,
                    auto,
                    answers: _,
                } => {
                    tracing::info!("Starting headless mode");
                    crate::io::headless::run(prompt, timeout, json, auto).await
                }
                Command::Mcp => {
                    tracing::info!("Starting MCP server");
                    crate::io::mcp::run().await
                }
                Command::Serve { port, host } => {
                    tracing::info!("Starting HTTP server on {host}:{port}");
                    crate::io::http::run(&host, port).await
                }
                Command::Config { action } => match action {
                    Some(ConfigAction::SetKey { provider }) => {
                        crate::onboarding::cmd_set_key(&provider).await
                    }
                    Some(ConfigAction::ListKeys) => {
                        crate::onboarding::cmd_list_keys().await
                    }
                    Some(ConfigAction::RemoveKey { provider }) => {
                        crate::onboarding::cmd_remove_key(&provider).await
                    }
                    None => crate::onboarding::run_wizard().await,
                },
                Command::Models { search } => {
                    crate::llm::list_models(search.as_deref());
                    Ok(())
                }
                Command::Sessions { action } => crate::sessions::handle_sessions(action).await,
                Command::Worktree { action } => crate::worktree::handle_worktree(action).await,

                // Workflow commands
                Command::Init => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_init(&cwd).await
                }
                Command::Status => {
                    let cwd = std::env::current_dir()?;
                    let status = crate::workflow::commands::cmd_status(&cwd).await?;
                    println!("{status}");
                    Ok(())
                }
                Command::Next => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_next(&cwd, self.model.as_deref()).await
                }
                Command::Blueprint { action } => {
                    let cwd = std::env::current_dir()?;
                    match action {
                        BlueprintAction::New { name } => {
                            crate::workflow::commands::cmd_blueprint_new(&cwd, &name).await
                        }
                        BlueprintAction::Complete => {
                            crate::workflow::commands::cmd_blueprint_complete(&cwd).await
                        }
                        BlueprintAction::List => {
                            crate::workflow::commands::cmd_blueprint_list(&cwd).await
                        }
                    }
                }
                Command::Stage { name, auto } => {
                    let cwd = std::env::current_dir()?;
                    if auto {
                        crate::workflow::commands::cmd_auto(&cwd, self.model.as_deref()).await
                    } else {
                        crate::workflow::commands::cmd_stage(&cwd, &name, self.model.as_deref())
                            .await
                    }
                }
                Command::Auto => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_auto(&cwd, self.model.as_deref()).await
                }
                Command::Ship => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_ship(&cwd).await
                }
                Command::Resume => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_resume(&cwd, self.model.as_deref()).await
                }
                Command::Pause => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_pause(&cwd)
                }
                Command::Map => {
                    let cwd = std::env::current_dir()?;
                    crate::workflow::commands::cmd_map(&cwd, self.model.as_deref()).await
                }
            };
        }

        // Handle --continue flag (resume most recent session)
        if self.r#continue {
            let paths = crate::config::paths::FinPaths::resolve()?;
            let store = crate::db::session::SessionStore::new(&paths.sessions_dir)?;
            let sessions = store.list()?;
            let latest = sessions
                .first()
                .ok_or_else(|| anyhow::anyhow!("No sessions to resume."))?;
            let messages = store.load(&latest.id)?;
            if messages.is_empty() {
                anyhow::bail!("Most recent session is empty.");
            }
            // If a prompt was also given, append it and run in print mode
            if let Some(ref prompt) = self.prompt {
                return crate::io::print::run_with_prompt_and_session(
                    &latest.id,
                    messages,
                    prompt,
                    self.model.as_deref(),
                )
                .await;
            }
            // Otherwise, resume in TUI (interactive) mode by falling through
            // The TUI already handles resume_session via the --continue flag
        }

        // Auto-onboarding: if no API key found and interactive, show wizard
        if !self.print && self.prompt.is_none() {
            let auth = crate::config::auth::AuthStore::default();
            if crate::onboarding::should_run_onboarding(&auth) {
                eprintln!("No API key found. Starting setup wizard...\n");
                crate::onboarding::run_wizard().await?;
                // Re-check after wizard — if still no key, exit
                let auth = crate::config::auth::AuthStore::default();
                if crate::onboarding::should_run_onboarding(&auth) {
                    eprintln!(
                        "No API key configured. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY."
                    );
                    return Ok(());
                }
            }
        }

        // Handle --print mode
        if self.print {
            let prompt = self.prompt.unwrap_or_default();
            return crate::io::print::run(&prompt, self.model.as_deref()).await;
        }

        // Handle --web mode
        if self.web {
            return crate::io::http::run(&self.host, self.port).await;
        }

        // Route by mode
        match self.mode {
            Mode::Interactive => {
                #[cfg(feature = "tui")]
                {
                    crate::tui::run(self).await
                }
                #[cfg(not(feature = "tui"))]
                {
                    anyhow::bail!(
                        "TUI feature not enabled. Compile with --features tui or use --mode text"
                    )
                }
            }
            Mode::Text | Mode::Json | Mode::StreamJson => {
                let prompt = self.prompt.unwrap_or_default();
                crate::io::print::run(&prompt, self.model.as_deref()).await
            }
            Mode::Rpc => crate::io::rpc::run().await,
            Mode::Mcp => crate::io::mcp::run().await,
        }
    }
}
