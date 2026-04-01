// Fin — TUI v1.1 Stack Research
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Technology Stack: Fin TUI v1.1 Enhancement

**Project:** Fin (Rust AI coding agent TUI)
**Researched:** 2026-04-01
**Scope:** Stack additions/changes for v1.1 features ONLY — existing ratatui/crossterm/workflow stack not re-evaluated.

---

## Research Baseline

Before adding anything, what does the project already have that directly applies to v1.1?

| Already Present | Applies To |
|-----------------|------------|
| `ratatui 0.29.0` | All TUI rendering — Clear widget, Layout helpers, Span/Style system |
| `crossterm 0.28.1` | Terminal event loop (no changes needed) |
| `pulldown-cmark 0.12.2` | Already in Cargo.toml — usable for inline span parsing |
| `chrono 0.4` | Timestamps for toast auto-dismiss |
| `tokio` (full) | Async timing for toast TTL, already present |
| `Span::styled()` + `Style::bold()`/`.italic()` | Ratatui native bold/italic rendering |

**Key observation:** pulldown-cmark is already a dependency. The project is not zero-cost on markdown parsing — it is already paying the binary cost. Using it for inline span rendering adds zero crate overhead.

---

## Feature-by-Feature Stack Analysis

### 1. Auto-Run Panel Expansion

**What it needs:** Extra rows showing blueprint id/name, current action, last git commit, context memory %, keybind hints during auto mode.

**Verdict: No new crates required.**

The existing `WorkflowState` struct in `widgets.rs` already tracks `blueprint_id`, `current_stage`, `current_section`, `current_task`. The panel expansion is purely a layout change — add `Constraint::Length(N)` rows to the existing `render_workflow_panel()` Layout, conditionally shown when `auto_mode == true`.

Git commit display: `git2 0.19.0` is already present. `repo.head()?.peel_to_commit()?.summary()` gives the last commit message — no new crate.

Memory %: This is a computed value from existing session token tracking (`tokens_in`/`tokens_out` already in `render_status_bar`). Wire the context window limit from the model registry into `WorkflowState` or pass it as a parameter.

**Integration point:** `render_workflow_panel(f, area, state, auto_mode: bool)` — add a bool parameter to switch between compact (3-line) and expanded (6-8 line) rendering.

---

### 2. Toast / Ephemeral Notification Overlays

**What it needs:** Banners that appear, display a message, and auto-dismiss after N seconds.

**Verdict: No new crates required. Implement with ratatui primitives + `std::time::Instant`.**

Ratatui 0.29 provides:
- `Clear` widget — clears a screen region before overdrawing (confirmed in official docs)
- `Block::bordered()` + `Paragraph` — renders the toast frame and message
- `Rect` helpers (`offset()`, `inner()`) — positions the toast at the bottom-right or top-right of the output area

Pattern (verified from ratatui official popup example):
```
frame.render_widget(Clear, toast_rect);
frame.render_widget(toast_paragraph, toast_rect);
```

**Toast state:** A `Vec<Toast>` in `AppState`, where each `Toast` holds:
- `message: String`
- `level: ToastLevel` (Info/Warn/Error/Success)
- `created_at: std::time::Instant`
- `ttl: Duration`

On each render tick, drain expired toasts from the vec (`created_at.elapsed() > ttl`). No external timer crate needed — the existing main loop tick drives this.

**Hook into `AgentEvent`:** The `AgentEvent` enum already has `ToolStart`, `ToolEnd`, `WorkflowUnitStart`, `WorkflowUnitEnd`, `WorkflowError`. The TUI event handler in `app.rs` already processes these — add toast push calls alongside the existing line-append logic.

**Why not `ratkit`/`ratatui-toolkit`?** ratkit 0.2.12 (~1.5MB of source, 30K SLoC, tokio full features which are already present but it also pulls `itertools`, `tracing` variants, and optional `syntect`/`ansi-to-tui`/`reqwest`). Its toast widget adds negligible functionality over a 40-line custom struct, at the cost of a moderately-sized dependency. Binary size constraint is 3.7MB stripped — ratkit's transitive tree is documented at 14-36MB SLoC. Avoid. (MEDIUM confidence — size estimate from lib.rs docs page, not a direct stripped binary measurement.)

---

### 3. Inline Markdown Span Rendering (bold, italic)

**What it needs:** Render `**bold**` and `*italic*` spans inside `LineKind::Assistant` output lines without showing raw asterisks.

**Verdict: Use pulldown-cmark's existing dependency. No new crate.**

pulldown-cmark 0.12.2 is already in `Cargo.toml` (currently unused in `src/` — confirmed by grep). The binary is already paying its cost (~190KB of the stripped binary per the Cargo.lock transitive chain). Using it adds zero additional binary size.

pulldown-cmark's `Parser` is an iterator over `Event` enums. For a single line of assistant text, the relevant events are:
- `Event::Start(Tag::Strong)` → push bold style
- `Event::Start(Tag::Emphasis)` → push italic style
- `Event::End(Tag::Strong | Tag::Emphasis)` → pop style
- `Event::Text(s)` → emit `Span::styled(s, current_style)`

This produces a `Vec<Span>` which becomes a `Line::from(spans)` — exactly how the rest of `widgets.rs` already works.

**Implementation shape:** A free function `parse_inline_spans(text: &str) -> Vec<Span<'static>>` that iterates one pulldown-cmark parse pass over the text and accumulates spans. Called from the `LineKind::Assistant` branch in `render_output()` instead of the current whole-line style application.

**Why not `tui-markdown`?** tui-markdown 0.3.7 adds pulldown-cmark 0.13 (version bump from 0.12 already present), `itertools 0.14`, and `tracing`. It also targets full document rendering (headers, fenced code blocks, tables) — overkill for the two inline patterns needed. It would also add a version conflict on pulldown-cmark (0.12 vs 0.13) requiring a Cargo.toml bump. The inline span logic is ~30 lines of code. (HIGH confidence — examined tui-markdown Cargo.toml directly.)

**pulldown-cmark version note:** The project currently uses 0.12.2. tui-markdown requires 0.13. If tui-markdown is ever wanted later, bumping pulldown-cmark to 0.13 is straightforward — the Event API is stable. For now, 0.12.2 covers everything needed.

---

### 4. Keybindings Help Overlay (`?` key)

**What it needs:** A modal overlay listing keybindings and slash commands, shown on `?` keypress, dismissed on any key.

**Verdict: No new crates required. Ratatui `Clear` + `Block` + `Paragraph` + `Table` widget.**

Pattern is identical to the toast overlay approach: compute a centered `Rect`, render `Clear`, then render the help content. The content is a static `Table` or multi-column `Paragraph`.

Ratatui's `Table` widget (already part of `ratatui 0.29`) renders columns cleanly. Two columns (keybind | description) separated by spaces. Alternatively, `Paragraph::new(Text::from_iter(lines))` with pre-formatted strings is simpler and more predictable in narrow terminals.

`Rect::inner()` with a `Margin` provides padding inside the block border. Centering is done manually with saturating arithmetic (same pattern already used in `render_splash()` in `widgets.rs`).

**State:** `show_help: bool` field in `AppState`. Toggle on `?`, dismiss on any key (check in the existing crossterm event loop).

---

### 5. Token / Cost Per-Message Tracking Display

**What it needs:** Show tokens and cost at the per-message level, or a cleaner session summary.

**Verdict: No new crates required.**

The `AgentEvent::AgentEnd { usage: Usage }` event and `AgentEvent::TurnEnd` already fire. The `Usage` struct (from `src/llm/types.rs`) holds `input_tokens` and `output_tokens`. The status bar already renders session totals.

Per-message tracking requires storing a `Vec<MessageCost>` where each entry records turn-level input/output tokens. `bpe-openai 0.3.0` is already present for token counting. Cost computation is a pure function against the model's pricing table (already exists in the model registry).

Display options (all zero-crate):
- Append a `LineKind::System` line after each `TurnEnd` event showing `[in: N, out: N, $0.0042]`
- Extend the status bar format string
- Add a row to the toggle-able side panel (see feature 7)

---

### 6. Visual Theme Consistency (Color Palette System)

**What it needs:** A unified color palette so all widgets use consistent colors instead of scattered `Color::Cyan` literals.

**Verdict: No new crates required. Define a `Palette` const struct in `widgets.rs`.**

ratatui 0.29 has `ratatui::style::palette::tailwind` and `ratatui::style::palette::material` built in as constants (confirmed via official docs). These are compile-time `const` color arrays — they add zero binary size beyond what's already in ratatui.

**Recommended pattern:** Define a `pub struct Palette` in `widgets.rs` (or a new `src/tui/theme.rs` module) with named semantic fields:

```rust
pub struct Palette;
impl Palette {
    pub const ACCENT:    Color = Color::Cyan;
    pub const MUTED:     Color = Color::DarkGray;
    pub const SUCCESS:   Color = Color::Green;
    pub const ERROR:     Color = Color::Red;
    pub const WARNING:   Color = Color::Yellow;
    pub const TEXT:      Color = Color::White;
    pub const BORDER:    Color = Color::DarkGray;
    pub const HIGHLIGHT: Color = Color::White;
}
```

Replace all scattered `Color::Cyan`, `Color::DarkGray` literals in `widgets.rs` with `Palette::ACCENT`, `Palette::MUTED`, etc. This is a pure refactor — no behavior change, no binary impact, consistent rendering guaranteed.

**Why not `ratatui-themes`?** ratatui-themes is a community crate offering 15+ themes (Dracula, Nord, Catppuccin, etc.). Fin has no theme-switching requirement — PROJECT.md specifies a "consistency pass," not a user-selectable theme system. A const struct costs nothing.

---

### 7. Toggle-able Side Info Panel (Ctrl+P)

**What it needs:** A collapsible panel showing model, tokens, workflow state — toggled with Ctrl+P.

**Verdict: No new crates required. Ratatui conditional `Layout` column.**

The existing layout in `app.rs` renders a vertical stack (output / workflow panel / input / status). Adding a side panel is a conditional horizontal split:

```
if state.side_panel_open {
    Layout::horizontal([Constraint::Min(0), Constraint::Length(28)])
} else {
    Layout::horizontal([Constraint::Percentage(100)])
}
```

The side panel renders in the right column using existing `Paragraph` + `Block` widgets. Content: model name, provider, session tokens, cost, current workflow state — all data already present in `AppState`.

**80-column constraint:** At 80 columns, a 28-char side panel leaves 52 chars for output — workable but tight. The PROJECT.md explicitly requires the panel to be "toggleable" and default-off. Document the minimum recommended width as 100 columns when the panel is open.

---

## Final Dependency Decision Table

| Feature | New Crate? | Rationale | Binary Impact |
|---------|-----------|-----------|---------------|
| Auto-run panel expansion | None | Layout/data already present | Zero |
| Toast overlays | None | `Clear` + `Instant` + existing event loop | Zero |
| Inline bold/italic | None | pulldown-cmark already in Cargo.toml | Zero (already paid) |
| Help overlay | None | `Clear` + `Block` + `Paragraph` | Zero |
| Token/cost per-message | None | Usage struct + bpe-openai already present | Zero |
| Color palette system | None | Const struct, ratatui built-in palette | Zero |
| Side panel | None | Conditional Layout split | Zero |

**Net new crates for v1.1: zero.**

All seven v1.1 features are buildable with ratatui 0.29 primitives and already-present dependencies. No Cargo.toml changes required. Binary size target of 3.7MB stripped is unaffected.

---

## What Was Explicitly Rejected

| Considered | Reason Rejected |
|------------|-----------------|
| `ratkit 0.2.12` (toast widget) | ~1.5MB source, 30K SLoC, adds itertools/ansi-to-tui/notify. Toast is 40 lines of custom code. |
| `tui-markdown 0.3.7` | Pulls pulldown-cmark 0.13 (version conflict), itertools, tracing. Only need 2 inline patterns. ~30 lines custom. |
| `ratatui-themes` | Theme switching not in scope. const struct is sufficient. |
| `markdown-rs` | No advantage over pulldown-cmark already present. Would add a second markdown parser dependency. |

---

## Integration Points Summary

| Module | Change |
|--------|--------|
| `src/tui/widgets.rs` | Add `parse_inline_spans()`, `render_toast()`, `render_help_overlay()`, `render_side_panel()`, `Palette` const struct; extend `render_workflow_panel()` with `auto_mode` bool |
| `src/tui/app.rs` | Add `show_help: bool`, `side_panel_open: bool`, `toasts: Vec<Toast>` to AppState; handle `?` key and `Ctrl+P`; push toasts on relevant `AgentEvent` variants; drain expired toasts each tick |
| `src/io/agent_io.rs` | No change — existing events are sufficient |
| `Cargo.toml` | No changes required |

---

## Confidence Assessment

| Area | Confidence | Source |
|------|------------|--------|
| ratatui `Clear` widget + overlay pattern | HIGH | Official ratatui docs + popup example |
| `Rect` helpers (inner, offset) in 0.29 | HIGH | docs.rs/ratatui/0.29.0/ratatui/layout/struct.Rect |
| pulldown-cmark already in Cargo.toml (unused) | HIGH | Grep of src/ confirmed no imports; Cargo.toml confirmed present |
| tui-markdown dependency list | HIGH | Examined raw Cargo.toml from GitHub directly |
| ratkit binary/dependency weight | MEDIUM | lib.rs docs page estimate; not a measured stripped binary |
| ratatui built-in palette (tailwind/material) | HIGH | Official ratatui style::palette docs |
| Binary size staying at 3.7MB | MEDIUM | No crates added; profile settings unchanged; not measured against v1.1 code |

---

## Sources

- [ratatui Popup Example](https://ratatui.rs/examples/apps/popup/)
- [Clear widget — docs.rs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Clear.html)
- [Ratatui Overwrite Regions Recipe](https://ratatui.rs/recipes/render/overwrite-regions/)
- [ratatui::style::palette — docs.rs](https://docs.rs/ratatui/latest/ratatui/style/palette/index.html)
- [Rect — docs.rs ratatui 0.29.0](https://docs.rs/ratatui/0.29.0/ratatui/layout/struct.Rect.html)
- [tui-markdown Cargo.toml (GitHub raw)](https://raw.githubusercontent.com/joshka/tui-markdown/main/tui-markdown/Cargo.toml)
- [ratkit on lib.rs](https://lib.rs/crates/ratkit)
- [Comprehensive terminal markdown rendering research gist](https://gist.github.com/nelson-ddatalabs/21290f85c8bd13bb56676560c114980d)
- [pulldown-cmark docs.rs](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
