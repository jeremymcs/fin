// Fin — TUI Widget Helpers
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Named color palette — single source of truth for all TUI widget colors.
/// Per D-03: No inline Color:: literals in render functions after Phase 1.
/// Per D-04: ANSI named colors only — no Color::Rgb or Color::Indexed.
pub struct Palette;

impl Palette {
    pub const ACCENT: Color = Color::Yellow; // D-01: amber accent
    pub const TOOL: Color = Color::Cyan; // D-02: tool-call highlight
    pub const TEXT: Color = Color::White; // body text
    pub const DIM: Color = Color::DarkGray; // subdued, borders, thinking
    pub const SUCCESS: Color = Color::Green; // user text, tool results, done markers
    pub const ERROR: Color = Color::Red; // error lines
    pub const STATUS_BG: Color = Color::DarkGray; // status bar background
}

/// Footer hint text shown in auto-mode panel (AUTO-05).
pub const FOOTER_HINTS: &str = "esc pause  │  ? help";

/// Truncate string to max chars, appending U+2026 ellipsis if needed.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

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
        .map(|l| Line::styled(*l, Style::default().fg(Palette::DIM)))
        .collect();
    let logo = Paragraph::new(logo_lines);
    f.render_widget(logo, h_layout[0]);

    // -- Separator --
    let sep_lines: Vec<Line> = (0..content_area.height)
        .map(|_| Line::styled("│", Style::default().fg(Palette::DIM)))
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
        Span::styled(" Fin", Style::default().fg(Palette::TEXT).bold()),
        Span::raw("  "),
        Span::styled(
            format!("v{}", info.version),
            Style::default().fg(Palette::DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(title_line), info_layout[0]);

    // Model
    let model_line = Line::from(vec![
        Span::styled(" Model     ", Style::default().fg(Palette::ACCENT).bold()),
        Span::styled(&info.model_id, Style::default().fg(Palette::TEXT)),
    ]);
    f.render_widget(Paragraph::new(model_line), info_layout[2]);

    // Provider
    let provider_line = Line::from(vec![
        Span::styled(" Provider  ", Style::default().fg(Palette::ACCENT).bold()),
        Span::styled(&info.provider, Style::default().fg(Palette::TEXT)),
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
        Span::styled(" Directory ", Style::default().fg(Palette::ACCENT).bold()),
        Span::styled(dir_display, Style::default().fg(Palette::TEXT)),
    ]);
    f.render_widget(Paragraph::new(dir_line), info_layout[4]);

    // Extensions status bar
    let mut ext_spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (name, ok)) in info.extensions.iter().enumerate() {
        if i > 0 {
            ext_spans.push(Span::styled("  ·  ", Style::default().fg(Palette::DIM)));
        }
        ext_spans.push(Span::styled(
            name.clone(),
            Style::default().fg(if *ok {
                Palette::SUCCESS
            } else {
                Palette::ERROR
            }),
        ));
        ext_spans.push(Span::styled(
            if *ok { " ✓" } else { " ✗" },
            Style::default().fg(if *ok {
                Palette::SUCCESS
            } else {
                Palette::ERROR
            }),
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
                        Style::default().fg(Palette::DIM),
                    ));
                }
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::SUCCESS).bold(),
                ));
            }
            LineKind::Assistant => {
                if !line.is_final {
                    // Streaming line — plain text, no markdown parsing (D-08: prevents flicker)
                    spans_lines.push(Line::styled(
                        line.text.clone(),
                        Style::default().fg(Palette::TEXT),
                    ));
                } else {
                    // Finalized line — apply markdown rendering (D-10: Assistant only)
                    let text = &line.text;
                    if text.starts_with("# ") || text.starts_with("## ") || text.starts_with("### ")
                    {
                        // Markdown headers — bold accent
                        spans_lines.push(Line::styled(
                            text.clone(),
                            Style::default().fg(Palette::ACCENT).bold(),
                        ));
                    } else if text.starts_with("```") {
                        // Code fence markers — dim
                        spans_lines.push(Line::styled(
                            text.clone(),
                            Style::default().fg(Palette::DIM),
                        ));
                    } else if text.starts_with("- ") || text.starts_with("* ") {
                        // Unordered bullet lists — accent prefix + markdown-parsed content
                        let content = text.trim_start_matches("- ").trim_start_matches("* ");
                        let base_style = Style::default().fg(Palette::TEXT);
                        let mut spans = vec![Span::styled(
                            "  \u{2022} ",
                            Style::default().fg(Palette::ACCENT),
                        )];
                        spans.extend(parse_inline_spans(content, base_style));
                        spans_lines.push(Line::from(spans));
                    } else if is_numbered_list(text) {
                        // Numbered lists — accent prefix + markdown-parsed content
                        let dot_pos = text.find(". ").unwrap_or(0);
                        let num = &text[..dot_pos + 1];
                        let content = text[dot_pos + 2..].to_string();
                        let base_style = Style::default().fg(Palette::TEXT);
                        let mut spans = vec![Span::styled(
                            format!("  {num} "),
                            Style::default().fg(Palette::ACCENT),
                        )];
                        spans.extend(parse_inline_spans(&content, base_style));
                        spans_lines.push(Line::from(spans));
                    } else if text.ends_with('?') {
                        // Questions — accent color with inline markdown
                        let base_style = Style::default().fg(Palette::ACCENT);
                        spans_lines.push(Line::from(parse_inline_spans(text, base_style)));
                    } else {
                        // Plain text with inline markdown (bold/italic/code)
                        let base_style = Style::default().fg(Palette::TEXT);
                        spans_lines.push(Line::from(parse_inline_spans(text, base_style)));
                    }
                }
            }
            LineKind::Thinking => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::DIM).italic(),
                ));
            }
            LineKind::Tool => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::TOOL).dim(),
                ));
            }
            LineKind::ToolResult => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::SUCCESS).dim(),
                ));
            }
            LineKind::Error => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::ERROR).bold(),
                ));
            }
            LineKind::System => {
                spans_lines.push(Line::styled(
                    line.text.clone(),
                    Style::default().fg(Palette::DIM),
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
        .style(Style::default().fg(Palette::ACCENT))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Palette::DIM)),
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
    let in_fmt = format_token_count(tokens_in);
    let out_fmt = format_token_count(tokens_out);
    let status = if is_streaming {
        format!(
            " {model} | streaming...{scroll_indicator}{wf_indicator} | in:{in_fmt} out:{out_fmt} | ${cost:.4}"
        )
    } else {
        format!(
            " {model} | ready{scroll_indicator}{wf_indicator} | in:{in_fmt} out:{out_fmt} | ${cost:.4}"
        )
    };

    Paragraph::new(status).style(Style::default().fg(Palette::TEXT).bg(Palette::STATUS_BG))
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
        .border_style(Style::default().fg(Palette::DIM))
        .title(Span::styled(
            format!(" {} ", state.blueprint_id),
            Style::default().fg(Palette::TEXT).bold(),
        ))
        .title(Span::styled(
            format!(" {} ", capitalize(&state.current_stage)),
            Style::default().fg(Palette::ACCENT),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let row_constraints: Vec<Constraint> = if state.is_auto && state.active {
        vec![
            Constraint::Length(1), // row 0: pipeline (unchanged)
            Constraint::Length(1), // row 1: progress bar (unchanged)
            Constraint::Length(1), // row 2: model + blueprint
            Constraint::Length(1), // row 3: stage + section
            Constraint::Length(1), // row 4: last commit
            Constraint::Length(1), // row 5: context % bar
            Constraint::Length(1), // row 6: footer hints
        ]
    } else {
        vec![
            Constraint::Length(1), // row 0: pipeline (unchanged)
            Constraint::Length(1), // row 1: progress bar (unchanged)
        ]
    };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(inner);

    // Row 0: Stage pipeline (AUTO-06 — unchanged)
    let mut pipeline_spans: Vec<Span> = Vec::new();
    for (i, (name, status)) in state.stage_pipeline.iter().enumerate() {
        if i > 0 {
            pipeline_spans.push(Span::styled("  ", Style::default()));
        }
        let (icon, color) = match status {
            StageStatus::Done => ("✓", Palette::SUCCESS),
            StageStatus::Active => ("●", Palette::ACCENT),
            StageStatus::Pending => ("○", Palette::DIM),
        };
        pipeline_spans.push(Span::styled(
            format!("{name} {icon}"),
            Style::default().fg(color),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(pipeline_spans)), layout[0]);

    // Row 1: Progress bar + counts (AUTO-06 — unchanged)
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
        Span::styled(&label, Style::default().fg(Palette::TEXT)),
        Span::styled(
            "█".repeat(filled as usize),
            Style::default().fg(Palette::SUCCESS),
        ),
        Span::styled(
            "░".repeat(empty as usize),
            Style::default().fg(Palette::DIM),
        ),
        Span::styled(
            format!("  {}/{} tasks", done, total),
            Style::default().fg(Palette::TEXT),
        ),
    ];
    f.render_widget(Paragraph::new(Line::from(progress_spans)), layout[1]);

    // Rows 2-6: Auto-mode expansion (AUTO-01 through AUTO-05)
    if state.is_auto && state.active && layout.len() >= 7 {
        let panel_width = inner.width as usize;

        // Row 2: model + blueprint (AUTO-01, UI-SPEC Row 2)
        let model_str = truncate_str(&state.model_display, panel_width / 2 - 3);
        let sep = "  │  ";
        let bp_max = panel_width.saturating_sub(model_str.chars().count() + sep.len() + 1);
        let bp_str = truncate_str(&state.blueprint_id, bp_max.min(30));
        let model_bp_spans = vec![
            Span::styled(model_str.as_str(), Style::default().fg(Palette::TEXT)),
            Span::styled(sep, Style::default().fg(Palette::DIM)),
            Span::styled(bp_str.as_str(), Style::default().fg(Palette::TEXT)),
        ];
        f.render_widget(Paragraph::new(Line::from(model_bp_spans)), layout[2]);

        // Row 3: stage + section (AUTO-02, UI-SPEC Row 3)
        let stage_display = capitalize(&state.current_stage);
        let section_display = state
            .current_section
            .as_deref()
            .or(state.current_task.as_deref())
            .unwrap_or("");
        let section_str = truncate_str(section_display, 30);
        let stage_section_spans = vec![
            Span::styled(stage_display.as_str(), Style::default().fg(Palette::ACCENT)),
            Span::styled(" › ", Style::default().fg(Palette::DIM)),
            Span::styled(section_str.as_str(), Style::default().fg(Palette::TEXT)),
        ];
        f.render_widget(Paragraph::new(Line::from(stage_section_spans)), layout[3]);

        // Row 4: last git commit (AUTO-03, UI-SPEC Row 4)
        if state.last_commit_hash.is_empty() {
            // Empty state: em dash (D-06)
            f.render_widget(
                Paragraph::new(Span::styled("—", Style::default().fg(Palette::DIM))),
                layout[4],
            );
        } else {
            let commit_max = panel_width.saturating_sub(9); // 7 hash + 1 space + 1 buffer
            let commit_msg = truncate_str(&state.last_commit_msg, commit_max);
            let commit_spans = vec![
                Span::styled(
                    state.last_commit_hash.as_str(),
                    Style::default().fg(Palette::DIM),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(commit_msg.as_str(), Style::default().fg(Palette::TEXT)),
            ];
            f.render_widget(Paragraph::new(Line::from(commit_spans)), layout[4]);
        }

        // Row 5: context % bar (AUTO-04, UI-SPEC Row 5, D-16)
        let ctx_label = if state.context_pct == 0 {
            "ctx  ?%".to_string()
        } else {
            format!("ctx {:>2}%", state.context_pct)
        };
        let bar_width = layout[5].width.saturating_sub(10);
        let filled = ((state.context_pct as f64 / 100.0) * bar_width as f64) as u16;
        let empty = bar_width.saturating_sub(filled);
        let ctx_spans = vec![
            Span::styled(
                format!("{:<8}  ", ctx_label),
                Style::default().fg(Palette::TEXT),
            ),
            Span::styled(
                "█".repeat(filled as usize),
                Style::default().fg(Palette::ACCENT),
            ),
            Span::styled(
                "░".repeat(empty as usize),
                Style::default().fg(Palette::DIM),
            ),
        ];
        f.render_widget(Paragraph::new(Line::from(ctx_spans)), layout[5]);

        // Row 6: footer hints (AUTO-05, D-15)
        let footer_spans = vec![Span::styled(
            FOOTER_HINTS,
            Style::default().fg(Palette::DIM),
        )];
        f.render_widget(Paragraph::new(Line::from(footer_spans)), layout[6]);
    }
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
    /// True when the line is complete and safe for markdown parsing.
    /// Per D-08: streaming (in-progress) lines render plain to prevent per-frame flicker.
    pub is_final: bool,
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
            is_final: true,
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::User,
            is_final: true,
        }
    }
    /// Assistant lines default to is_final=false (finalized by TextDelta newline or AgentEnd).
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Assistant,
            is_final: false,
        }
    }
    pub fn thinking(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Thinking,
            is_final: true,
        }
    }
    pub fn tool(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Tool,
            is_final: true,
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Error,
            is_final: true,
        }
    }
}

// ── Markdown span helpers ────────────────────────────────────────────

/// Flush accumulated text as a styled span, if non-empty.
fn flush_span(spans: &mut Vec<Span<'static>>, text: &mut String, style: Style) {
    if !text.is_empty() {
        spans.push(Span::styled(std::mem::take(text), style));
    }
}

/// Parse inline markdown spans (bold, italic, code) into styled ratatui Spans.
/// Only call this for finalized assistant lines (is_final == true).
/// Per D-07: Uses pulldown-cmark 0.12 for parsing.
/// Per D-09: **bold** -> BOLD, *italic* -> ITALIC, `code` -> REVERSED.
/// Per D-10: Only LineKind::Assistant lines should be passed here.
pub fn parse_inline_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_style = base_style;
    let mut current_text = String::new();

    let parser = Parser::new_ext(text, Options::empty());
    for event in parser {
        match event {
            Event::Text(t) => current_text.push_str(&t),
            Event::Code(t) => {
                flush_span(&mut spans, &mut current_text, current_style);
                spans.push(Span::styled(
                    t.to_string(),
                    base_style.add_modifier(Modifier::REVERSED),
                ));
            }
            Event::Start(Tag::Strong) => {
                flush_span(&mut spans, &mut current_text, current_style);
                current_style = base_style.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                flush_span(&mut spans, &mut current_text, current_style);
                current_style = base_style;
            }
            Event::Start(Tag::Emphasis) => {
                flush_span(&mut spans, &mut current_text, current_style);
                current_style = base_style.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                flush_span(&mut spans, &mut current_text, current_style);
                current_style = base_style;
            }
            _ => {}
        }
    }
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }
    spans
}

/// Format token count for display — abbreviates values >= 1000 (e.g., 1243 -> "1.2k").
/// Per D-11: status bar and per-message annotation use this formatter.
pub fn format_token_count(count: u64) -> String {
    if count >= 1000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else {
        count.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_inline_spans tests ─────────────────────────────────────

    #[test]
    fn test_parse_inline_spans_bold() {
        let base = Style::default().fg(Color::White);
        let spans = parse_inline_spans("foo **bar** baz", base);
        assert_eq!(spans.len(), 3, "expected 3 spans for bold");
        assert_eq!(spans[1].content.as_ref(), "bar");
        assert!(
            spans[1].style.add_modifier.contains(Modifier::BOLD),
            "span[1] must have BOLD modifier"
        );
    }

    #[test]
    fn test_parse_inline_spans_italic() {
        let base = Style::default().fg(Color::White);
        let spans = parse_inline_spans("foo *bar* baz", base);
        assert_eq!(spans.len(), 3, "expected 3 spans for italic");
        assert_eq!(spans[1].content.as_ref(), "bar");
        assert!(
            spans[1].style.add_modifier.contains(Modifier::ITALIC),
            "span[1] must have ITALIC modifier"
        );
    }

    #[test]
    fn test_parse_inline_spans_code() {
        let base = Style::default().fg(Color::White);
        let spans = parse_inline_spans("foo `bar` baz", base);
        assert_eq!(spans.len(), 3, "expected 3 spans for inline code");
        assert_eq!(spans[1].content.as_ref(), "bar");
        assert!(
            spans[1].style.add_modifier.contains(Modifier::REVERSED),
            "span[1] must have REVERSED modifier"
        );
    }

    #[test]
    fn test_parse_inline_spans_plain() {
        let base = Style::default().fg(Color::White);
        let spans = parse_inline_spans("no markdown here", base);
        assert_eq!(spans.len(), 1, "expected 1 span for plain text");
        assert_eq!(spans[0].content.as_ref(), "no markdown here");
    }

    // ── format_token_count tests ─────────────────────────────────────

    #[test]
    fn test_format_token_count_zero() {
        assert_eq!(format_token_count(0), "0");
    }

    #[test]
    fn test_format_token_count_below() {
        assert_eq!(format_token_count(999), "999");
    }

    #[test]
    fn test_format_token_count_exact_thousand() {
        assert_eq!(format_token_count(1000), "1.0k");
    }

    #[test]
    fn test_format_token_count_above() {
        assert_eq!(format_token_count(1243), "1.2k");
    }

    #[test]
    fn test_format_token_count_large() {
        assert_eq!(format_token_count(15432), "15.4k");
    }

    // ── Phase 3 Wave 0: auto-run panel tests ────────────────────────

    #[test]
    fn test_workflow_state_auto_fields() {
        let state = WorkflowState::default();
        assert!(!state.is_auto, "is_auto must default to false");
        assert!(
            state.model_display.is_empty(),
            "model_display must default to empty"
        );
        assert!(
            state.last_commit_hash.is_empty(),
            "last_commit_hash must default to empty"
        );
        assert!(
            state.last_commit_msg.is_empty(),
            "last_commit_msg must default to empty"
        );
        assert_eq!(state.context_pct, 0, "context_pct must default to 0");
    }

    #[test]
    fn test_workflow_state_unit_start() {
        let mut state = WorkflowState::default();
        state.active = true;
        state.current_stage = "build".to_string();
        state.current_section = Some("section-03".to_string());

        assert_eq!(
            state.current_stage, "build",
            "current_stage must be populated from WorkflowUnitStart"
        );
        assert_eq!(
            state.current_section.as_deref(),
            Some("section-03"),
            "current_section must be populated from WorkflowUnitStart"
        );

        state.update_pipeline();
        let build_status = state
            .stage_pipeline
            .iter()
            .find(|(name, _)| name == "Build")
            .map(|(_, status)| status);
        assert!(
            build_status.is_some(),
            "Build stage must appear in pipeline after update"
        );
    }

    #[test]
    fn test_context_pct_calculation() {
        assert_eq!(compute_context_pct(50_000, 200_000), 25);
        assert_eq!(compute_context_pct(100_000, 200_000), 50);
        assert_eq!(compute_context_pct(0, 200_000), 0);
        assert_eq!(compute_context_pct(200_000, 200_000), 100);
    }

    #[test]
    fn test_context_pct_clamped() {
        assert_eq!(compute_context_pct(250_000, 200_000), 100);
        assert_eq!(compute_context_pct(999_999, 200_000), 100);
    }

    #[test]
    fn test_context_pct_zero_window() {
        assert_eq!(compute_context_pct(50_000, 0), 0);
    }

    #[test]
    fn test_pipeline_row_unchanged_in_auto() {
        let mut state_normal = WorkflowState::default();
        state_normal.active = true;
        state_normal.current_stage = "build".to_string();
        state_normal.update_pipeline();

        let mut state_auto = WorkflowState::default();
        state_auto.active = true;
        state_auto.is_auto = true;
        state_auto.current_stage = "build".to_string();
        state_auto.update_pipeline();

        assert_eq!(
            state_normal.stage_pipeline.len(),
            state_auto.stage_pipeline.len()
        );
        for (i, ((name_n, status_n), (name_a, status_a))) in state_normal
            .stage_pipeline
            .iter()
            .zip(state_auto.stage_pipeline.iter())
            .enumerate()
        {
            assert_eq!(name_n, name_a, "pipeline name mismatch at index {i}");
            assert_eq!(
                std::mem::discriminant(status_n),
                std::mem::discriminant(status_a),
                "pipeline status mismatch at index {i}"
            );
        }
    }

    #[test]
    fn test_render_footer_hints() {
        // AUTO-05: verify the footer hints constant is accessible and correct.
        assert_eq!(FOOTER_HINTS, "esc pause  │  ? help");
        assert!(FOOTER_HINTS.contains("esc"), "footer must mention esc key");
        assert!(FOOTER_HINTS.contains("help"), "footer must mention help");
    }
}
