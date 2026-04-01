// Fin — Phase 1: Foundation Research
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Phase 1: Foundation - Research

**Researched:** 2026-04-01
**Domain:** Ratatui TUI widget rendering — Rust, ratatui 0.29, crossterm 0.28, pulldown-cmark 0.12.2
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Color Palette**
- D-01: Amber accent — `Color::Yellow` becomes the primary accent color (labels, borders, active indicators, splash title). Replaces the current cyan-as-accent role.
- D-02: `Color::Cyan` becomes the tool-call highlight color (replaces Yellow dim for `LineKind::Tool`). Semantic swap: Yellow → accent, Cyan → tools.
- D-03: All color references must go through a single named `Palette` const struct in `widgets.rs`. No inline `Color::` literals scattered across render functions after Phase 1.
- D-04: ANSI named colors only (`Color::Yellow`, `Color::Cyan`, `Color::White`, `Color::DarkGray`, `Color::Green`, `Color::Red`). No `Color::Rgb` or `Color::Indexed` — terminal compatibility required.

**AppLayout Struct**
- D-05: Extract a named `AppLayout` struct (or equivalent named bindings) that replaces all `chunks[N]` index arithmetic in `app.rs`. Phase 1 prerequisite for Phases 3 and 4.
- D-06: The struct must accommodate the two existing layout variants (with workflow panel active, without) without raw index offsets.

**Inline Markdown Rendering**
- D-07: Use `pulldown-cmark` 0.12.2 (already in `Cargo.toml`, currently unused) for span parsing. No new crates.
- D-08: Parser is gated behind an `is_final: bool` flag on `OutputLine`. Streaming (in-progress) lines render as plain text. Parser only runs on finalized lines.
- D-09: Three patterns in scope: `**bold**` → `Modifier::BOLD`, `*italic*` → `Modifier::ITALIC`, `` `code` `` → `Modifier::REVERSED` preferred (helix-style); fall back to `Modifier::BOLD | Modifier::DIM` if terminal theme issues found during implementation.
- D-10: Only `LineKind::Assistant` lines are markdown-parsed. Other kinds (User, Tool, System, Error, Thinking) render as plain text.

**Token/Cost Display**
- D-11: Status bar format: `{model} | {state}{scroll}{workflow} | in:{n} out:{n} | ${cost:.4}`. Formatting improvement: abbreviate large numbers (e.g., `1.2k` instead of `1243`).
- D-12: Per-message cost annotation: append a new `LineKind::System` `OutputLine` after each completed assistant response (on `AgentEvent::AgentEnd`). Format: `  ↳ {in} in / {out} out  ${cost:.4}` rendered dim.
- D-13: Per-message annotation is not emitted during streaming — only after the turn completes.

### Claude's Discretion

- Inline code (`code`) visual modifier: REVERSED preferred (helix convention, immediately recognizable), BOLD+DIM acceptable fallback — choose based on visual testing during implementation.
- Number abbreviation threshold for token counts: suggest ≥1000 → `1.2k` format, but exact breakpoint is implementation detail.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within Phase 1 scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| THEME-01 | User sees a consistent color palette across all TUI widgets (output, status bar, workflow panel, input, splash) | Palette const struct pattern documented below; all 6 color roles identified |
| THEME-02 | Named palette constants are defined in `widgets.rs` so future color changes require editing one place | `const struct Palette` design with associated consts; see Architecture Patterns |
| MD-01 | User sees **bold** text rendered with bold styling in assistant output | pulldown-cmark span parsing approach with `is_final` gate documented; ratatui `Modifier::BOLD` spans |
| MD-02 | User sees *italic* text rendered with italic styling in assistant output | Same span parser as MD-01; `Modifier::ITALIC` |
| MD-03 | User sees `inline code` rendered with distinct styling in assistant output | `Modifier::REVERSED` preferred; see D-09 |
| MD-04 | Markdown rendering does not flicker on partial/streaming lines | `is_final: bool` field on `OutputLine`; finalization logic in `AgentEnd` handler documented |
| TOK-01 | User sees per-message cost annotation as a dim line after each completed assistant response | `AgentEvent::AgentEnd` carries `Usage`; append `LineKind::System` with `↳` format at that hook point |
| TOK-02 | User sees a cleaner token/cost summary in the status bar (formatted counts, not raw integers) | `format_token_count()` helper design documented; touches only `render_status_bar()` |
</phase_requirements>

---

## Summary

Phase 1 is a pure rendering layer change. All work lives in two files — `src/tui/widgets.rs` and a small portion of `src/tui/app.rs`. No new crates, no new `AgentEvent` variants, no layout structural changes. The build is currently clean (verified: `cargo build` succeeds in 0.35s dev profile). pulldown-cmark 0.12.2 and ratatui 0.29.0 are already present and locked in Cargo.lock.

The four implementation areas decompose cleanly: (1) extract a `Palette` const struct and do a color-role swap in `widgets.rs`; (2) extract an `AppLayout` struct in `app.rs` to replace all `chunks[N]` references; (3) add `is_final: bool` to `OutputLine` and a `parse_inline_spans()` function in `widgets.rs` that runs pulldown-cmark on finalized assistant lines; (4) replace the existing per-turn `└─` annotation with the new `↳` format in the `AgentEnd` handler and update `render_status_bar()` to abbreviate large numbers.

The highest-risk item is the `is_final` finalization gate: lines must be marked final at the right moment (at `\n` receipt in `TextDelta` and unconditionally in `AgentEnd` for the last line). The lowest-risk items are the Palette refactor and the status bar number formatting, which are purely cosmetic.

**Primary recommendation:** Implement in this order — AppLayout struct first (de-risks future phases), then Palette (pure style constants), then status bar formatting (no state changes), then `is_final` field + markdown parser (most logic, but contained to widgets.rs).

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.29.0 (locked) | TUI frame rendering, widgets, layout | Already in use; all existing widget code is ratatui |
| crossterm | 0.28.1 (locked) | Terminal backend for ratatui | Already in use; provides raw mode, key events |
| pulldown-cmark | 0.12.2 (locked) | Markdown span parsing (bold/italic/code) | Already in Cargo.toml; no new dependency cost |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ratatui `Modifier` | (part of ratatui) | Text styling flags: BOLD, ITALIC, REVERSED, DIM | Used in `Span::styled()` calls for inline markdown |
| ratatui `Style` | (part of ratatui) | Combines `Color` fg/bg with `Modifier` | Every styled span in widgets.rs |
| ratatui `Line` / `Span` | (part of ratatui) | Multi-span line construction | Required for per-span markdown styling |
| `std::fmt` | stdlib | `format_token_count()` helper | Token abbreviation formatting |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| pulldown-cmark span events | Hand-rolled `**`/`*` scanner | Scanner is ~40 lines; pulldown-cmark handles edge cases (escaped delimiters, nested spans) and is already present — no reason to hand-roll |
| `Modifier::REVERSED` for code | `Color::Rgb` background tint | Rgb requires truecolor; REVERSED works on all terminals and is already the helix convention |
| Named `Palette` const struct | Module-level `const` items | Either works; const struct groups logically, is zero-cost, and is the pattern already in widgets.rs style |

**Installation:** No new packages. All dependencies already in `Cargo.toml` and `Cargo.lock`.

**Version verification (confirmed from Cargo.lock 2026-04-01):**
- pulldown-cmark: 0.12.2
- ratatui: 0.29.0
- crossterm: 0.28.1

---

## Architecture Patterns

### Recommended Project Structure

Phase 1 touches two files only:

```
src/tui/
├── widgets.rs     — Palette struct, parse_inline_spans(), is_final field, render_* updates
└── app.rs         — AppLayout struct, OutputLine finalization, AgentEnd annotation update
```

No new files are created.

### Pattern 1: Palette Const Struct

**What:** A zero-cost `const struct Palette` at the top of `widgets.rs` that groups all color and style constants. Render functions reference `Palette::ACCENT`, `Palette::TOOL`, etc. instead of inline `Color::` literals.

**When to use:** Every `Style::default().fg(Color::X)` call in `widgets.rs` is replaced with `Style::default().fg(Palette::ACCENT)` (or whichever semantic role applies).

**Design:**
```rust
// Source: direct analysis of widgets.rs color audit
struct Palette;

impl Palette {
    // Accent (amber/yellow) — labels, active borders, splash title, input prompt
    const ACCENT: Color = Color::Yellow;
    // Tool events highlight (cyan)
    const TOOL: Color = Color::Cyan;
    // Normal text
    const TEXT: Color = Color::White;
    // Subdued / separators / thinking / borders
    const DIM: Color = Color::DarkGray;
    // User input, tool results, workflow done
    const SUCCESS: Color = Color::Green;
    // Errors
    const ERROR: Color = Color::Red;
    // Status bar background
    const STATUS_BG: Color = Color::DarkGray;
}
```

**Color role swap (D-01/D-02):**

| Widget/Line Kind | Current color | Phase 1 color | Palette constant |
|-----------------|---------------|---------------|-----------------|
| Splash title "Fin" | `Color::White` bold | unchanged | `Palette::TEXT` |
| Splash labels ("Model", "Provider") | `Color::Cyan` bold | `Color::Yellow` bold | `Palette::ACCENT` |
| Input prompt `[model] >` | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |
| Workflow panel active stage | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |
| Workflow panel border title (stage name) | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |
| `LineKind::Tool` events | `Color::Yellow` dim | `Color::Cyan` dim | `Palette::TOOL` |
| Pipeline active stage `●` | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |
| Bullet/numbered list prefix `•` / `N.` | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |
| Header lines `# ...` | `Color::Cyan` bold | `Color::Yellow` bold | `Palette::ACCENT` |
| Question lines (ends with `?`) | `Color::Cyan` | remove heuristic or use `Palette::ACCENT` | `Palette::ACCENT` |
| User text | `Color::Green` bold | unchanged | `Palette::SUCCESS` |
| Model picker border | `Color::Cyan` | `Color::Yellow` | `Palette::ACCENT` |

### Pattern 2: AppLayout Struct

**What:** A struct returned from the layout computation that assigns named `Rect` fields instead of positional `chunks[N]` access.

**When to use:** Replace the entire `if wf_active { ... } else { ... }` layout block in `app.rs:222–243` and all downstream `chunks[N]` references.

**Design:**
```rust
// Source: direct analysis of app.rs layout block lines 222–343
struct AppLayout {
    output:   Rect,
    workflow: Option<Rect>,  // None when !wf_active
    status:   Rect,
    input:    Rect,
}

impl AppLayout {
    fn compute(area: Rect, wf_active: bool) -> Self {
        if wf_active {
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
                output:   chunks[0],
                workflow: Some(chunks[1]),
                status:   chunks[3],
                input:    chunks[4],
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
                output:   chunks[0],
                workflow: None,
                status:   chunks[2],
                input:    chunks[3],
            }
        }
    }
}
```

**Downstream fix:** The cursor placement at `app.rs:338–342` currently reads:
```rust
let input_chunk = if wf_active { chunks[3] } else { chunks[2] };
```
With `AppLayout` this becomes simply `layout.input`.

### Pattern 3: OutputLine Finalization and is_final Gate

**What:** Add `is_final: bool` to `OutputLine`. Lines are created with `is_final: false`. A line is finalized either when a `\n` is encountered in `TextDelta` (causing a new line to start) or unconditionally at `AgentEnd` (the last partial line is complete). `parse_inline_spans()` is only called when `line.is_final == true`.

**When to use:** Applied only in the `LineKind::Assistant` render branch of `render_output()`.

**Finalization sites in app.rs:**

1. In the `TextDelta` handler — when `i > 0` (a newline split), the line at `i-1` is already pushed and considered final. The *previous* assistant line needs finalizing before the new one starts:
   ```rust
   // When splitting on \n: finalize the line that just ended
   if let Some(prev) = output_lines.iter_mut().rev()
       .skip(1)  // skip the one we just pushed
       .find(|l| matches!(l.kind, LineKind::Assistant)) {
       prev.is_final = true;
   }
   ```
   Simpler approach: mark the last assistant line as final before pushing any new line after a `\n`.

2. In the `AgentEnd` handler — finalize the last assistant line unconditionally:
   ```rust
   if let Some(last) = output_lines.iter_mut().rev()
       .find(|l| matches!(l.kind, LineKind::Assistant)) {
       last.is_final = true;
   }
   ```

**OutputLine struct change:**
```rust
// Source: widgets.rs lines 472-527
pub struct OutputLine {
    pub text: String,
    pub kind: LineKind,
    pub is_final: bool,   // NEW: true when line content is complete
}
```

All existing `OutputLine::assistant()`, `OutputLine::system()`, `OutputLine::user()` etc. constructors default `is_final: false` for User/System/Tool (they are always complete at push time, so set to `true`), and `false` for Assistant lines (finalized later).

**Simpler approach for non-assistant lines:** Since User, System, Tool, Error, Thinking lines are always pushed as complete units (not built incrementally), their `is_final` can default to `true` at construction time. Only `LineKind::Assistant` lines start as `false` and need explicit finalization.

### Pattern 4: parse_inline_spans() Using pulldown-cmark

**What:** A function in `widgets.rs` that takes a finalized assistant line text and returns a `Vec<Span<'static>>` with bold/italic/code spans applied.

**When to use:** Called from the plain-text fallback branch of the `LineKind::Assistant` match arm in `render_output()`, only when `line.is_final`.

**pulldown-cmark usage:**
```rust
// Source: pulldown-cmark 0.12 API — Parser iterates events
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

fn parse_inline_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_style = base_style;
    let mut current_text = String::new();

    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH);
    for event in parser {
        match event {
            Event::Text(t) => current_text.push_str(&t),
            Event::Code(t) => {
                // flush buffered plain text
                if !current_text.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut current_text),
                        current_style,
                    ));
                }
                // inline code span
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
    // flush remainder
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }
    spans
}

fn flush_span(spans: &mut Vec<Span<'static>>, text: &mut String, style: Style) {
    if !text.is_empty() {
        spans.push(Span::styled(std::mem::take(text), style));
    }
}
```

**pulldown-cmark 0.12.2 API notes (verified from Cargo.lock):**
- `Tag::Strong` → bold, `Tag::Emphasis` → italic
- `Event::Code(CowStr)` → inline code (backtick span)
- `TagEnd` is the closing event type in 0.12 (not `Tag::End` — this is a breaking change from 0.11)
- `Options::ENABLE_STRIKETHROUGH` is safe to include; unused here but harmless

### Pattern 5: Token Count Abbreviation

**What:** A helper function for the status bar.

**Design:**
```rust
// Format token count: ≥1000 → "1.2k", otherwise raw integer
fn format_token_count(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}
```

**Status bar format after Phase 1:**
```
 claude-sonnet-4-5 | ready | in:12.4k out:3.1k | $0.0142
```

### Pattern 6: Per-Message Cost Annotation

**What:** Replace the existing `└─ N in / M out ────` system line in the `AgentEnd` handler with the new `↳` format that includes cost.

**Current code (app.rs:420-426):**
```rust
AgentEvent::AgentEnd { usage } => {
    // ...
    if usage.input_tokens > 0 || usage.output_tokens > 0 {
        output_lines.push(OutputLine::system(format!(
            "└─ {} in / {} out ────",
            usage.input_tokens, usage.output_tokens
        )));
    }
```

**After Phase 1:**
```rust
AgentEvent::AgentEnd { usage } => {
    // finalize last assistant line first
    if let Some(last) = output_lines.iter_mut().rev()
        .find(|l| matches!(l.kind, LineKind::Assistant)) {
        last.is_final = true;
    }
    // ...
    if usage.input_tokens > 0 || usage.output_tokens > 0 {
        output_lines.push(OutputLine::system(format!(
            "  \u{21b3} {} in / {} out  ${:.4}",
            format_token_count(usage.input_tokens),
            format_token_count(usage.output_tokens),
            usage.cost.total
        )));
    }
```

The `↳` character is U+21B3 (DOWNWARDS ARROW WITH TIP RIGHTWARDS). The `System` line rendered dim via `Color::DarkGray` (via `Palette::DIM`) satisfies D-12/D-13.

### Anti-Patterns to Avoid

- **Inline `Color::` literals in render functions:** After Phase 1, all color references go through `Palette`. Adding new render code with `Color::Cyan` directly violates D-03.
- **`chunks[N]` index access after AppLayout extraction:** Once `AppLayout::compute()` is in place, any `chunks[N]` reference is a regression. The struct fields are the contract.
- **Calling `parse_inline_spans()` on streaming (non-final) lines:** The `is_final` gate exists precisely to prevent per-frame flicker. Bypassing it defeats MD-04.
- **Using `Color::Rgb` for palette entries:** D-04 requires ANSI named colors only. No `Color::Rgb` anywhere.
- **Removing the `is_numbered_list()` function:** The existing `is_numbered_list()` helper in `widgets.rs` is used in the `LineKind::Assistant` render branch. Phase 1 does not remove it — but it runs before `parse_inline_spans()` (line-level dispatch happens first, span-level parsing is only in the plain-text fallback branch).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Markdown bold/italic/code parsing | Custom `**` delimiter scanner | pulldown-cmark 0.12.2 | Handles escaped delimiters, nested spans, edge cases like `**a*b**`. Already in Cargo.toml. ~70-line hand-roll would miss these. |
| Token count abbreviation | None — this is 3 lines | `format_token_count()` helper | Too small to justify a crate. Stdlib `format!` is sufficient. |
| Color const grouping | A crate for theming | `struct Palette` with `const` items | Zero-cost, zero-dependency. No theming crate exists that improves on this for a single-file widget module. |

**Key insight:** The only genuinely complex problem in Phase 1 is markdown span parsing. pulldown-cmark already solves it. Everything else is formatting and struct refactoring.

---

## Common Pitfalls

### Pitfall 1: Streaming Line Finalization Timing

**What goes wrong:** If `is_final` is not set correctly, assistant lines that were broken mid-way remain `is_final: false` permanently. After the turn completes, these finalized lines still render raw (no markdown) because the finalization in `AgentEnd` only finds the *last* assistant line by searching backwards.

**Why it happens:** The `TextDelta` handler builds lines incrementally. When a `\n` arrives, a new line is started but the *previous* line is not explicitly finalized — it just stops receiving appends.

**How to avoid:** At the moment a new line is started (after the `\n` split in `TextDelta`, when `i > 0`), find the just-completed assistant line and set `is_final = true`. The simplest implementation: track a `current_assistant_line_idx: Option<usize>` in the app state that always points to the index of the in-progress assistant line. When a new line starts, finalize the tracked index before updating it. At `AgentEnd`, finalize the tracked index.

**Warning signs:** Bold/italic text in multi-line responses shows raw asterisks on all lines except the last one.

---

### Pitfall 2: pulldown-cmark Wraps Paragraph Content in Paragraph Events

**What goes wrong:** When `Parser::new_ext(line_text, opts)` parses a single line, pulldown-cmark wraps the content in `Event::Start(Tag::Paragraph)` / `Event::End(TagEnd::Paragraph)` events around the inline events. Failing to account for these outer wrapper events is harmless (the match ignores them), but failing to account for `Event::SoftBreak` / `Event::HardBreak` emitted at line boundaries can cause spurious whitespace or missing text.

**Why it happens:** pulldown-cmark is a document parser; even a single-line input goes through paragraph normalization.

**How to avoid:** The `parse_inline_spans()` implementation above handles this correctly by matching only `Text`, `Code`, `Start(Tag::Strong)`, etc. and ignoring all other events via the `_ => {}` arm. Specifically, `Start(Tag::Paragraph)` and `End(TagEnd::Paragraph)` fall through to `_ => {}` and are silently ignored — correct behavior.

**Warning signs:** Text content appears doubled, or blank lines appear within a single assistant line's rendered output.

---

### Pitfall 3: chunks[N] Index Off-By-One After AppLayout Refactor

**What goes wrong:** The cursor placement at `app.rs:338` reads `let input_chunk = if wf_active { chunks[3] } else { chunks[2] }`. After the `AppLayout` refactor this is replaced with `layout.input`. If any call site is missed (e.g., the spacer chunk that is never rendered but was used as a placeholder index), the cursor may draw in the wrong position with no compile error.

**Why it happens:** Rust does not type-check `chunks[N]` index values. `chunks[3]` and `chunks[4]` are both valid `Rect` at runtime even if the semantic intent is wrong.

**How to avoid:** Search for all `chunks[` occurrences in `app.rs` before submitting the AppLayout refactor. The list as of current code: `chunks[0]` (output/splash), `chunks[1]` (workflow panel when wf_active), `chunks[3]` (status bar when wf_active), `chunks[4]` (input when wf_active), `chunks[2]` (status bar when !wf_active), `chunks[3]` (input when !wf_active). All must be replaced with named AppLayout fields.

**Warning signs:** Cursor appears in the status bar row. Input renders over the workflow panel. Both are visual-only and will not panic.

---

### Pitfall 4: OutputLine Constructor Defaults for is_final

**What goes wrong:** All constructors in `widgets.rs:490-527` (`system()`, `user()`, `tool()`, `error()`, `thinking()`) push complete, non-streaming lines. If these constructors set `is_final: false`, the markdown parser will never run on them — but since only `LineKind::Assistant` is parsed, this is harmless. However, if anyone later tries to add markdown to Tool or System lines, the `false` default will silently suppress it.

**How to avoid:** Non-Assistant constructors should set `is_final: true` at construction time (they are always complete). The `assistant()` constructor sets `is_final: false` (it will be set true by the finalization logic later).

---

### Pitfall 5: Color Swap Creates Visual Regression in the Workflow Panel

**What goes wrong:** The workflow pipeline uses `Color::Cyan` for the `Active` stage indicator (`●`). Swapping Cyan → Yellow for `Palette::ACCENT` (D-01/D-02) changes the active stage color from Cyan to Yellow. The `Done` stage uses `Color::Green` (no change). This is the intended visual outcome per D-01. However, the Thinking line kind also currently uses `Color::DarkGray` italic — it uses no Cyan, so it is unaffected. The one potential regression is readability: if Yellow active + Yellow accent elsewhere makes the active stage `●` less distinct from other Yellow elements.

**How to avoid:** During visual testing, verify that the active stage `●` in Yellow is distinct from the surrounding `DarkGray` pending `○` circles. Yellow on a dark terminal background is high-contrast. No code action needed — just verify during implementation.

---

## Code Examples

Verified patterns from source analysis:

### Current render_output() Assistant Arm (before Phase 1)
```rust
// Source: src/tui/widgets.rs lines 167-208
LineKind::Assistant => {
    let text = &line.text;
    if text.starts_with("# ") || ... {
        // headers, bullets, etc.
    } else {
        // plain-text fallback — ALL assistant lines end here if none of the above match
        spans_lines.push(Line::styled(
            text.clone(),
            Style::default().fg(Color::White),
        ));
    }
}
```

### AgentEnd Handler — Current Per-Turn Annotation
```rust
// Source: src/tui/app.rs lines 415-434
AgentEvent::AgentEnd { usage } => {
    is_streaming = false;
    total_in += usage.input_tokens;
    total_out += usage.output_tokens;
    total_cost += usage.cost.total;
    if usage.input_tokens > 0 || usage.output_tokens > 0 {
        output_lines.push(OutputLine::system(format!(
            "└─ {} in / {} out ────",
            usage.input_tokens, usage.output_tokens
        )));
    }
    output_lines.push(OutputLine::system(String::new()));
    auto_scroll(...);
}
```

### Usage Struct (available at AgentEnd)
```rust
// Source: src/llm/types.rs lines 136-152
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost: Cost,  // cost.total: f64
}
```

### Existing Two-Layout Block in app.rs (target for AppLayout refactor)
```rust
// Source: src/tui/app.rs lines 221-243
let wf_active = workflow_state.active;
let chunks = if wf_active {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Output
            Constraint::Length(4), // Workflow progress panel
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Status bar
            Constraint::Length(2), // Input
        ])
        .split(f.area())
} else {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Output
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Status bar
            Constraint::Length(2), // Input
        ])
        .split(f.area())
};
// ... then chunks[0], chunks[1], chunks[2], chunks[3], chunks[4] throughout
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| pulldown-cmark `Event::End(Tag::Strong)` | pulldown-cmark `Event::End(TagEnd::Strong)` | 0.12 (breaking change from 0.11) | Match on `TagEnd` enum, not nested `Tag`. Confirmed against 0.12.2 lock. |
| ratatui `Style::fg().add_modifier()` chaining | Same API, still valid in 0.29 | — | No change needed |
| `chunks[N]` raw indexing | Named layout struct | Phase 1 (this phase) | Future layout changes safe |

**Deprecated/outdated:**
- `Event::End(Tag::Strong)` from pulldown-cmark 0.11 — use `Event::End(TagEnd::Strong)` in 0.12.2.

---

## Open Questions

1. **Finalization of mid-stream lines on multi-line responses**
   - What we know: `AgentEnd` finalizes the last assistant line. Lines broken by `\n` during streaming are never explicitly finalized.
   - What's unclear: Whether a `current_assistant_line_idx` tracking variable should be added to `app.rs` state, or whether the simpler approach of finalizing all un-finalized assistant lines in `AgentEnd` is sufficient.
   - Recommendation: In `AgentEnd`, iterate all `output_lines` and set `is_final = true` for every `LineKind::Assistant` line that has `is_final == false`. This is O(n) but happens once per turn, not per frame. Simpler than index tracking.

2. **Question-mark heuristic removal**
   - What we know: `widgets.rs:200` has `text.ends_with('?') → Color::Cyan` for assistant lines. This is a line-level branch that runs before the plain-text fallback.
   - What's unclear: Whether this heuristic should be kept, removed, or replaced with Palette color.
   - Recommendation: Replace with `Palette::ACCENT` (Yellow) rather than removing, since the intent was visual distinction. With the Cyan → Yellow swap this now correctly uses the accent color. Mark this in the plan so the implementer is aware.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo | All compilation | ✓ | 1.93.1 (Homebrew) | — |
| pulldown-cmark | MD-01/02/03 | ✓ | 0.12.2 (Cargo.lock) | — |
| ratatui | All TUI rendering | ✓ | 0.29.0 (Cargo.lock) | — |
| crossterm | Terminal backend | ✓ | 0.28.1 (Cargo.lock) | — |

**Missing dependencies with no fallback:** None.

**Build status:** Clean. `cargo build` completes without warnings or errors (verified 2026-04-01).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | None — standard Cargo test runner |
| Quick run command | `cargo test --lib 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| THEME-01 | All widgets use Palette colors | Visual / manual | Manual terminal inspection | — |
| THEME-02 | Palette const struct exists in widgets.rs; single edit site | Structural / grep | `grep -c "Palette::" src/tui/widgets.rs` | — |
| MD-01 | `**bold**` → BOLD modifier, no raw `**` visible | Unit | `cargo test --lib parse_inline_spans` | ❌ Wave 0 |
| MD-02 | `*italic*` → ITALIC modifier, no raw `*` visible | Unit | `cargo test --lib parse_inline_spans` | ❌ Wave 0 |
| MD-03 | `` `code` `` → REVERSED modifier, no raw backticks visible | Unit | `cargo test --lib parse_inline_spans` | ❌ Wave 0 |
| MD-04 | No markdown parsed when `is_final: false` | Unit | `cargo test --lib is_final_gate` | ❌ Wave 0 |
| TOK-01 | `↳` annotation appended after AgentEnd | Integration / manual | Manual: trigger a turn, observe output | — |
| TOK-02 | Token counts ≥1000 shown as `Nk` in status bar | Unit | `cargo test --lib format_token_count` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green + manual terminal visual check before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/tui/widgets_test.rs` or `#[cfg(test)] mod tests` block in `widgets.rs` — covers MD-01, MD-02, MD-03, MD-04, TOK-02
  - `test_parse_inline_spans_bold()` — input: `"foo **bar** baz"`, assert span 1 has `Modifier::BOLD`
  - `test_parse_inline_spans_italic()` — input: `"foo *bar* baz"`, assert span 1 has `Modifier::ITALIC`
  - `test_parse_inline_spans_code()` — input: "foo \`bar\` baz", assert span 1 has `Modifier::REVERSED`
  - `test_parse_inline_spans_no_final()` — `is_final: false` line renders as single raw span
  - `test_format_token_count_below_threshold()` — `format_token_count(999)` == `"999"`
  - `test_format_token_count_above_threshold()` — `format_token_count(1243)` == `"1.2k"`

---

## Sources

### Primary (HIGH confidence)
- Direct code analysis: `src/tui/widgets.rs` (527 lines, read in full, 2026-04-01)
- Direct code analysis: `src/tui/app.rs` (lines 218–480, read 2026-04-01)
- Direct code analysis: `src/io/agent_io.rs` (108 lines, read in full, 2026-04-01)
- Direct code analysis: `src/llm/types.rs` (Usage struct, lines 135-152, 2026-04-01)
- Cargo.lock version confirmation: pulldown-cmark 0.12.2, ratatui 0.29.0, crossterm 0.28.1 (2026-04-01)
- `.planning/research/ARCHITECTURE.md` — integration points map (HIGH confidence, source-grounded)
- `.planning/research/PITFALLS.md` — 14 identified pitfalls (HIGH confidence, source-grounded)

### Secondary (MEDIUM confidence)
- pulldown-cmark 0.12 API — `TagEnd` enum introduced as breaking change from 0.11; confirmed against locked version 0.12.2 in Cargo.lock
- ratatui `Modifier::REVERSED` for inline code — helix-editor convention; verified as valid ratatui modifier

### Tertiary (LOW confidence)
- None — all claims are grounded in the locked dependency versions and live source.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions locked in Cargo.lock, build verified clean
- Architecture: HIGH — all patterns derived from direct source code analysis
- Pitfalls: HIGH — grounded in actual code behavior observed in widgets.rs/app.rs

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable dependencies, no fast-moving ecosystem concerns for Phase 1)

## Project Constraints (from CLAUDE.md)

The following directives from the user's global `~/.claude/CLAUDE.md` apply to this phase:

| Directive | Impact on Phase 1 |
|-----------|-------------------|
| Read relevant files before making changes — never speculate | All source files read before this document was written |
| Keep changes simple and minimal — avoid over-engineering | Phase 1 is four targeted changes; no new crates, no new files |
| Build and test after major code changes | `cargo build` must be clean after each task; test gaps documented in Wave 0 Gaps |
| All files must contain copyright header | `widgets.rs` and `app.rs` already have headers; any new test module needs the header |
| Save plan files for safety | RESEARCH.md is the plan artifact; written to `.planning/phases/01-foundation/` |
| Always build an extensive plan, save to `.plans` folder | Plans go in `.planning/phases/01-foundation/`; no separate `.plans` folder needed (project already uses `.planning/`) |
| After any UI or frontend change, always run the build | After each widgets.rs / app.rs change: `cargo build` before commit |
| Reference CONTRIBUTING.md and adhere to it | Read CONTRIBUTING.md before implementation tasks if present |
