// Fin — TUI Widget Helpers
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Splash screen info for rendering the startup banner.
pub struct SplashInfo {
    pub version: String,
    pub model_id: String,
    pub provider: String,
    pub directory: String,
    pub extensions: Vec<(String, bool)>, // (name, available)
}

/// ASCII art logo for FIN (block-style similar to GSD reference).
const FIN_LOGO: &[&str] = &[
    r" ___________ .___   _______  ",
    r" \_   _____/ |   |  \      \ ",
    r"  |    __)   |   |  /   |   \",
    r"  |     \    |   | /    |    \",
    r"  \___  /    |___| \____|__  /",
    r"      \/                   \/ ",
];

/// Render the splash screen with logo, info panel, and extension status.
pub fn render_splash(f: &mut Frame, area: Rect, info: &SplashInfo) {
    let logo_width: u16 = 32;
    let logo_height = FIN_LOGO.len() as u16;

    // Vertical centering: place splash content in the middle of the area
    let total_height = logo_height.max(6) + 3; // logo/info + separator + extensions
    let v_pad = area.height.saturating_sub(total_height) / 2;

    let v_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(total_height),
            Constraint::Min(0),
        ])
        .split(area);

    let content_area = v_layout[1];

    // Horizontal: logo | info
    let h_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(logo_width + 2),
            Constraint::Length(1), // separator
            Constraint::Min(20),
        ])
        .split(Rect {
            x: content_area.x + 1,
            y: content_area.y,
            width: content_area.width.saturating_sub(2),
            height: content_area.height,
        });

    // -- Logo --
    let logo_lines: Vec<Line> = FIN_LOGO
        .iter()
        .map(|l| Line::styled(*l, Style::default().fg(Color::DarkGray)))
        .collect();
    let logo = Paragraph::new(logo_lines);
    f.render_widget(logo, h_layout[0]);

    // -- Separator --
    let sep_lines: Vec<Line> = (0..content_area.height)
        .map(|_| Line::styled("│", Style::default().fg(Color::DarkGray)))
        .collect();
    let sep = Paragraph::new(sep_lines);
    f.render_widget(sep, h_layout[1]);

    // -- Info panel --
    let info_area = h_layout[2];
    let info_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title + version
            Constraint::Length(1), // blank
            Constraint::Length(1), // model
            Constraint::Length(1), // provider
            Constraint::Length(1), // directory
            Constraint::Length(1), // blank
            Constraint::Length(1), // extensions
            Constraint::Min(0),
        ])
        .split(info_area);

    // Title + version
    let title_line = Line::from(vec![
        Span::styled(" Fin", Style::default().fg(Color::White).bold()),
        Span::raw("  "),
        Span::styled(
            format!("v{}", info.version),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(Paragraph::new(title_line), info_layout[0]);

    // Model
    let model_line = Line::from(vec![
        Span::styled(" Model     ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(&info.model_id, Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(model_line), info_layout[2]);

    // Provider
    let provider_line = Line::from(vec![
        Span::styled(" Provider  ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(&info.provider, Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(provider_line), info_layout[3]);

    // Directory (shorten home dir)
    let home = std::env::var("HOME").unwrap_or_default();
    let dir_display = if info.directory.starts_with(&home) {
        format!("~{}", &info.directory[home.len()..])
    } else {
        info.directory.clone()
    };
    let dir_line = Line::from(vec![
        Span::styled(" Directory ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(dir_display, Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(dir_line), info_layout[4]);

    // Extensions status bar
    let mut ext_spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (name, ok)) in info.extensions.iter().enumerate() {
        if i > 0 {
            ext_spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
        }
        ext_spans.push(Span::styled(
            name.clone(),
            Style::default().fg(if *ok { Color::Green } else { Color::Red }),
        ));
        ext_spans.push(Span::styled(
            if *ok { " ✓" } else { " ✗" },
            Style::default().fg(if *ok { Color::Green } else { Color::Red }),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(ext_spans)), info_layout[6]);
}

/// Render the output area (scrollable conversation history).
pub fn render_output<'a>(lines: &[OutputLine], scroll: u16) -> Paragraph<'a> {
    let mut spans_lines: Vec<Line> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        match line.kind {
            LineKind::User => {
                // Visual separator before user messages (except first)
                if i > 0 {
                    spans_lines.push(Line::styled(
                        "─".repeat(60),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Green).bold(),
                ));
            }
            LineKind::Assistant => {
                // Parse markdown-like formatting in assistant output
                let text = &line.text;
                if text.starts_with("# ") || text.starts_with("## ") || text.starts_with("### ") {
                    // Markdown headers — bold cyan
                    spans_lines.push(Line::styled(
                        text.clone(),
                        Style::default().fg(Color::Cyan).bold(),
                    ));
                } else if text.starts_with("```") {
                    // Code fence markers — dim
                    spans_lines.push(Line::styled(
                        text.clone(),
                        Style::default().fg(Color::DarkGray),
                    ));
                } else if text.starts_with("- ") || text.starts_with("* ") {
                    // Unordered bullet lists
                    let content = text.trim_start_matches("- ").trim_start_matches("* ");
                    let spans = vec![
                        Span::styled("  • ", Style::default().fg(Color::Cyan)),
                        Span::styled(content.to_string(), Style::default().fg(Color::White)),
                    ];
                    spans_lines.push(Line::from(spans));
                } else if is_numbered_list(text) {
                    // Numbered lists (1. 2. etc.) — extract number and content
                    let dot_pos = text.find(". ").unwrap_or(0);
                    let num = &text[..dot_pos + 1];
                    let content = text[dot_pos + 2..].to_string();
                    let spans = vec![
                        Span::styled(format!("  {num} "), Style::default().fg(Color::Cyan)),
                        Span::styled(content, Style::default().fg(Color::White)),
                    ];
                    spans_lines.push(Line::from(spans));
                } else if text.ends_with('?') {
                    // Questions — make them stand out
                    spans_lines.push(Line::styled(text.clone(), Style::default().fg(Color::Cyan)));
                } else {
                    spans_lines.push(Line::styled(
                        text.clone(),
                        Style::default().fg(Color::White),
                    ));
                }
            }
            LineKind::Thinking => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::DarkGray).italic(),
                ));
            }
            LineKind::Tool => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Yellow).dim(),
                ));
            }
            LineKind::ToolResult => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Green).dim(),
                ));
            }
            LineKind::Error => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Red).bold(),
                ));
            }
            LineKind::System => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
    }

    Paragraph::new(spans_lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

/// Render the input area.
pub fn render_input<'a>(text: &str, model_name: &str) -> Paragraph<'a> {
    let prompt = format!("[{model_name}] > {text}");
    Paragraph::new(prompt)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
}

/// Render the status bar.
pub fn render_status_bar<'a>(
    model: &str,
    tokens_in: u64,
    tokens_out: u64,
    cost: f64,
    is_streaming: bool,
    scrolled_up: bool,
    workflow: Option<&WorkflowState>,
) -> Paragraph<'a> {
    let scroll_indicator = if scrolled_up { " [SCROLLED] " } else { "" };
    let wf_indicator = workflow
        .filter(|w| w.active)
        .map(|w| {
            let pos = match (&w.current_section, &w.current_task) {
                (Some(s), Some(t)) => {
                    format!(" {} {} {}/{}", w.blueprint_id, w.current_stage, s, t)
                }
                (Some(s), None) => format!(" {} {} {}", w.blueprint_id, w.current_stage, s),
                _ => format!(" {} {}", w.blueprint_id, w.current_stage),
            };
            format!(" |{pos}")
        })
        .unwrap_or_default();
    let status = if is_streaming {
        format!(
            " {model} | streaming...{scroll_indicator}{wf_indicator} | in:{tokens_in} out:{tokens_out} | ${cost:.4}"
        )
    } else {
        format!(
            " {model} | ready{scroll_indicator}{wf_indicator} | in:{tokens_in} out:{tokens_out} | ${cost:.4}"
        )
    };

    Paragraph::new(status).style(Style::default().fg(Color::White).bg(Color::DarkGray))
}

// ── Workflow progress panel ──────────────────────────────────────────

/// Status of a stage in the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum StageStatus {
    Done,
    Active,
    Pending,
}

/// Live workflow state for the progress panel.
#[derive(Debug, Clone)]
pub struct WorkflowState {
    pub active: bool,
    pub blueprint_id: String,
    pub current_stage: String,
    pub current_section: Option<String>,
    pub current_task: Option<String>,
    pub sections_total: u32,
    pub sections_done: u32,
    pub tasks_total: u32,
    pub tasks_done: u32,
    pub stage_pipeline: Vec<(String, StageStatus)>,
    pub is_auto: bool,
    pub model_display: String,
    pub last_commit_hash: String,
    pub last_commit_msg: String,
    pub context_pct: u8,
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self {
            active: false,
            blueprint_id: String::new(),
            current_stage: String::new(),
            current_section: None,
            current_task: None,
            sections_total: 0,
            sections_done: 0,
            tasks_total: 0,
            tasks_done: 0,
            stage_pipeline: vec![
                ("Define".into(), StageStatus::Pending),
                ("Explore".into(), StageStatus::Pending),
                ("Architect".into(), StageStatus::Pending),
                ("Build".into(), StageStatus::Pending),
                ("Validate".into(), StageStatus::Pending),
                ("Seal".into(), StageStatus::Pending),
            ],
            is_auto: false,
            model_display: String::new(),
            last_commit_hash: String::new(),
            last_commit_msg: String::new(),
            context_pct: 0,
        }
    }
}

impl WorkflowState {
    /// Update the pipeline status indicators based on the current stage.
    pub fn update_pipeline(&mut self) {
        let stage_order = [
            "define",
            "explore",
            "architect",
            "build",
            "validate",
            "seal-section",
        ];
        let current_idx = stage_order
            .iter()
            .position(|s| *s == self.current_stage)
            .unwrap_or(0);

        for (i, (_, status)) in self.stage_pipeline.iter_mut().enumerate() {
            if i < current_idx {
                *status = StageStatus::Done;
            } else if i == current_idx {
                *status = StageStatus::Active;
            } else {
                *status = StageStatus::Pending;
            }
        }
    }
}

/// Render the workflow progress panel (3 lines).
pub fn render_workflow_panel(f: &mut Frame, area: Rect, state: &WorkflowState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", state.blueprint_id),
            Style::default().fg(Color::White).bold(),
        ))
        .title(Span::styled(
            format!(" {} ", capitalize(&state.current_stage)),
            Style::default().fg(Color::Cyan),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    // Line 1: Stage pipeline
    let mut pipeline_spans: Vec<Span> = Vec::new();
    for (i, (name, status)) in state.stage_pipeline.iter().enumerate() {
        if i > 0 {
            pipeline_spans.push(Span::styled("  ", Style::default()));
        }
        let (icon, color) = match status {
            StageStatus::Done => ("✓", Color::Green),
            StageStatus::Active => ("●", Color::Cyan),
            StageStatus::Pending => ("○", Color::DarkGray),
        };
        pipeline_spans.push(Span::styled(
            format!("{name} {icon}"),
            Style::default().fg(color),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(pipeline_spans)), layout[0]);

    // Line 2: Progress bar + counts
    let (label, done, total) = if state.tasks_total > 0 {
        let sec = state.current_section.as_deref().unwrap_or("");
        (format!("{sec}: "), state.tasks_done, state.tasks_total)
    } else {
        (String::new(), state.sections_done, state.sections_total)
    };

    let bar_width = layout[1].width.saturating_sub(label.len() as u16 + 12);
    let filled = if total > 0 {
        ((done as f64 / total as f64) * bar_width as f64) as u16
    } else {
        0
    };
    let empty = bar_width.saturating_sub(filled);

    let progress_spans = vec![
        Span::styled(&label, Style::default().fg(Color::White)),
        Span::styled(
            "█".repeat(filled as usize),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            "░".repeat(empty as usize),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("  {}/{} tasks", done, total),
            Style::default().fg(Color::White),
        ),
    ];
    f.render_widget(Paragraph::new(Line::from(progress_spans)), layout[1]);
}

/// Check if a line is a numbered list item (e.g., "1. Something").
fn is_numbered_list(text: &str) -> bool {
    let trimmed = text.trim_start();
    if let Some(dot_pos) = trimmed.find(". ") {
        trimmed[..dot_pos].chars().all(|c| c.is_ascii_digit()) && dot_pos > 0
    } else {
        false
    }
}

/// Capitalize first letter of a string.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[derive(Clone, Debug)]
pub struct OutputLine {
    pub text: String,
    pub kind: LineKind,
}

#[derive(Clone, Debug)]
pub enum LineKind {
    Assistant,
    User,
    Thinking,
    Tool,
    #[allow(dead_code)]
    ToolResult,
    Error,
    System,
}

impl OutputLine {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::System,
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::User,
        }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Assistant,
        }
    }
    pub fn thinking(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Thinking,
        }
    }
    pub fn tool(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Tool,
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Error,
        }
    }
}

// ── Pure helper functions ────────────────────────────────────────────

/// Compute context window usage percentage. Clamps at 100.
pub fn compute_context_pct(input_tokens: u64, context_window: u64) -> u8 {
    if context_window == 0 {
        return 0;
    }
    ((input_tokens as f64 / context_window as f64) * 100.0).min(100.0) as u8
}

/// Parse a `git log -1 --format='%h %s'` output line into (hash, subject).
pub fn parse_git_log_line(line: &str) -> (String, String) {
    let trimmed = line.trim();
    if let Some(pos) = trimmed.find(' ') {
        (trimmed[..pos].to_string(), trimmed[pos + 1..].to_string())
    } else {
        (trimmed.to_string(), String::new())
    }
}
