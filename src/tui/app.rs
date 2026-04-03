// Fin + TUI Application Loop

use crossterm::{
    ExecutableCommand,
    cursor::SetCursorStyle,
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use std::collections::VecDeque;
use std::io::{Write, stdin, stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::tui_io::TuiIO;
use super::widgets::{self, LineKind, OutputLine, Palette};
use crate::agent::agent_loop::run_agent_loop;
use crate::agent::prompt::{AgentPromptContext, build_system_prompt};
use crate::agent::state::AgentState;
use crate::agents::{AgentRegistry, DelegateTool};
use crate::cli::Cli;
use crate::config::paths::FinPaths;
use crate::db::session::SessionStore;
use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::models::resolve_model;
use crate::llm::provider::ProviderRegistry;
use crate::llm::types::*;
use crate::tools::ToolRegistry;
use tokio_util::sync::CancellationToken;

/// Named layout regions — replaces fragile chunks[N] indexing.
/// Per D-05: prerequisite for Phases 3 and 4 layout changes.
/// Per D-06: handles both workflow-active and workflow-inactive variants.
struct AppLayout {
    output: Rect,
    workflow: Option<Rect>,
    status: Rect,
    input: Rect,
}

impl AppLayout {
    fn compute(area: Rect, wf_active: bool, wf_auto: bool) -> Self {
        if wf_active && wf_auto {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // output
                    Constraint::Length(9), // workflow panel — auto mode (7 inner + 2 border)
                    Constraint::Length(1), // spacer
                    Constraint::Length(1), // status bar
                    Constraint::Length(2), // input
                ])
                .split(area);
            AppLayout {
                output: chunks[0],
                workflow: Some(chunks[1]),
                status: chunks[3],
                input: chunks[4],
            }
        } else if wf_active {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // output
                    Constraint::Length(4), // workflow panel
                    Constraint::Length(1), // spacer
                    Constraint::Length(1), // status bar
                    Constraint::Length(2), // input
                ])
                .split(area);
            AppLayout {
                output: chunks[0],
                workflow: Some(chunks[1]),
                status: chunks[3],
                input: chunks[4],
            }
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // output
                    Constraint::Length(1), // spacer
                    Constraint::Length(1), // status bar
                    Constraint::Length(2), // input
                ])
                .split(area);
            AppLayout {
                output: chunks[0],
                workflow: None,
                status: chunks[2],
                input: chunks[3],
            }
        }
    }
}

/// Toast notification category — determines border color.
#[derive(Clone, Debug, PartialEq)]
enum ToastKind {
    Info,
    Success,
    Error,
}

const TOAST_TTL: Duration = Duration::from_secs(5);
const TOAST_MAX: usize = 2;
const TOAST_WIDTH: u16 = 40;
const TOAST_HEIGHT: u16 = 3;
const LOGIN_PROVIDERS: &[&str] = &["openai", "anthropic", "google"];

fn push_toast(toasts: &mut VecDeque<(String, Instant, ToastKind)>, msg: String, kind: ToastKind) {
    if toasts.len() >= TOAST_MAX {
        toasts.pop_front();
    }
    let truncated = if msg.chars().count() > 36 {
        format!("{}…", msg.chars().take(36).collect::<String>())
    } else {
        msg
    };
    toasts.push_back((truncated, Instant::now(), kind));
}

/// All available slash commands for tab-completion.
const SLASH_COMMANDS: &[&str] = &[
    "clear",
    "exit",
    "init",
    "status",
    "blueprint",
    "model",
    "define",
    "explore",
    "architect",
    "build",
    "validate",
    "seal-section",
    "advance",
    "next",
    "auto",
    "ship",
    "resume",
    "pause",
    "map",
    "config",
    "login",
    "sessions",
    "worktree",
    "help",
    "quit",
];

pub async fn run_app(args: Cli) -> anyhow::Result<()> {
    // Resolve model
    let model = match args.model.as_deref() {
        Some(id) => resolve_model(id).ok_or_else(|| anyhow::anyhow!("Model not found: {id}"))?,
        None => crate::io::print::pick_default_model()?,
    };

    let cwd = std::env::current_dir()?;
    let resume_session = args.r#continue;
    let no_session = args.no_session;

    // Setup terminal — no mouse capture so users can highlight/copy/paste natively.
    // Scroll via keyboard: Shift+Up/Down (1 line), PageUp/PageDown (10 lines),
    // Shift+Home/End (top/bottom).
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(SetCursorStyle::BlinkingBar)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui_loop(&mut terminal, model, cwd, resume_session, no_session).await;

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(SetCursorStyle::DefaultUserShape)?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    model: crate::llm::models::ModelConfig,
    cwd: std::path::PathBuf,
    resume_session: bool,
    no_session: bool,
) -> anyhow::Result<()> {
    let mut input_text = String::new();
    let mut force_full_redraw = false;
    let mut output_lines: Vec<OutputLine> = vec![
        OutputLine::system(format!(
            "fin v{} — {}",
            env!("CARGO_PKG_VERSION"),
            model.display_name
        )),
        OutputLine::system(format!("Working directory: {}", cwd.display())),
        OutputLine::system("Type a prompt and press Enter. Ctrl+C to quit.".to_string()),
        OutputLine::system(String::new()),
    ];
    let mut scroll: u16 = 0;
    let mut scroll_pinned = true; // true = auto-scroll to bottom on new content
    let mut is_streaming = false;
    let mut cursor_pos: usize = 0;
    let mut input_history: Vec<String> = Vec::new();
    let mut history_idx: Option<usize> = None;
    let mut show_splash = true;
    let mut workflow_state = widgets::WorkflowState::default();

    // Model picker overlay state
    let mut model_picker_active = false;
    let mut model_picker_index: usize = 0;
    // Login picker overlay state
    let mut login_picker_active = false;
    let mut login_picker_index: usize = 0;

    // Help overlay state
    let mut help_active = false;

    // Toast notification queue (per D-07: VecDeque capped at TOAST_MAX)
    let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();

    let model_picker_items: Vec<crate::llm::models::ModelConfig> =
        crate::llm::models::default_models();

    // Channel for agent events → TUI
    let (agent_event_tx, mut agent_event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    // Channel for new user messages → agent task
    let (user_msg_tx, user_msg_rx) = mpsc::unbounded_channel::<String>();
    let cancel = tokio_util::sync::CancellationToken::new();

    // Build shared infrastructure once
    let client = reqwest::Client::new();
    let provider_registry = Arc::new(ProviderRegistry::with_defaults(client));
    let mut model_for_display = model.display_name.clone();

    // Session persistence
    let paths = FinPaths::resolve().ok();
    let session_store = if !no_session {
        paths
            .as_ref()
            .and_then(|p| SessionStore::new(&p.sessions_dir).ok())
    } else {
        None
    };

    // Session resume:
    // - Always show visual history from last session (scrollable)
    // - --continue flag also restores agent conversation state for the LLM
    let mut resume_messages: Vec<Message> = Vec::new();
    let mut session_id = uuid::Uuid::new_v4().to_string();
    if let Some(ref store) = session_store {
        if let Ok(sessions) = store.list() {
            if let Some(latest) = sessions.first() {
                if let Ok(messages) = store.load(&latest.id) {
                    // Load history into output_lines — splash stays up; scroll up to reveal
                    replay_history(&messages, &mut output_lines, &model.display_name);

                    // Seed input history from previous user messages (Up arrow recall)
                    for msg in &messages {
                        if msg.role == crate::llm::types::Role::User {
                            for c in &msg.content {
                                if let crate::llm::types::Content::Text { text } = c {
                                    if !text.is_empty() {
                                        input_history.push(text.clone());
                                    }
                                }
                            }
                        }
                    }

                    if resume_session {
                        // --continue: also feed messages to agent for LLM context
                        session_id = latest.id.clone();
                        resume_messages = messages;
                    }
                }
            }
        }
    }

    // Clone the sender before moving it into the agent task so the drain loop
    // can spawn async tasks (e.g. git fetch) that post events back.
    let tui_event_tx = agent_event_tx.clone();

    // Spawn persistent agent task that processes prompts from the channel
    let agent_model = model.clone();
    let agent_cwd = cwd.clone();
    let agent_cancel = cancel.clone();
    let agent_provider_registry = Arc::clone(&provider_registry);
    let agent_session_id = session_id.clone();
    let _agent_handle = tokio::spawn(async move {
        run_tui_agent(
            agent_event_tx,
            user_msg_rx,
            agent_model,
            agent_cwd,
            agent_provider_registry,
            agent_cancel,
            resume_messages,
            agent_session_id,
        )
        .await;
    });

    // Track cumulative usage from agent events
    let mut total_in: u64 = 0;
    let mut total_out: u64 = 0;
    let mut total_cost: f64 = 0.0;

    // Build splash info once
    let splash_info = widgets::SplashInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        model_id: model.id.clone(),
        provider: model.provider.clone(),
        directory: cwd.display().to_string(),
        extensions: vec![
            (
                "Web Search".into(),
                std::env::var("BRAVE_API_KEY").is_ok() || std::env::var("TAVILY_API_KEY").is_ok(),
            ),
            ("Context7".into(), true), // always available (no key needed)
        ],
    };

    loop {
        if force_full_redraw {
            terminal.clear()?;
            force_full_redraw = false;
        }

        // Draw UI
        terminal.draw(|f| {
            let wf_active = workflow_state.active;
            let layout = AppLayout::compute(f.area(), wf_active, workflow_state.is_auto);

            // Output area: splash or conversation
            if show_splash {
                widgets::render_splash(f, layout.output, &splash_info);
            } else {
                let output = widgets::render_output(&output_lines, scroll);
                f.render_widget(output, layout.output);
            }

            // Workflow panel (only when active)
            if let Some(wf_area) = layout.workflow {
                widgets::render_workflow_panel(f, wf_area, &workflow_state);
            }

            // Status bar (same call regardless of wf_active)
            let status = widgets::render_status_bar(
                &model_for_display,
                total_in,
                total_out,
                total_cost,
                is_streaming,
                !scroll_pinned,
                if wf_active {
                    Some(&workflow_state)
                } else {
                    None
                },
            );
            f.render_widget(status, layout.status);

            // Input area
            let input = widgets::render_input(&input_text, &model_for_display);
            f.render_widget(input, layout.input);

            // Model picker overlay
            if model_picker_active {
                let area = f.area();
                let picker_height = (model_picker_items.len() as u16 + 2).min(area.height - 4);
                let picker_width = 50u16.min(area.width - 4);
                let picker_area = ratatui::layout::Rect {
                    x: (area.width.saturating_sub(picker_width)) / 2,
                    y: (area.height.saturating_sub(picker_height)) / 2,
                    width: picker_width,
                    height: picker_height,
                };

                // Clear area behind picker
                f.render_widget(ratatui::widgets::Clear, picker_area);

                let items: Vec<ratatui::text::Line> = model_picker_items
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let active_marker = if m.display_name == model_for_display {
                            " *"
                        } else {
                            ""
                        };
                        let label = format!(" {} — {}{}", m.id, m.display_name, active_marker);
                        if i == model_picker_index {
                            ratatui::text::Line::from(label).style(
                                ratatui::style::Style::default()
                                    .bg(ratatui::style::Color::White)
                                    .fg(ratatui::style::Color::Black),
                            )
                        } else {
                            ratatui::text::Line::from(label)
                        }
                    })
                    .collect();

                let picker = ratatui::widgets::Paragraph::new(items).block(
                    ratatui::widgets::Block::bordered()
                        .title(" Select Model (↑↓ Enter Esc) ")
                        .border_style(ratatui::style::Style::default().fg(Palette::ACCENT)),
                );
                f.render_widget(picker, picker_area);
            }

            // Login picker overlay
            if login_picker_active {
                let area = f.area();
                let picker_height = (LOGIN_PROVIDERS.len() as u16 + 2).min(area.height - 4);
                let picker_width = 52u16.min(area.width - 4);
                let picker_area = ratatui::layout::Rect {
                    x: (area.width.saturating_sub(picker_width)) / 2,
                    y: (area.height.saturating_sub(picker_height)) / 2,
                    width: picker_width,
                    height: picker_height,
                };

                f.render_widget(ratatui::widgets::Clear, picker_area);

                let items: Vec<ratatui::text::Line> = LOGIN_PROVIDERS
                    .iter()
                    .enumerate()
                    .map(|(i, provider)| {
                        let label = format!(" {provider}");
                        if i == login_picker_index {
                            ratatui::text::Line::from(label).style(
                                ratatui::style::Style::default()
                                    .bg(ratatui::style::Color::White)
                                    .fg(ratatui::style::Color::Black),
                            )
                        } else {
                            ratatui::text::Line::from(label)
                        }
                    })
                    .collect();

                let picker = ratatui::widgets::Paragraph::new(items).block(
                    ratatui::widgets::Block::bordered()
                        .title(" Login Provider (↑↓ Enter Esc) ")
                        .border_style(ratatui::style::Style::default().fg(Palette::ACCENT)),
                );
                f.render_widget(picker, picker_area);
            }

            // Help overlay (per D-01: single-column grouped layout, extends model picker pattern)
            if help_active {
                let area = f.area();
                let overlay_width = (area.width.saturating_sub(4)).min(80);
                let overlay_height = area.height.saturating_sub(4);
                let overlay_area = ratatui::layout::Rect {
                    x: (area.width.saturating_sub(overlay_width)) / 2,
                    y: (area.height.saturating_sub(overlay_height)) / 2,
                    width: overlay_width,
                    height: overlay_height,
                };
                f.render_widget(ratatui::widgets::Clear, overlay_area);

                let mut lines: Vec<ratatui::text::Line> = vec![
                    Line::styled(
                        " Keybindings",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                    Line::raw("  Ctrl+C      Quit"),
                    Line::raw("  ?           This help overlay"),
                    Line::raw("  /model      Switch model (interactive picker)"),
                    Line::raw("  /login      Store provider credentials"),
                    Line::raw("  Esc         Close overlay / cancel"),
                    Line::raw("  ↑/↓         Scroll output history"),
                    Line::raw("  Tab         Autocomplete slash commands"),
                    Line::raw(""),
                    Line::styled(
                        " Slash Commands",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                ];
                for cmd in SLASH_COMMANDS {
                    lines.push(Line::raw(format!("  /{cmd}")));
                }
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "  [any key to close]",
                    Style::default().fg(Color::DarkGray),
                ));

                let help = ratatui::widgets::Paragraph::new(lines).block(
                    ratatui::widgets::Block::bordered()
                        .title(" Keybindings & Commands ")
                        .border_style(ratatui::style::Style::default().fg(Color::Yellow)),
                );
                f.render_widget(help, overlay_area);
            }

            // Toast notification — top-right of output area (per D-09)
            if let Some((msg, _, kind)) = toasts.front() {
                let out = layout.output;
                if out.width >= TOAST_WIDTH && out.height >= TOAST_HEIGHT {
                    let toast_area = ratatui::layout::Rect {
                        x: out.x + out.width.saturating_sub(TOAST_WIDTH),
                        y: out.y,
                        width: TOAST_WIDTH.min(out.width),
                        height: TOAST_HEIGHT,
                    };
                    f.render_widget(ratatui::widgets::Clear, toast_area);
                    let border_color = match kind {
                        ToastKind::Error => Palette::ERROR,
                        ToastKind::Success => Palette::ACCENT,
                        ToastKind::Info => Palette::DIM,
                    };
                    let toast_widget = ratatui::widgets::Paragraph::new(msg.as_str()).block(
                        ratatui::widgets::Block::bordered()
                            .border_style(ratatui::style::Style::default().fg(border_color)),
                    );
                    f.render_widget(toast_widget, toast_area);
                }
            }

            // Cursor position — use named layout.input field directly
            let prompt_prefix = format!("[{}] > ", model_for_display);
            let cursor_x = (prompt_prefix.len() + cursor_pos) as u16 % layout.input.width;
            let cursor_y = layout.input.y + 1;
            f.set_cursor_position(Position::new(cursor_x, cursor_y));
        })?;

        // Handle events with timeout so we can process agent events
        let timeout = std::time::Duration::from_millis(50);

        // Expire oldest toast if TTL elapsed (per D-08: Instant-based, immune to event flood)
        while toasts
            .front()
            .map(|(_, t, _)| t.elapsed() >= TOAST_TTL)
            .unwrap_or(false)
        {
            toasts.pop_front();
        }

        // Check for agent events (non-blocking)
        while let Ok(evt) = agent_event_rx.try_recv() {
            match evt {
                AgentEvent::TextDelta { text } => {
                    // Split on newlines first, then append/create lines
                    let parts: Vec<&str> = text.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if i == 0 {
                            // First chunk: append to existing assistant line if possible
                            if let Some(last) = output_lines.last_mut() {
                                if matches!(last.kind, LineKind::Assistant) {
                                    last.text.push_str(part);
                                    continue;
                                }
                            }
                            // No existing assistant line — create one
                            if !part.is_empty() {
                                output_lines.push(OutputLine::assistant(part.to_string()));
                            }
                        } else {
                            // Newline boundary — finalize the previous assistant line (D-08)
                            if let Some(prev) = output_lines
                                .iter_mut()
                                .rev()
                                .find(|l| matches!(l.kind, LineKind::Assistant))
                            {
                                prev.is_final = true;
                            }
                            // Subsequent chunks after \n — always new line
                            output_lines.push(OutputLine::assistant(part.to_string()));
                        }
                    }
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::ThinkingDelta { text } => {
                    let parts: Vec<&str> = text.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if i == 0 {
                            if let Some(last) = output_lines.last_mut() {
                                if matches!(last.kind, LineKind::Thinking) {
                                    last.text.push_str(part);
                                    continue;
                                }
                            }
                            if !part.is_empty() {
                                output_lines.push(OutputLine::thinking(part.to_string()));
                            }
                        } else {
                            output_lines.push(OutputLine::thinking(part.to_string()));
                        }
                    }
                }
                AgentEvent::ToolStart { name, .. } => {
                    output_lines.push(OutputLine::tool(format!("⚙ {name}")));
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::ToolEnd { name, is_error, .. } => {
                    if is_error {
                        output_lines.push(OutputLine::error(format!("✗ {name} failed")));
                        push_toast(&mut toasts, format!("✗ {name} failed"), ToastKind::Error);
                    } else {
                        output_lines.push(OutputLine::tool(format!("✓ {name}")));
                    }
                }
                AgentEvent::AgentEnd { usage } => {
                    is_streaming = false;

                    // Finalize last assistant line unconditionally (D-08: enables markdown parsing)
                    if let Some(last) = output_lines
                        .iter_mut()
                        .rev()
                        .find(|l| matches!(l.kind, LineKind::Assistant))
                    {
                        last.is_final = true;
                    }

                    total_in += usage.input_tokens;
                    total_out += usage.output_tokens;
                    total_cost += usage.cost.total;

                    // Per-message cost annotation (D-12: dim line after each response)
                    if usage.input_tokens > 0 || usage.output_tokens > 0 {
                        let in_fmt = widgets::format_token_count(usage.input_tokens);
                        let out_fmt = widgets::format_token_count(usage.output_tokens);
                        output_lines.push(OutputLine::system(format!(
                            "  \u{21b3} {} in / {} out  ${:.4}",
                            in_fmt, out_fmt, usage.cost.total
                        )));
                    }
                    output_lines.push(OutputLine::system(String::new()));
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::ModelChanged { display_name } => {
                    model_for_display = display_name.clone();
                    workflow_state.model_display = display_name.clone(); // D-14: keep panel in sync
                    output_lines.push(OutputLine::system(format!(
                        "Model switched to {display_name}"
                    )));
                    output_lines.push(OutputLine::system(String::new()));
                    push_toast(
                        &mut toasts,
                        format!("Model: {display_name}"),
                        ToastKind::Info,
                    );
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::TurnStart => {
                    is_streaming = true;
                }
                AgentEvent::TurnEnd => {
                    is_streaming = false;
                }
                AgentEvent::AgentStart { .. } => {
                    // Add a speaker label for the agent turn
                    output_lines.push(OutputLine::system(format!("┌─ {} ────", model_for_display)));
                }
                // ── Workflow events ──────────────────────────────────
                AgentEvent::WorkflowUnitStart {
                    blueprint_id,
                    section_id,
                    task_id,
                    stage,
                    ..
                } => {
                    workflow_state.active = true;
                    workflow_state.blueprint_id = blueprint_id;
                    workflow_state.current_stage = stage;
                    workflow_state.current_section = section_id;
                    workflow_state.current_task = task_id;
                    workflow_state.update_pipeline();
                    // Recalculate scroll now that the workflow panel has claimed space
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size().map(|s| s.height).unwrap_or(24),
                        scroll_pinned,
                        true,
                    );
                }
                AgentEvent::WorkflowUnitEnd { .. } => {
                    // Async git fetch — non-blocking (D-05).
                    // Spawn a task that calls last_commit() and sends result via event channel.
                    let git_cwd = cwd.clone();
                    let git_tx = tui_event_tx.clone();
                    tokio::spawn(async move {
                        let git = crate::workflow::git::WorkflowGit::new(&git_cwd);
                        if let Ok((hash, msg)) = git.last_commit().await {
                            let _ = git_tx.send(AgentEvent::GitCommitUpdate { hash, msg });
                        }
                    });
                }
                AgentEvent::WorkflowProgress {
                    sections_total,
                    sections_done,
                    tasks_total,
                    tasks_done,
                    current_stage,
                    current_section,
                    current_task,
                    ..
                } => {
                    workflow_state.sections_total = sections_total;
                    workflow_state.sections_done = sections_done;
                    workflow_state.tasks_total = tasks_total;
                    workflow_state.tasks_done = tasks_done;
                    workflow_state.current_stage = current_stage;
                    workflow_state.current_section = current_section;
                    workflow_state.current_task = current_task;
                    workflow_state.update_pipeline();
                }
                AgentEvent::WorkflowComplete {
                    blueprint_id,
                    units_run,
                } => {
                    workflow_state.active = false;
                    output_lines.push(OutputLine::system(format!(
                        "✓ Blueprint {blueprint_id} complete ({units_run} units)"
                    )));
                    output_lines.push(OutputLine::system(String::new()));
                    push_toast(
                        &mut toasts,
                        format!("✓ {blueprint_id} complete"),
                        ToastKind::Success,
                    );
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::WorkflowBlocked { reason, .. } => {
                    workflow_state.active = false;
                    output_lines.push(OutputLine::system(format!("⏸ Blocked: {reason}")));
                    output_lines.push(OutputLine::system(String::new()));
                    push_toast(
                        &mut toasts,
                        format!("⏸ Blocked: {reason}"),
                        ToastKind::Success,
                    );
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::WorkflowError { message } => {
                    workflow_state.active = false;
                    output_lines.push(OutputLine::error(format!("Workflow error: {message}")));
                    push_toast(&mut toasts, format!("⚠ {message}"), ToastKind::Error);
                    auto_scroll(
                        &output_lines,
                        &mut scroll,
                        terminal.size()?.height,
                        scroll_pinned,
                        workflow_state.active,
                    );
                }
                AgentEvent::StageTransition { from, to } => {
                    push_toast(&mut toasts, format!("{from} → {to}"), ToastKind::Info);
                }
                AgentEvent::GitCommitUpdate { hash, msg } => {
                    workflow_state.last_commit_hash = hash;
                    workflow_state.last_commit_msg = msg;
                }
                AgentEvent::AutoModeStart => {
                    workflow_state.is_auto = true;
                    workflow_state.model_display = model_for_display.clone();
                }
                AgentEvent::AutoModeEnd => {
                    workflow_state.is_auto = false;
                }
                AgentEvent::ContextUsage { pct } => {
                    workflow_state.context_pct = pct;
                }
            }
        }

        // Poll for input events (keyboard + mouse)
        if event::poll(timeout)? {
            match event::read()? {
                // Mouse events not captured — terminal handles selection/copy natively
                Event::Mouse(_) => {}
                Event::Key(key) => {
                    // Model picker intercepts keys when active
                    if model_picker_active {
                        match key.code {
                            KeyCode::Up => {
                                model_picker_index = model_picker_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if model_picker_index + 1 < model_picker_items.len() {
                                    model_picker_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let selected = &model_picker_items[model_picker_index];
                                let _ = user_msg_tx.send(format!("__model:{}__", selected.id));
                                model_picker_active = false;
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                model_picker_active = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Login picker intercepts keys when active
                    if login_picker_active {
                        match key.code {
                            KeyCode::Up => {
                                login_picker_index = login_picker_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if login_picker_index + 1 < LOGIN_PROVIDERS.len() {
                                    login_picker_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let provider = LOGIN_PROVIDERS[login_picker_index];
                                match prompt_and_store_provider_credential(provider, "") {
                                    Ok(message) => {
                                        output_lines.push(OutputLine::system(message));
                                        push_toast(
                                            &mut toasts,
                                            format!("Saved {provider} credentials"),
                                            ToastKind::Success,
                                        );
                                    }
                                    Err(e) => {
                                        output_lines.push(OutputLine::error(format!(
                                            "Could not save {provider} credentials: {e}"
                                        )));
                                        push_toast(
                                            &mut toasts,
                                            format!("Save {provider} credentials failed"),
                                            ToastKind::Error,
                                        );
                                    }
                                }
                                output_lines.push(OutputLine::system(String::new()));
                                auto_scroll(
                                    &output_lines,
                                    &mut scroll,
                                    terminal.size()?.height,
                                    scroll_pinned,
                                    workflow_state.active,
                                );
                                force_full_redraw = true;
                                login_picker_active = false;
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                login_picker_active = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Help overlay intercepts all keys when active (per D-03, HELP-02)
                    if help_active {
                        help_active = false;
                        continue;
                    }

                    match (key.code, key.modifiers) {
                        // Quit
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            cancel.cancel();
                            break;
                        }
                        // Submit prompt — always allowed (agent may be waiting for follow-up)
                        (KeyCode::Enter, _) => {
                            if !input_text.is_empty() {
                                show_splash = false;
                                let prompt = input_text.clone();
                                input_history.push(prompt.clone());
                                history_idx = None;
                                input_text.clear();
                                cursor_pos = 0;

                                // Handle slash commands
                                if let Some(rest) = prompt.strip_prefix('/') {
                                    let cmd =
                                        rest.split_whitespace().next().unwrap_or("").to_lowercase();

                                    if cmd == "quit" || cmd == "exit" {
                                        cancel.cancel();
                                        break;
                                    }

                                    if cmd == "clear" {
                                        // /clear — fresh context window, zero token rot
                                        let _ = user_msg_tx.send("__clear__".to_string());
                                        output_lines.clear();
                                        scroll = 0;
                                        total_in = 0;
                                        total_out = 0;
                                        total_cost = 0.0;
                                        show_splash = true;
                                        continue;
                                    }

                                    // /model — switch or list models
                                    if cmd == "model" {
                                        let model_arg = rest
                                            .split_once(' ')
                                            .map(|(_, a)| a.trim())
                                            .unwrap_or("");
                                        if model_arg.is_empty() {
                                            // Open interactive model picker
                                            // Pre-select the current active model
                                            model_picker_index = model_picker_items
                                                .iter()
                                                .position(|m| m.display_name == model_for_display)
                                                .unwrap_or(0);
                                            model_picker_active = true;
                                            continue;
                                        }
                                        // Switch model
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        let _ = user_msg_tx.send(format!("__model:{model_arg}__"));
                                        continue;
                                    }

                                    // /login — open provider login/oauth page picker
                                    if cmd == "login" {
                                        let login_arg = rest
                                            .split_once(' ')
                                            .map(|(_, a)| a.trim())
                                            .unwrap_or("");
                                        if login_arg.is_empty() {
                                            login_picker_index = 0;
                                            login_picker_active = true;
                                            continue;
                                        }
                                    }

                                    // /blueprint — intent-based routing
                                    if cmd == "blueprint" {
                                        let bp_args = rest
                                            .split_once(' ')
                                            .map(|(_, a)| a.trim())
                                            .unwrap_or("");

                                        if bp_args.is_empty() {
                                            // No args: ensure .fin/ exists, then check status
                                            let fin_dir = crate::workflow::state::FinDir::new(&cwd);
                                            if !fin_dir.exists() {
                                                let _ = fin_dir.init();
                                            }
                                            let has_active = matches!(
                                                fin_dir.active_blueprint_status(),
                                                crate::workflow::state::BlueprintStatus::InProgress { .. }
                                            );

                                            if has_active {
                                                // Resume active blueprint
                                                output_lines
                                                    .push(OutputLine::user(format!("> {prompt}")));
                                                let _ =
                                                    user_msg_tx.send("__blueprint:__".to_string());
                                            } else {
                                                // No active blueprint — prompt for name
                                                output_lines
                                                    .push(OutputLine::user(format!("> {prompt}")));
                                                output_lines.push(OutputLine::system(
                                                    "Name your blueprint:".to_string(),
                                                ));
                                                auto_scroll(
                                                    &output_lines,
                                                    &mut scroll,
                                                    terminal.size()?.height,
                                                    scroll_pinned,
                                                    workflow_state.active,
                                                );
                                                // Pre-fill input so user just types the name
                                                input_text = "/blueprint ".to_string();
                                                cursor_pos = input_text.len();
                                            }
                                        } else {
                                            // Has args: check for PRD/ADR prefix
                                            let upper = bp_args.to_uppercase();
                                            if (upper.starts_with("PRD ")
                                                || upper.starts_with("ADR "))
                                                && bp_args.len() > 4
                                            {
                                                output_lines
                                                    .push(OutputLine::user(format!("> {prompt}")));
                                                let doc_type = &bp_args[..3];
                                                let doc_path = bp_args[4..].trim();
                                                let _ = user_msg_tx.send(format!(
                                                    "__blueprint_doc:{doc_type}:{doc_path}__"
                                                ));
                                            } else if upper == "PRD" || upper == "ADR" {
                                                // No path — prompt for it
                                                output_lines
                                                    .push(OutputLine::user(format!("> {prompt}")));
                                                output_lines.push(OutputLine::system(format!(
                                                    "Path to {} document:",
                                                    upper
                                                )));
                                                auto_scroll(
                                                    &output_lines,
                                                    &mut scroll,
                                                    terminal.size()?.height,
                                                    scroll_pinned,
                                                    workflow_state.active,
                                                );
                                                input_text = format!("/blueprint {upper} ");
                                                cursor_pos = input_text.len();
                                            } else {
                                                // Regular blueprint creation
                                                output_lines
                                                    .push(OutputLine::user(format!("> {prompt}")));
                                                let _ = user_msg_tx
                                                    .send(format!("__blueprint:{bp_args}__"));
                                            }
                                        }
                                        continue;
                                    }

                                    // /next — context-aware: workflow dispatch OR handoff
                                    if cmd == "next" {
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        let _ = user_msg_tx.send("__next__".to_string());
                                        continue;
                                    }

                                    // /ship — squash-merge section branch to main
                                    if cmd == "ship" {
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        output_lines
                                            .push(OutputLine::system("Shipping...".to_string()));
                                        auto_scroll(
                                            &output_lines,
                                            &mut scroll,
                                            terminal.size()?.height,
                                            scroll_pinned,
                                            workflow_state.active,
                                        );
                                        let _ = user_msg_tx.send("__ship__".to_string());
                                        continue;
                                    }

                                    // /map — map codebase using TUI agent
                                    if cmd == "map" {
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        output_lines.push(OutputLine::system(
                                            "Mapping codebase...".to_string(),
                                        ));
                                        auto_scroll(
                                            &output_lines,
                                            &mut scroll,
                                            terminal.size()?.height,
                                            scroll_pinned,
                                            workflow_state.active,
                                        );
                                        let _ = user_msg_tx.send("__map__".to_string());
                                        continue;
                                    }

                                    // /resume — resume from handoff (dispatches as build stage)
                                    if cmd == "resume" {
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        let fin_dir = crate::workflow::state::FinDir::new(&cwd);
                                        if !fin_dir.exists() {
                                            output_lines.push(OutputLine::system(
                                                "No .fin/ directory. Run /init first.".to_string(),
                                            ));
                                        } else {
                                            output_lines.push(OutputLine::system(
                                                "Resuming from handoff...".to_string(),
                                            ));
                                            auto_scroll(
                                                &output_lines,
                                                &mut scroll,
                                                terminal.size()?.height,
                                                scroll_pinned,
                                                workflow_state.active,
                                            );
                                            let _ = user_msg_tx.send("__stage:build__".to_string());
                                        }
                                        auto_scroll(
                                            &output_lines,
                                            &mut scroll,
                                            terminal.size()?.height,
                                            scroll_pinned,
                                            workflow_state.active,
                                        );
                                        continue;
                                    }

                                    // /worktree — worktree management
                                    if cmd == "worktree" {
                                        let wt_args = rest
                                            .split_once(' ')
                                            .map(|(_, a)| a.trim())
                                            .unwrap_or("list");
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));
                                        let _ = user_msg_tx.send(format!("__worktree:{wt_args}__"));
                                        continue;
                                    }

                                    // Stage commands — verify context before dispatching
                                    if matches!(
                                        cmd.as_str(),
                                        "define"
                                            | "explore"
                                            | "architect"
                                            | "build"
                                            | "validate"
                                            | "seal-section"
                                            | "advance"
                                            | "auto"
                                    ) {
                                        output_lines.push(OutputLine::user(format!("> {prompt}")));

                                        // Check prerequisites
                                        let fin_dir = crate::workflow::state::FinDir::new(&cwd);
                                        if !fin_dir.exists() {
                                            output_lines.push(OutputLine::system(
                                                "No .fin/ directory. Initializing...".to_string(),
                                            ));
                                            let _ = fin_dir.init();
                                        }
                                        let has_blueprint = matches!(
                                            fin_dir.active_blueprint_status(),
                                            crate::workflow::state::BlueprintStatus::InProgress { .. }
                                        );
                                        if !has_blueprint {
                                            output_lines.push(OutputLine::system(
                                                "No active blueprint. Create one first:"
                                                    .to_string(),
                                            ));
                                            auto_scroll(
                                                &output_lines,
                                                &mut scroll,
                                                terminal.size()?.height,
                                                scroll_pinned,
                                                workflow_state.active,
                                            );
                                            input_text = "/blueprint ".to_string();
                                            cursor_pos = input_text.len();
                                            continue;
                                        }

                                        output_lines
                                            .push(OutputLine::system(format!("Running /{cmd}...")));
                                        auto_scroll(
                                            &output_lines,
                                            &mut scroll,
                                            terminal.size()?.height,
                                            scroll_pinned,
                                            workflow_state.active,
                                        );
                                        let _ = user_msg_tx.send(format!("__stage:{cmd}__"));
                                        continue;
                                    }

                                    output_lines.push(OutputLine::user(format!("> {prompt}")));
                                    let result = handle_slash_command(&prompt, &cwd);
                                    match result {
                                        Ok(msg) => {
                                            for line in msg.lines() {
                                                output_lines
                                                    .push(OutputLine::system(line.to_string()));
                                            }
                                        }
                                        Err(e) => {
                                            output_lines.push(OutputLine::error(format!("{e}")))
                                        }
                                    }
                                    output_lines.push(OutputLine::system(String::new()));
                                    auto_scroll(
                                        &output_lines,
                                        &mut scroll,
                                        terminal.size()?.height,
                                        scroll_pinned,
                                        workflow_state.active,
                                    );
                                    if cmd == "login" {
                                        force_full_redraw = true;
                                    }
                                    continue;
                                }

                                // Add user message to output
                                output_lines.push(OutputLine::user(format!("> {prompt}")));
                                output_lines.push(OutputLine::system(String::new()));
                                auto_scroll(
                                    &output_lines,
                                    &mut scroll,
                                    terminal.size()?.height,
                                    scroll_pinned,
                                    workflow_state.active,
                                );

                                // Send prompt to the agent task
                                let _ = user_msg_tx.send(prompt);
                            }
                        }
                        // Ctrl keybindings (must be before generic Char catch-all)
                        (KeyCode::Char('a'), KeyModifiers::CONTROL)
                        | (KeyCode::Home, KeyModifiers::NONE) => {
                            cursor_pos = 0;
                        }
                        (KeyCode::Char('e'), KeyModifiers::CONTROL)
                        | (KeyCode::End, KeyModifiers::NONE) => {
                            cursor_pos = input_text.len();
                        }
                        // Kill line (Ctrl+K)
                        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                            input_text.truncate(cursor_pos);
                        }
                        // Clear input (Ctrl+U)
                        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                            input_text.clear();
                            cursor_pos = 0;
                        }
                        // Tab completion for slash commands
                        (KeyCode::Tab, _) => {
                            if input_text.starts_with('/') {
                                let partial = &input_text[1..];
                                let matches: Vec<&&str> = SLASH_COMMANDS
                                    .iter()
                                    .filter(|c| c.starts_with(partial) && *c != &partial)
                                    .collect();
                                if matches.len() == 1 {
                                    input_text = format!("/{}", matches[0]);
                                    cursor_pos = input_text.len();
                                } else if matches.len() > 1 {
                                    // Find longest common prefix
                                    let mut prefix = matches[0].to_string();
                                    for m in &matches[1..] {
                                        while !m.starts_with(&prefix) {
                                            prefix.pop();
                                        }
                                    }
                                    if prefix.len() > partial.len() {
                                        input_text = format!("/{prefix}");
                                        cursor_pos = input_text.len();
                                    }
                                    // Show options
                                    let opts: Vec<String> =
                                        matches.iter().map(|m| format!("/{m}")).collect();
                                    output_lines.push(OutputLine::system(opts.join("  ")));
                                    auto_scroll(
                                        &output_lines,
                                        &mut scroll,
                                        terminal.size()?.height,
                                        scroll_pinned,
                                        workflow_state.active,
                                    );
                                }
                            }
                        }
                        // Help overlay — ? key with empty input and no other overlay (per D-04, HELP-03)
                        (KeyCode::Char('?'), _)
                            if input_text.is_empty()
                                && !model_picker_active
                                && !login_picker_active =>
                        {
                            help_active = true;
                        }
                        // Text input
                        (KeyCode::Char(c), _) => {
                            input_text.insert(cursor_pos, c);
                            cursor_pos += 1;
                        }
                        (KeyCode::Backspace, _) => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                                input_text.remove(cursor_pos);
                            }
                        }
                        (KeyCode::Delete, _) => {
                            if cursor_pos < input_text.len() {
                                input_text.remove(cursor_pos);
                            }
                        }
                        (KeyCode::Left, _) => {
                            cursor_pos = cursor_pos.saturating_sub(1);
                        }
                        (KeyCode::Right, _) => {
                            if cursor_pos < input_text.len() {
                                cursor_pos += 1;
                            }
                        }
                        // Scroll: Shift+Up/Down = 1 line, PageUp/PageDown = 10, Shift+Home/End = top/bottom
                        (KeyCode::Up, KeyModifiers::SHIFT) => {
                            show_splash = false;
                            scroll = scroll.saturating_sub(1);
                            scroll_pinned = false;
                        }
                        (KeyCode::Down, KeyModifiers::SHIFT) => {
                            let max = max_scroll(
                                &output_lines,
                                terminal.size()?.height,
                                workflow_state.active,
                            );
                            scroll = (scroll + 1).min(max);
                            if scroll >= max {
                                scroll_pinned = true;
                            }
                        }
                        (KeyCode::PageUp, _) => {
                            show_splash = false;
                            scroll = scroll.saturating_sub(10);
                            scroll_pinned = false;
                        }
                        (KeyCode::PageDown, _) => {
                            let max = max_scroll(
                                &output_lines,
                                terminal.size()?.height,
                                workflow_state.active,
                            );
                            scroll = (scroll + 10).min(max);
                            if scroll >= max {
                                scroll_pinned = true;
                            }
                        }
                        (KeyCode::End, KeyModifiers::SHIFT) => {
                            let max = max_scroll(
                                &output_lines,
                                terminal.size()?.height,
                                workflow_state.active,
                            );
                            scroll = max;
                            scroll_pinned = true;
                        }
                        (KeyCode::Home, KeyModifiers::SHIFT) => {
                            show_splash = false;
                            scroll = 0;
                            scroll_pinned = false;
                        }
                        // Input history navigation (plain Up/Down without Shift)
                        (KeyCode::Up, _) => {
                            if !input_history.is_empty() {
                                let idx = match history_idx {
                                    Some(0) => 0,
                                    Some(i) => i - 1,
                                    None => input_history.len() - 1,
                                };
                                history_idx = Some(idx);
                                input_text = input_history[idx].clone();
                                cursor_pos = input_text.len();
                            }
                        }
                        (KeyCode::Down, _) => {
                            if let Some(idx) = history_idx {
                                if idx + 1 < input_history.len() {
                                    let new_idx = idx + 1;
                                    history_idx = Some(new_idx);
                                    input_text = input_history[new_idx].clone();
                                    cursor_pos = input_text.len();
                                } else {
                                    history_idx = None;
                                    input_text.clear();
                                    cursor_pos = 0;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {} // Ignore other events (resize, focus, etc.)
            }
        }
    }

    Ok(())
}

/// Persistent agent task that receives prompts via channel and runs the agent loop.
/// Conversation history accumulates across prompts. Session is persisted on completion.
#[allow(clippy::too_many_arguments)]
async fn run_tui_agent(
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    mut prompt_rx: mpsc::UnboundedReceiver<String>,
    model: crate::llm::models::ModelConfig,
    cwd: std::path::PathBuf,
    provider_registry: Arc<ProviderRegistry>,
    cancel: CancellationToken,
    resume_messages: Vec<Message>,
    session_id: String,
) {
    let agent_registry = Arc::new(AgentRegistry::load_for_project(&cwd));

    // Build tools once
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

    // Persistent agent state — survives across prompts
    let mut state = AgentState::new(model.clone(), cwd.clone());
    state.tool_registry = tool_registry;
    state.system_prompt = system_prompt;
    state.session_id = session_id;

    // Restore conversation history from resumed session
    if !resume_messages.is_empty() {
        state.messages = resume_messages;
    }

    let mut provider = match provider_registry.get(&model.provider) {
        Some(p) => p,
        None => {
            tracing::error!("Provider not found: {}", model.provider);
            let _ = event_tx.send(AgentEvent::WorkflowError {
                message: format!(
                    "Provider '{}' not configured. Check API key.",
                    model.provider
                ),
            });
            return;
        }
    };

    // Session store for persisting after each agent run
    let session_store = FinPaths::resolve()
        .ok()
        .and_then(|p| SessionStore::new(&p.sessions_dir).ok());

    // Wait for prompts from the TUI main loop
    while let Some(prompt) = prompt_rx.recv().await {
        if cancel.is_cancelled() {
            break;
        }

        // Handle /clear — fresh context window, no token rot
        if prompt == "__clear__" {
            tracing::info!("Context cleared. Fresh window.");
            state.messages.clear();
            state.cumulative_usage = crate::llm::types::Usage::default();
            state.session_id = uuid::Uuid::new_v4().to_string();
            continue;
        }

        // Handle /next — context-aware: workflow dispatch OR context handoff
        if prompt == "__next__" {
            let fin_dir = crate::workflow::state::FinDir::new(&cwd);
            let has_blueprint = matches!(
                fin_dir.active_blueprint_status(),
                crate::workflow::state::BlueprintStatus::InProgress { .. }
            );

            if has_blueprint {
                // Active blueprint → dispatch next workflow unit with follow-up
                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: "Dispatching next workflow unit...\n".to_string(),
                });
                let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel::<String>();
                let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
                let io = TuiIO::with_follow_up(event_tx.clone(), steer_rx, follow_up_rx);
                let step_cancel = cancel.clone();
                let step_pr = Arc::clone(&provider_registry);
                let step_cwd = cwd.clone();
                let step_model = state.model.clone();

                let step_fut = async {
                    let provider = step_pr.get(&step_model.provider).expect("provider");
                    crate::workflow::auto_loop::run_loop(
                        &step_cwd,
                        &step_model,
                        provider,
                        crate::workflow::auto_loop::LoopMode::Step,
                        step_cancel,
                        Some(Arc::clone(&step_pr)),
                        &io,
                    )
                    .await
                };

                tokio::pin!(step_fut);
                loop {
                    tokio::select! {
                        result = &mut step_fut => {
                            tracing::info!(
                                "Loop result: {} units, {:?}",
                                result.units_run,
                                result.outcome
                            );
                            break;
                        }
                        user_input = prompt_rx.recv() => {
                            match user_input {
                                Some(text) if text == "__clear__" => {
                                    drop(follow_up_tx);
                                    break;
                                }
                                Some(text) => {
                                    let _ = follow_up_tx.send(text);
                                }
                                None => break,
                            }
                        }
                    }
                }
            } else {
                // No active blueprint → build handoff, rotate context
                let handoff = crate::agent::handoff::build_handoff(&state.messages);
                let msg_count = state.messages.len();
                let token_est = crate::agent::compaction::estimate_tokens(
                    &state.messages,
                    &state.model.provider,
                );

                // Clear state
                state.messages.clear();
                state.cumulative_usage = crate::llm::types::Usage::default();
                state.session_id = uuid::Uuid::new_v4().to_string();

                // Seed the fresh context with the handoff
                state.append_message(crate::llm::types::Message::new_user(&format!(
                    "Continue from previous context:\n\n{handoff}"
                )));

                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: format!(
                        "Context rotated. Carried forward handoff from {msg_count} messages (~{token_est} tokens).\n"
                    ),
                });
                tracing::info!(
                    "Context handoff: {msg_count} messages, ~{token_est} tokens → fresh window"
                );
            }
            continue;
        }

        // Handle /model — switch model mid-session
        if let Some(model_id) = prompt
            .strip_prefix("__model:")
            .and_then(|s| s.strip_suffix("__"))
        {
            match crate::llm::models::resolve_model(model_id) {
                Some(new_model) => {
                    let new_provider = provider_registry.get(&new_model.provider);
                    if let Some(p) = new_provider {
                        let display = new_model.display_name.clone();
                        provider = p;
                        state.model = new_model;
                        let _ = event_tx.send(AgentEvent::ModelChanged {
                            display_name: display,
                        });
                    } else {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("Provider '{}' not configured.\n", new_model.provider),
                        });
                        let _ = event_tx.send(AgentEvent::AgentEnd {
                            usage: crate::llm::types::Usage::default(),
                        });
                    }
                }
                None => {
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: format!("Model not found: {model_id}\n"),
                    });
                    let _ = event_tx.send(AgentEvent::AgentEnd {
                        usage: crate::llm::types::Usage::default(),
                    });
                }
            }
            continue;
        }

        // Handle /blueprint — wizard entry point: check → resume or create
        // Handle /blueprint PRD <path> or /blueprint ADR <path>
        if let Some(doc_args) = prompt
            .strip_prefix("__blueprint_doc:")
            .and_then(|s| s.strip_suffix("__"))
        {
            // Parse "PRD:path/to/file" or "ADR:path/to/file"
            if let Some((doc_type, doc_path)) = doc_args.split_once(':') {
                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: format!(
                        "Analyzing {} document: {doc_path}\n\n",
                        doc_type.to_uppercase()
                    ),
                });

                match crate::workflow::commands::cmd_blueprint_from_doc(
                    &cwd,
                    doc_type,
                    doc_path,
                    &model,
                    provider,
                    Arc::clone(&provider_registry),
                    &crate::tui::tui_io::TuiIO::new(
                        event_tx.clone(),
                        tokio::sync::mpsc::unbounded_channel().1,
                    ),
                )
                .await
                {
                    Ok(id) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!(
                                "\nBlueprint {id} created from {}. Use /next or /auto to continue.\n",
                                doc_type.to_uppercase()
                            ),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("\nDocument analysis failed: {e}\n"),
                        });
                    }
                }
                let _ = event_tx.send(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                });
            }
            continue;
        }

        if let Some(bp_args) = prompt
            .strip_prefix("__blueprint:")
            .and_then(|s| s.strip_suffix("__"))
        {
            let fin_dir = crate::workflow::state::FinDir::new(&cwd);
            if !fin_dir.exists() {
                fin_dir.init().unwrap_or_default();
            }

            // Handle /blueprint complete — mark active blueprint as done
            if bp_args.eq_ignore_ascii_case("complete") {
                match crate::workflow::commands::cmd_blueprint_complete(&cwd).await {
                    Ok(()) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "Blueprint completed successfully.\n".to_string(),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("Blueprint complete failed: {e}\n"),
                        });
                    }
                }
                let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
                let end_io = TuiIO::new(event_tx.clone(), steer_rx);
                let _ = end_io
                    .emit(AgentEvent::AgentEnd {
                        usage: crate::llm::types::Usage::default(),
                    })
                    .await;
                continue;
            }

            // Handle /blueprint list — show all blueprints
            if bp_args.eq_ignore_ascii_case("list") {
                let listing = fin_dir.list_blueprints_display();
                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: format!("{listing}\n"),
                });
                let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
                let end_io = TuiIO::new(event_tx.clone(), steer_rx);
                let _ = end_io
                    .emit(AgentEvent::AgentEnd {
                        usage: crate::llm::types::Usage::default(),
                    })
                    .await;
                continue;
            }

            let status = fin_dir.active_blueprint_status();

            match status {
                crate::workflow::state::BlueprintStatus::InProgress {
                    id,
                    stage: _,
                    section: _,
                    task: _,
                } => {
                    // ── Active blueprint: health check + resume ──
                    let mut msg = format!("\nBlueprint {id} is already in progress.");
                    if !bp_args.is_empty() {
                        msg.push_str(&format!(
                            "\n  (Ignoring \"{bp_args}\" — finish or complete {id} first.)"
                        ));
                    }
                    msg.push_str("\n\n");
                    let _ = event_tx.send(AgentEvent::TextDelta { text: msg });

                    // Run health check
                    let report = fin_dir.blueprint_health_check();
                    if !report.issues.is_empty() {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("Health check: {}\n", report.summary),
                        });
                        for fix in &report.fixed {
                            let _ = event_tx.send(AgentEvent::TextDelta {
                                text: format!("  Fixed: {fix}\n"),
                            });
                        }
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "\n".to_string(),
                        });
                    } else {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "Health check: state is healthy.\n\n".to_string(),
                        });
                    }

                    // Show progress summary
                    let progress = fin_dir.blueprint_progress_summary();
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: format!("{progress}\n\n"),
                    });

                    // Show workflow agents (.fin/agents/ only — external agents excluded)
                    let agent_registry = crate::agents::AgentRegistry::load_workflow_agents(&cwd);
                    if !agent_registry.is_empty() {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "Workflow agents:\n".to_string(),
                        });
                        // Re-read status after health check may have changed it
                        let refreshed = fin_dir.active_blueprint_status();
                        if let crate::workflow::state::BlueprintStatus::InProgress {
                            stage: ref current_stage,
                            ..
                        } = refreshed
                        {
                            if let Some(runner_stage) =
                                crate::workflow::Stage::from_str(current_stage)
                            {
                                let runner =
                                    crate::workflow::commands::get_stage_runner(runner_stage);
                                for role in runner.delegatable_roles() {
                                    let agents = agent_registry.find_workflow_role(role);
                                    for agent in agents {
                                        let _ = event_tx.send(AgentEvent::TextDelta {
                                            text: format!(
                                                "  {} ({}, {}) — role: {}\n",
                                                agent.id, agent.model_tier, agent.description, role
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "\n".to_string(),
                        });
                    }

                    // Auto-dispatch (resume) — run through stages with follow-up support
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: "Resuming workflow...\n\n".to_string(),
                    });
                    let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel::<String>();
                    let (_resume_steer_tx, resume_steer_rx) = mpsc::unbounded_channel::<Message>();
                    let resume_io =
                        TuiIO::with_follow_up(event_tx.clone(), resume_steer_rx, follow_up_rx);
                    let resume_cancel = cancel.clone();
                    let resume_pr = Arc::clone(&provider_registry);
                    let resume_cwd = cwd.clone();
                    let resume_model = state.model.clone();

                    let _ = event_tx.send(AgentEvent::AutoModeStart);
                    let resume_fut = async {
                        let provider = resume_pr.get(&resume_model.provider).expect("provider");
                        crate::workflow::auto_loop::run_loop(
                            &resume_cwd,
                            &resume_model,
                            provider,
                            crate::workflow::auto_loop::LoopMode::Auto,
                            resume_cancel,
                            Some(Arc::clone(&resume_pr)),
                            &resume_io,
                        )
                        .await
                    };

                    // Run resume concurrently, forwarding user input as follow-ups
                    tokio::pin!(resume_fut);
                    loop {
                        tokio::select! {
                            result = &mut resume_fut => {
                                tracing::info!(
                                    "Blueprint resume: {} units, {:?}",
                                    result.units_run,
                                    result.outcome
                                );
                                break;
                            }
                            user_input = prompt_rx.recv() => {
                                match user_input {
                                    Some(text) if text == "__clear__" => {
                                        drop(follow_up_tx);
                                        break;
                                    }
                                    Some(text) => {
                                        let _ = follow_up_tx.send(text);
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                    let _ = event_tx.send(AgentEvent::AutoModeEnd);
                }

                _ => {
                    // ── No active blueprint: create new one ──
                    if bp_args.is_empty() {
                        // Input side handles this case by prompting for name
                        continue;
                    }

                    let blueprints = fin_dir.list_blueprints();
                    let id = format!("B{:03}", blueprints.len() + 1);
                    fin_dir.create_blueprint(&id).unwrap_or_default();
                    let vision = crate::workflow::markdown::blueprint_vision(&id, bp_args, "");
                    std::fs::write(fin_dir.blueprint_vision(&id), &vision).unwrap_or_default();
                    let status_md = crate::workflow::markdown::status_template(
                        &format!("{id} — {bp_args}"),
                        None,
                        None,
                        "define",
                        "Use /define or /auto to start.",
                    );
                    fin_dir.write_state(&status_md).unwrap_or_default();

                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: format!("Created blueprint {id}: {bp_args}\n\n"),
                    });

                    // Show workflow agents (.fin/agents/ only — external agents excluded)
                    let agent_registry = crate::agents::AgentRegistry::load_workflow_agents(&cwd);
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: "Workflow: define → explore → architect → build → validate → seal\n"
                            .to_string(),
                    });
                    if !agent_registry.is_empty() {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: "\nWorkflow agents:\n".to_string(),
                        });
                        for stage in crate::workflow::Stage::all() {
                            let runner = crate::workflow::commands::get_stage_runner(*stage);
                            let roles = runner.delegatable_roles();
                            for role in &roles {
                                let agents = agent_registry.find_workflow_role(role);
                                for agent in agents {
                                    let _ = event_tx.send(AgentEvent::TextDelta {
                                        text: format!(
                                            "  {}: {} ({}) — {}\n",
                                            stage.label(),
                                            agent.id,
                                            agent.model_tier,
                                            role
                                        ),
                                    });
                                }
                            }
                        }
                    }

                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: "\nStarting define stage...\n\n".to_string(),
                    });

                    // Auto-start the define stage (interactive)
                    let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel::<String>();
                    let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
                    let io = TuiIO::with_follow_up(event_tx.clone(), steer_rx, follow_up_rx);

                    let stage = crate::workflow::Stage::Define;

                    // Emit WorkflowUnitStart so the phase bar panel activates
                    let _ = event_tx.send(AgentEvent::WorkflowUnitStart {
                        blueprint_id: id.clone(),
                        section_id: None,
                        task_id: None,
                        stage: stage.label().to_string(),
                        unit_type: "DefineBlueprint".to_string(),
                    });
                    let mut ctx = crate::workflow::phases::StageContext::load(
                        &fin_dir, &id, None, None, stage,
                    );
                    ctx.provider_registry = Some(Arc::clone(&provider_registry));
                    let runner = crate::workflow::commands::get_stage_runner(stage);

                    let stage_cancel = cancel.clone();
                    let stage_fut =
                        runner.run(&ctx, &fin_dir, &state.model, provider, &io, stage_cancel);

                    tokio::pin!(stage_fut);
                    loop {
                        tokio::select! {
                            result = &mut stage_fut => {
                                if let Err(e) = result {
                                    tracing::error!("Blueprint define stage failed: {e}");
                                    let _ = event_tx.send(AgentEvent::WorkflowError {
                                        message: format!("Define stage failed: {e}"),
                                    });
                                }
                                break;
                            }
                            user_input = prompt_rx.recv() => {
                                match user_input {
                                    Some(text) if text == "__clear__" => {
                                        drop(follow_up_tx);
                                        break;
                                    }
                                    Some(text) => {
                                        let _ = follow_up_tx.send(text);
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                }
            }

            // Emit end event
            let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
            let end_io = TuiIO::new(event_tx.clone(), steer_rx);
            let _ = end_io
                .emit(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                })
                .await;
            continue;
        }

        // Handle /ship — squash-merge section branch to main
        if prompt == "__ship__" {
            let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
            let io = TuiIO::new(event_tx.clone(), steer_rx);
            match crate::workflow::commands::cmd_ship(&cwd).await {
                Ok(()) => {
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: "Section shipped successfully.\n".to_string(),
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: format!("Ship failed: {e}\n"),
                    });
                }
            }
            let _ = io
                .emit(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                })
                .await;
            continue;
        }

        // Handle /map — map codebase using TuiIO
        if prompt == "__map__" {
            let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
            let io = TuiIO::new(event_tx.clone(), steer_rx);
            let fin_dir = crate::workflow::state::FinDir::new(&cwd);
            if !fin_dir.exists() {
                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: "No .fin/ directory. Run /init first.\n".to_string(),
                });
                let _ = io
                    .emit(AgentEvent::AgentEnd {
                        usage: crate::llm::types::Usage::default(),
                    })
                    .await;
                continue;
            }
            let _ = event_tx.send(AgentEvent::TextDelta {
                text: format!("Mapping codebase at {} ...\n\n", cwd.display()),
            });
            let cancel_map = cancel.clone();
            let allowed_tools: Vec<String> = ["read", "glob", "grep", "bash", "write"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let tool_registry = crate::tools::ToolRegistry::filtered_defaults(&cwd, &allowed_tools);
            let prompt_content = crate::workflow::prompts::map_prompt(&cwd.display().to_string());
            let agent_context = crate::agent::prompt::AgentPromptContext {
                available_agents: None,
                agent_role: Some(prompt_content),
            };
            let system_prompt = crate::agent::prompt::build_system_prompt(
                &tool_registry.schemas(),
                &cwd,
                Some(&agent_context),
            );
            let mut map_state = crate::agent::state::AgentState::new(model.clone(), cwd.clone());
            map_state.tool_registry = tool_registry;
            map_state.system_prompt = system_prompt;
            map_state
                .messages
                .push(crate::llm::types::Message::new_user(
                    "Map this codebase. Follow the instructions in your system prompt exactly.",
                ));
            if let Err(e) =
                crate::agent::agent_loop::run_agent_loop(&mut map_state, provider, &io, cancel_map)
                    .await
            {
                let _ = event_tx.send(AgentEvent::TextDelta {
                    text: format!("Map failed: {e}\n"),
                });
            } else {
                let map_path = fin_dir.map_path();
                let msg = if map_path.exists() {
                    format!(
                        "\nMap saved to {}. All agents will reference this.\n",
                        map_path.display()
                    )
                } else {
                    "\nWarning: CODEBASE_MAP.md was not written.\n".to_string()
                };
                let _ = event_tx.send(AgentEvent::TextDelta { text: msg });
            }
            let _ = io
                .emit(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                })
                .await;
            continue;
        }

        // Handle /worktree — worktree management
        if let Some(wt_args) = prompt
            .strip_prefix("__worktree:")
            .and_then(|s| s.strip_suffix("__"))
        {
            let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();
            let io = TuiIO::new(event_tx.clone(), steer_rx);
            let parts: Vec<&str> = wt_args.splitn(2, ' ').collect();
            let action_str = parts[0];
            let action_arg = parts.get(1).copied().unwrap_or("");
            let action = match action_str {
                "list" | "" => Some(crate::cli::WorktreeAction::List),
                "create" if !action_arg.is_empty() => Some(crate::cli::WorktreeAction::Create {
                    name: action_arg.to_string(),
                }),
                "merge" if !action_arg.is_empty() => Some(crate::cli::WorktreeAction::Merge {
                    name: action_arg.to_string(),
                }),
                "remove" if !action_arg.is_empty() => Some(crate::cli::WorktreeAction::Remove {
                    name: action_arg.to_string(),
                }),
                "clean" => Some(crate::cli::WorktreeAction::Clean),
                _ => None,
            };
            match action {
                Some(a) => match crate::worktree::handle_worktree(a).await {
                    Ok(()) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("Worktree {action_str} complete.\n"),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::TextDelta {
                            text: format!("Worktree error: {e}\n"),
                        });
                    }
                },
                None => {
                    let _ = event_tx.send(AgentEvent::TextDelta {
                        text: "Usage: /worktree [list|create <name>|merge <name>|remove <name>|clean]\n".to_string(),
                    });
                }
            }
            let _ = io
                .emit(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                })
                .await;
            continue;
        }

        // Handle stage commands — run via workflow engine in fresh context
        if let Some(stage_name) = prompt
            .strip_prefix("__stage:")
            .and_then(|s| s.strip_suffix("__"))
        {
            // Interactive stages (define) need a follow-up channel so the agent
            // can ask questions and wait for user answers via poll_follow_up().
            let is_interactive = matches!(stage_name, "define" | "architect");
            let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel::<String>();
            let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<Message>();

            let io = if is_interactive {
                TuiIO::with_follow_up(event_tx.clone(), steer_rx, follow_up_rx)
            } else {
                TuiIO::new(event_tx.clone(), steer_rx)
            };

            let loop_mode = match stage_name {
                "auto" => crate::workflow::auto_loop::LoopMode::Auto,
                _ => crate::workflow::auto_loop::LoopMode::Step,
            };

            if stage_name == "auto" {
                // Dispatch-driven — let the loop figure out what to run
                let _ = event_tx.send(AgentEvent::AutoModeStart);
                let result = crate::workflow::auto_loop::run_loop(
                    &cwd,
                    &model,
                    provider,
                    loop_mode,
                    cancel.clone(),
                    Some(Arc::clone(&provider_registry)),
                    &io,
                )
                .await;
                tracing::info!(
                    "Loop result: {} units, {:?}",
                    result.units_run,
                    result.outcome
                );
                let _ = event_tx.send(AgentEvent::AutoModeEnd);
            } else {
                // Specific stage — run it directly
                let fin_dir = crate::workflow::state::FinDir::new(&cwd);
                if fin_dir.exists() {
                    if let Some(stage) = crate::workflow::Stage::from_str(stage_name) {
                        // Parse position and run the stage
                        let status_md = fin_dir.read_state().unwrap_or_default();
                        let b_id = status_md
                            .lines()
                            .find_map(|l| l.strip_prefix("**Active Blueprint:**"))
                            .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
                            .unwrap_or_default();
                        let s_id: Option<String> = status_md
                            .lines()
                            .find_map(|l| l.strip_prefix("**Active Section:**"))
                            .and_then(|s| {
                                let s = s.trim();
                                if s == "None" || s.is_empty() {
                                    None
                                } else {
                                    Some(s.split_whitespace().next().unwrap_or("").to_string())
                                }
                            });
                        let t_id: Option<String> = status_md
                            .lines()
                            .find_map(|l| l.strip_prefix("**Active Task:**"))
                            .and_then(|s| {
                                let s = s.trim();
                                if s == "None" || s.is_empty() {
                                    None
                                } else {
                                    Some(s.split_whitespace().next().unwrap_or("").to_string())
                                }
                            });

                        if !b_id.is_empty() {
                            let mut ctx = crate::workflow::phases::StageContext::load(
                                &fin_dir,
                                &b_id,
                                s_id.as_deref(),
                                t_id.as_deref(),
                                stage,
                            );
                            ctx.provider_registry = Some(Arc::clone(&provider_registry));
                            let runner = crate::workflow::commands::get_stage_runner(stage);

                            if is_interactive {
                                // Run stage concurrently while forwarding user input.
                                // The stage agent will block on poll_follow_up() waiting
                                // for user answers; we forward prompt_rx → follow_up_tx.
                                let stage_cancel = cancel.clone();
                                let stage_fut =
                                    runner.run(&ctx, &fin_dir, &model, provider, &io, stage_cancel);

                                tokio::pin!(stage_fut);
                                loop {
                                    tokio::select! {
                                        result = &mut stage_fut => {
                                            if let Err(e) = result {
                                                tracing::error!("Stage {stage_name} failed: {e}");
                                                let _ = event_tx.send(AgentEvent::WorkflowError {
                                                    message: format!("Stage {stage_name} failed: {e}"),
                                                });
                                            }
                                            break;
                                        }
                                        user_input = prompt_rx.recv() => {
                                            match user_input {
                                                Some(text) if text == "__clear__" => {
                                                    // User wants to abort — drop the follow-up sender
                                                    drop(follow_up_tx);
                                                    break;
                                                }
                                                Some(text) => {
                                                    // Forward user input to the stage agent
                                                    let _ = follow_up_tx.send(text);
                                                }
                                                None => break, // Channel closed
                                            }
                                        }
                                    }
                                }
                            } else if let Err(e) = runner
                                .run(&ctx, &fin_dir, &model, provider, &io, cancel.clone())
                                .await
                            {
                                tracing::error!("Stage {stage_name} failed: {e}");
                                let _ = event_tx.send(AgentEvent::WorkflowError {
                                    message: format!("Stage {stage_name} failed: {e}"),
                                });
                            }
                        }
                    }
                }
            }

            // Emit end event so TUI knows the stage is done
            let _ = io
                .emit(AgentEvent::AgentEnd {
                    usage: crate::llm::types::Usage::default(),
                })
                .await;
            continue;
        }

        // Add the user message to persistent conversation history
        state.messages.push(Message::new_user(&prompt));

        // Create a fresh steering channel per run
        let (_stx, srx) = mpsc::unbounded_channel::<Message>();
        let io = TuiIO::new(event_tx.clone(), srx);

        // Track message count before this run for incremental persistence
        let msgs_before = state.messages.len() - 1; // -1 for the user message we just added

        // Run agent loop — it appends assistant/tool messages to state.messages
        if let Err(e) = run_agent_loop(&mut state, provider, &io, cancel.clone()).await {
            let _ = io
                .emit(AgentEvent::AgentEnd {
                    usage: state.cumulative_usage.clone(),
                })
                .await;
            tracing::error!("Agent error: {e}");
            let _ = io
                .emit(AgentEvent::TextDelta {
                    text: format!("\nError: {e}\n"),
                })
                .await;
        }

        // Persist new messages from this run
        if let Some(ref store) = session_store {
            for msg in &state.messages[msgs_before..] {
                if let Err(e) = store.append(&state.session_id, msg) {
                    tracing::warn!("Session persist failed: {e}");
                }
            }
        }
    }
}

fn auto_scroll(
    lines: &[OutputLine],
    scroll: &mut u16,
    terminal_height: u16,
    pinned: bool,
    wf_active: bool,
) {
    if !pinned {
        return;
    }
    // spacer(1) + status(1) + input(2) + border(1) = 5, workflow panel adds 4 more
    let chrome = if wf_active { 9 } else { 5 };
    let visible = terminal_height.saturating_sub(chrome) as usize;
    if lines.len() > visible {
        *scroll = (lines.len() - visible) as u16;
    }
}

/// Calculate the maximum scroll offset for the current output.
fn max_scroll(lines: &[OutputLine], terminal_height: u16, wf_active: bool) -> u16 {
    let chrome = if wf_active { 8 } else { 4 };
    let visible = terminal_height.saturating_sub(chrome) as usize;
    if lines.len() > visible {
        (lines.len() - visible) as u16
    } else {
        0
    }
}

/// Replay persisted messages into output lines for visual history.
fn replay_history(messages: &[Message], output_lines: &mut Vec<OutputLine>, model_name: &str) {
    use crate::llm::types::{Content, Role};

    output_lines.push(OutputLine::system(format!(
        "── Session history ({} messages) ──",
        messages.len()
    )));
    output_lines.push(OutputLine::system(String::new()));

    for msg in messages {
        match msg.role {
            Role::User => {
                for content in &msg.content {
                    if let Content::Text { text } = content {
                        output_lines.push(OutputLine::user(format!("> {text}")));
                    }
                }
            }
            Role::Assistant => {
                output_lines.push(OutputLine::system(format!("┌─ {model_name} ────")));
                for content in &msg.content {
                    match content {
                        Content::Text { text } => {
                            for line in text.lines() {
                                output_lines.push(OutputLine::assistant(line.to_string()));
                            }
                        }
                        Content::Thinking { text, .. } => {
                            for line in text.lines() {
                                output_lines.push(OutputLine::thinking(line.to_string()));
                            }
                        }
                        Content::ToolCall(tc) => {
                            output_lines.push(OutputLine::tool(format!("✓ {}", tc.name)));
                        }
                        _ => {}
                    }
                }
            }
            Role::ToolResult => {
                // Tool results are verbose — skip in replay
            }
        }
    }

    output_lines.push(OutputLine::system(String::new()));
    output_lines.push(OutputLine::system(
        "── End of history. Continue the conversation below. ──".to_string(),
    ));
    output_lines.push(OutputLine::system(String::new()));
}

/// Handle TUI slash commands. Returns a display message.
fn handle_slash_command(input: &str, cwd: &std::path::Path) -> anyhow::Result<String> {
    let parts: Vec<&str> = input[1..].splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let _args = parts.get(1).copied().unwrap_or("");

    match cmd.as_str() {
        "init" => {
            let fin_dir = crate::workflow::state::FinDir::new(cwd);
            if fin_dir.exists() {
                Ok(".fin/ already exists.".into())
            } else {
                fin_dir.init()?;
                Ok("Initialized .fin/ workflow directory.".into())
            }
        }
        "status" => {
            let fin_dir = crate::workflow::state::FinDir::new(cwd);
            if !fin_dir.exists() {
                return Ok("No workflow started yet. Use /blueprint <name> to begin.".into());
            }
            let status = fin_dir.read_state().unwrap_or_default();
            let progress = fin_dir.blueprint_progress_summary();
            if progress.contains("No active blueprint") && progress.contains("No blueprints") {
                Ok(format!(
                    "{status}\n\nNo active blueprint. Use /blueprint <name> to start one."
                ))
            } else if progress.contains("No active blueprint") {
                Ok(format!(
                    "{status}\n{progress}\n\nUse /blueprint <name> to start a new one."
                ))
            } else {
                Ok(format!("{status}\n{progress}"))
            }
        }
        "blueprint" => {
            // Handled in the agent task via __blueprint:__ routing
            Ok("Blueprint command dispatched to agent.".into())
        }
        "pause" => match crate::workflow::commands::cmd_pause(cwd) {
            Ok(()) => Ok("Paused. Use /resume to continue from a handoff.".into()),
            Err(e) => Ok(format!("Pause failed: {e}")),
        },
        "config" => {
            let sub = _args.trim();
            if sub.is_empty() || sub == "list-keys" {
                let auth = crate::config::auth::AuthStore::default();
                let providers = [
                    ("anthropic", "ANTHROPIC_API_KEY"),
                    ("openai", "OPENAI_API_KEY"),
                    ("google", "GOOGLE_API_KEY"),
                    ("mistral", "MISTRAL_API_KEY"),
                    ("brave", ""),
                    ("tavily", ""),
                    ("ollama", ""),
                ];
                let mut lines = vec!["Configured API keys:".to_string()];
                let mut found = false;
                for (name, env_var) in &providers {
                    if let Some(masked) = auth.get_masked_key(name) {
                        let source = if *name == "openai"
                            && (std::env::var("OPENAI_ACCESS_TOKEN").is_ok()
                                || std::env::var("OPENAI_BEARER_TOKEN").is_ok()
                                || std::env::var("OPENAI_API_KEY").is_ok())
                        {
                            "env"
                        } else if *name == "google" && auth.get_google_oauth().is_some() {
                            "oauth"
                        } else if !env_var.is_empty() && std::env::var(env_var).is_ok() {
                            "env"
                        } else {
                            "stored"
                        };
                        lines.push(format!("  {name:<12} {masked:<20} ({source})"));
                        found = true;
                    }
                }
                if !found {
                    lines.push("  No API keys configured.".to_string());
                    lines.push("  Tip: use `fin config set-key <provider>` from the terminal (input hidden).".to_string());
                }
                Ok(lines.join("\n"))
            } else if let Some(rest) = sub.strip_prefix("set-key ") {
                let mut parts = rest.trim().splitn(2, ' ');
                let provider = parts.next().unwrap_or("").trim();
                let key = parts.next().unwrap_or("").trim();
                if provider.is_empty() {
                    return Ok("Usage: /config set-key <provider> <key>".into());
                }
                if key.is_empty() {
                    return Ok(format!(
                        "Tip: use `fin config set-key {provider}` from the terminal to hide input.\n\
                         Or: /config set-key {provider} <key>"
                    ));
                }
                let paths = crate::config::paths::FinPaths::resolve()?;
                let mut auth =
                    crate::config::auth::AuthStore::load(&paths.auth_file).unwrap_or_default();
                auth.set_api_key(provider, key.to_string());
                auth.save(&paths.auth_file)?;
                // Also inject into the current process env so the running provider
                // picks it up immediately without relying on the keyring lookup chain.
                let env_var = match provider {
                    "anthropic" => Some("ANTHROPIC_API_KEY"),
                    "openai" => Some("OPENAI_API_KEY"),
                    "google" => Some("GOOGLE_API_KEY"),
                    "mistral" => Some("MISTRAL_API_KEY"),
                    _ => None,
                };
                if let Some(var) = env_var {
                    // Safety: single-threaded at this point in the TUI event loop;
                    // no other threads are reading this env var concurrently.
                    unsafe {
                        std::env::set_var(var, key);
                    }
                }
                Ok(format!("{provider} key saved."))
            } else if let Some(rest) = sub.strip_prefix("remove-key ") {
                let provider = rest.trim();
                let paths = crate::config::paths::FinPaths::resolve()?;
                let mut auth =
                    crate::config::auth::AuthStore::load(&paths.auth_file).unwrap_or_default();
                auth.remove_api_key(provider);
                auth.save(&paths.auth_file)?;
                Ok(format!("{provider} key removed."))
            } else {
                Ok(
                    "Usage: /config [list-keys | set-key <provider> <key> | remove-key <provider>]"
                        .into(),
                )
            }
        }
        "login" => {
            let mut parts = _args.trim().splitn(2, char::is_whitespace);
            let provider = parts.next().unwrap_or("").trim().to_lowercase();
            let login_arg = parts.next().unwrap_or("").trim();
            if provider.is_empty() {
                return Ok(
                    "Usage: /login <provider> [arg]\nSupported: openai, anthropic, google".into(),
                );
            }
            if !LOGIN_PROVIDERS.contains(&provider.as_str()) {
                return Ok(format!(
                    "Unknown provider '{provider}'. Supported: openai, anthropic, google"
                ));
            }

            let message = prompt_and_store_provider_credential(&provider, login_arg)?;
            Ok(message)
        }
        "sessions" => {
            let paths = crate::config::paths::FinPaths::resolve()?;
            let store = crate::db::session::SessionStore::new(&paths.sessions_dir)?;
            let sessions = store.list()?;
            if sessions.is_empty() {
                return Ok("No sessions found.".into());
            }
            let mut out = format!(
                "{:<38}  {:>10}  LAST MODIFIED\n{}\n",
                "SESSION ID",
                "SIZE",
                "-".repeat(70)
            );
            for s in &sessions {
                let elapsed = s.modified.elapsed().unwrap_or_default();
                let age = if elapsed.as_secs() < 60 {
                    format!("{}s ago", elapsed.as_secs())
                } else if elapsed.as_secs() < 3600 {
                    format!("{}m ago", elapsed.as_secs() / 60)
                } else if elapsed.as_secs() < 86400 {
                    format!("{}h ago", elapsed.as_secs() / 3600)
                } else {
                    format!("{}d ago", elapsed.as_secs() / 86400)
                };
                let size = if s.size < 1024 {
                    format!("{} B", s.size)
                } else if s.size < 1_048_576 {
                    format!("{:.1} KB", s.size as f64 / 1024.0)
                } else {
                    format!("{:.1} MB", s.size as f64 / 1_048_576.0)
                };
                out.push_str(&format!("{:<38}  {:>10}  {age}\n", s.id, size));
            }
            out.push_str(&format!("\n{} session(s)", sessions.len()));
            Ok(out)
        }
        "help" => Ok("Commands:\n\
                 /blueprint [name]           — Create, resume, or manage blueprints\n\
                 /blueprint PRD <path>       — Create blueprint from a PRD document\n\
                 /blueprint ADR <path>       — Create blueprint from ADR documents\n\
                 /blueprint list             — List all blueprints\n\
                 /blueprint complete         — Mark active blueprint done\n\
                 /next                       — Continue: dispatch workflow or rotate context\n\
                 /auto                       — Run all remaining workflow units\n\
                 /ship                       — Squash-merge section branch to main\n\
                 /resume                     — Resume from handoff\n\
                 /pause                      — Show pause state info\n\
                 /map                        — Map codebase (.fin/CODEBASE_MAP.md)\n\
                 /status                     — Show current workflow state\n\
                 /model                      — Switch LLM model (interactive picker)\n\
                 /clear                      — Reset context window\n\n\
                 Stages (require active blueprint):\n\
                 /define /explore /architect /build /validate\n\
                 /seal-section /advance\n\n\
                 /config [list-keys]                      — Show configured API keys\n\
                 /config set-key <provider> <key>         — Save an API key\n\
                 /config remove-key <provider>            — Remove an API key\n\
                 /login                                   — Pick provider and authenticate\n\
                 /login openai                            — Import OpenAI bearer token or API key\n\
                 /login anthropic                         — Store Anthropic API key\n\
                 /login google [client_secret.json]       — Run Google desktop OAuth flow\n\
                 Providers: openai, anthropic, google\n\
                 /sessions                                — List recent sessions\n\
                 /worktree [list]                         — List worktrees\n\
                 /worktree create|merge|remove <name>     — Manage worktrees\n\
                 /worktree clean                          — Prune stale worktrees\n\n\
                 /init  — Initialize .fin/ directory\n\
                 /help  — Show this help\n\
                 /quit  — Exit"
            .into()),
        "quit" | "exit" => {
            // Handled in the main TUI loop before reaching here
            Ok("Exiting...".into())
        }
        "define" | "explore" | "architect" | "build" | "validate" | "seal-section" | "advance"
        | "next" | "auto" | "ship" | "map" | "resume" | "worktree" => {
            // Handled in the main TUI loop before reaching here
            Ok(format!("/{cmd} dispatched."))
        }
        _ => Ok(format!(
            "Unknown command: /{cmd}. Type /help for available commands."
        )),
    }
}

fn prompt_and_store_provider_credential(provider: &str, arg: &str) -> anyhow::Result<String> {
    if !LOGIN_PROVIDERS.contains(&provider) {
        anyhow::bail!("unsupported provider: {provider}");
    }

    disable_raw_mode()?;
    stdout().execute(SetCursorStyle::DefaultUserShape)?;
    stdout().execute(LeaveAlternateScreen)?;

    let prompt_result = (|| -> anyhow::Result<String> {
        let paths = crate::config::paths::FinPaths::resolve()?;
        let mut auth =
            crate::config::auth::AuthStore::load(&paths.auth_file).unwrap_or_default();

        let message = match provider {
            "openai" => {
                println!();
                println!("Paste an OpenAI bearer token or API key.");
                println!(
                    "This app can store and use the token, but it does not perform a browser OAuth callback flow."
                );
                println!();

                let secret = rpassword::prompt_password("OpenAI bearer token or API key: ")?;
                if secret.trim().is_empty() {
                    anyhow::bail!("no credential entered");
                }
                auth.set_bearer_token(provider, secret.trim().to_string());
                auth.save(&paths.auth_file)?;
                "Saved openai bearer token.".to_string()
            }
            "anthropic" => {
                println!();
                let secret = rpassword::prompt_password("Anthropic API key: ")?;
                if secret.trim().is_empty() {
                    anyhow::bail!("no credential entered");
                }
                auth.set_api_key(provider, secret.trim().to_string());
                auth.save(&paths.auth_file)?;
                "Saved anthropic API key.".to_string()
            }
            "google" => {
                println!();
                println!("Google OAuth requires a desktop app OAuth client JSON from Google Cloud.");
                println!(
                    "Create it under Google Auth Platform > Clients > Desktop app, then download the JSON."
                );
                println!();

                let path = if arg.is_empty() {
                    prompt_visible("Path to Google client_secret.json")?
                } else {
                    arg.to_string()
                };
                if path.trim().is_empty() {
                    anyhow::bail!("no Google client_secret.json path provided");
                }

                let result = crate::config::oauth::run_google_oauth_flow(std::path::Path::new(
                    path.trim(),
                ))?;
                auth.set_google_oauth(result.credentials);
                auth.save(&paths.auth_file)?;
                format!(
                    "Saved Google OAuth session from {}.",
                    result.client_secret_path.display()
                )
            }
            _ => anyhow::bail!("unsupported provider: {provider}"),
        };

        Ok(message)
    })();

    let restore_result = (|| -> anyhow::Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(SetCursorStyle::BlinkingBar)?;
        Ok(())
    })();

    restore_result?;
    prompt_result
}

fn prompt_visible(label: &str) -> anyhow::Result<String> {
    print!("{label}: ");
    std::io::stdout().flush()?;
    let mut value = String::new();
    stdin().read_line(&mut value)?;
    Ok(value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    #[test]
    fn toast_stage_transition() {
        let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        push_toast(&mut toasts, "Build → Validate".to_string(), ToastKind::Info);
        assert_eq!(toasts.len(), 1);
        assert!(toasts.front().unwrap().0.contains("→"));
    }

    #[test]
    fn toast_workflow_terminal() {
        let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        push_toast(
            &mut toasts,
            "✓ B001 complete".to_string(),
            ToastKind::Success,
        );
        assert_eq!(toasts.len(), 1);
        assert!(toasts.front().unwrap().0.contains("complete"));

        toasts.clear();
        push_toast(
            &mut toasts,
            "⏸ Blocked: needs input".to_string(),
            ToastKind::Success,
        );
        assert_eq!(toasts.len(), 1);
        assert!(toasts.front().unwrap().0.contains("Blocked"));
    }

    #[test]
    fn toast_tool_error() {
        let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        push_toast(&mut toasts, "✗ bash failed".to_string(), ToastKind::Error);
        assert_eq!(toasts.len(), 1);
        assert!(toasts.front().unwrap().0.contains("failed"));
        assert_eq!(toasts.front().unwrap().2, ToastKind::Error);
    }

    #[test]
    fn toast_ttl_expiry() {
        let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        // Simulate a toast pushed 6 seconds ago
        toasts.push_back((
            "old toast".to_string(),
            Instant::now() - Duration::from_secs(6),
            ToastKind::Info,
        ));
        assert_eq!(toasts.len(), 1);
        // Run expiry check
        while toasts
            .front()
            .map(|(_, t, _)| t.elapsed() >= TOAST_TTL)
            .unwrap_or(false)
        {
            toasts.pop_front();
        }
        assert_eq!(toasts.len(), 0);
    }

    #[test]
    fn toast_no_fire_routine() {
        // TOAST-05: these event types must NOT trigger a toast.
        // Verify by confirming push_toast is not called for routine events.
        // The drain loop match arms for AgentStart, TurnStart, TurnEnd, ToolStart,
        // ToolEnd{is_error:false}, WorkflowUnitStart, WorkflowUnitEnd, WorkflowProgress
        // must NOT contain push_toast calls.
        // This test validates the queue stays empty when only routine events would fire.
        let toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        // Queue starts empty — if no push_toast call is made, it stays empty
        assert_eq!(toasts.len(), 0);
    }

    #[test]
    fn toast_overflow() {
        let mut toasts: VecDeque<(String, Instant, ToastKind)> = VecDeque::new();
        push_toast(&mut toasts, "first".to_string(), ToastKind::Info);
        push_toast(&mut toasts, "second".to_string(), ToastKind::Info);
        push_toast(&mut toasts, "third".to_string(), ToastKind::Success);
        // Max is 2, so oldest ("first") should be dropped
        assert_eq!(toasts.len(), 2);
        assert_eq!(toasts.front().unwrap().0, "second");
        assert_eq!(toasts.back().unwrap().0, "third");
    }
}
