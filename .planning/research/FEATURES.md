// Fin — TUI Enhancement v1.1 Research: Feature Landscape
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Feature Landscape: Fin TUI Enhancement v1.1

**Domain:** Terminal UI for an autonomous AI coding agent (Rust, ratatui/crossterm)
**Researched:** 2026-04-01
**Scope:** Six new features only — existing features (splash, output area, status bar,
workflow pipeline panel, model picker, session history replay) are already shipped.

---

## Existing System Anchors (Do Not Re-Architect)

Before classifying new features, these constraints from the live codebase shape every
decision below:

- `WorkflowState` struct already carries: `blueprint_id`, `current_stage`,
  `current_section`, `current_task`, `sections_total/done`, `tasks_total/done`,
  `stage_pipeline`. The auto-run panel is an extension of `render_workflow_panel()`.
- `LineKind` enum (Assistant, User, Thinking, Tool, ToolResult, Error, System) drives
  all output rendering. Markdown spans live inside `LineKind::Assistant` lines only.
- `AgentEvent` mpsc channel is the event bus. Toast hooks directly into this stream —
  no new channel needed.
- Status bar already shows model/tokens/cost as a single formatted string. Token/cost
  improvements are a formatting change, not a data-pipeline change.
- Binary size constraint: 3.7MB stripped. No large parser crates.

---

## Table Stakes

Features users expect once they see them in comparable tools. Missing = the TUI feels
incomplete relative to peers (aider, opencode, conduit, lazygit).

### 1. Auto-Run Live Panel (expanded workflow widget)

**What users expect:** When the agent is running autonomously, a dedicated widget that
shows what is happening right now — not just the stage name, but the specific action,
which blueprint, git/memory context, and escape hints. Lazygit, bottom (btm), and
aider all show a continuously-updating status area during long-running operations.

**Concrete behavior in comparable tools:**
- lazygit: top-left status panel always reflects current repo state; updates live.
- bottom (btm): widget panel refreshes on a tick (100-250ms) with current readings.
- aider/opencode: a status line beneath the chat output shows current operation.
- GSD reference (from PROJECT.md): 6-row widget — status line, blueprint+model,
  stage/action, separator, progress bar, footer with git+memory+key hints.

**Expected rows for Fin's widget (extending existing 3-line panel):**
```
┌─ blueprint-id ──────────── Stage: Build ─┐
│ Model: claude-sonnet-4-6                 │
│ Action: Writing src/workflow/phases/build│
│ ─────────────────────────────────────── │
│ ████████░░░░░░░░  12/28 tasks            │
│ git: abc1234  mem: 42%  [Esc] cancel     │
└──────────────────────────────────────────┘
```

**Complexity:** Low-Medium. The data already exists in `WorkflowState`. This is a
rendering change: extend `render_workflow_panel()` to accept an `AutoRunInfo` struct
with model name, current action description, last git short-hash, and memory %. The
layout splits from 2 inner rows to 4-6 rows when `auto_mode == true`.

**Dependencies on existing system:**
- `WorkflowState` (already has most fields)
- Auto-loop state from `src/workflow/auto_loop.rs` (needs to surface last action string
  and git commit short hash)
- Memory % requires a context-window-usage counter to be threaded through

**"Done" definition:**
- Panel shows all 6 rows only in auto mode; collapses back to existing 3-line form
  in interactive mode (no layout shift during normal chat)
- Git short hash updates on each tool call completion
- Memory % is within ±5% of actual context usage
- [Esc] cancel hint is functional — pressing Esc during auto-run halts the loop

---

### 2. Toast / Ephemeral Notification System

**What users expect:** Brief, auto-dismissing banners that surface important events
without interrupting the scrollable output. This is universal in modern TUIs — helix
shows mode transitions, lazygit shows operation results, bottom shows alerts.

**Concrete behavior in comparable tools:**
- Standard positioning: bottom-right corner or top-right corner, stacked vertically.
  Bottom-right is the most common in terminal apps (lazygit, helix status messages
  appear bottom-left but in a dedicated bar — for non-status-bar toasts, bottom-right
  is the convention).
- Timing: 2-4 seconds for info/success, 5-8 seconds for warnings, errors stay until
  dismissed with a keypress. The ratkit/ratatui-toolkit crate uses 3s default.
- Stacking: up to 3-5 toasts visible at once, LIFO (newest on top). Older toasts
  shift down as new ones arrive. Max stack of 5 is the ratatui-toolkit default.
- Levels: info (cyan/white), success (green), warning (yellow), error (red bold).
- Width: fixed, typically 30-50 columns, never wider than half the terminal.

**Rendering pattern (ratatui):**
```
// Per frame:
// 1. Render all normal layout widgets
// 2. Calculate toast Rects in bottom-right corner
// 3. render_widget(Clear, toast_rect)  // prevent bleed-through
// 4. render_widget(toast_block, toast_rect)
// 5. Tick toast timers — remove expired entries from Vec<Toast>
```
The `Clear` widget is the critical step. Without it, styled cells from the output
area bleed into the toast overlay.

**Event sources (hook into AgentEvent):**
- Tool start/completion events → brief info toast ("Read: src/main.rs")
- Stage transitions → success toast ("Build complete — moving to Validate")
- Errors → error toast (stays until dismissed)
- Auto-loop completion → success toast ("Workflow complete — 28 tasks")

**Anti-pattern to avoid:** Emitting a toast for every single tool call. Tool events
are high-frequency. Toast only for: stage transitions, errors, loop start/stop,
and notable tool milestones (git commit, validation pass/fail).

**Complexity:** Low-Medium. No external crate needed — implement a `ToastQueue`
struct with a `Vec<(Instant, ToastLevel, String)>` and render on each frame tick.
The ratatui `Clear` + overlay pattern is well-documented and straightforward.

**Dependencies on existing system:**
- `AgentEvent` enum — add variants or match on existing ones for toast triggers
- Frame render loop in `src/tui/app.rs` — add toast render pass after main layout

**"Done" definition:**
- Info toasts auto-dismiss after 3s, error toasts persist until Esc
- No more than 5 toasts stacked at once (oldest drops off)
- Toasts never obscure the input area
- Stage transition events always produce a toast regardless of noise-filtering

---

### 3. Inline Markdown Bold/Italic Rendering

**What users expect:** Assistant text with `**bold**` and `*italic*` renders with
terminal styling, not raw asterisks. Every AI chat TUI that renders assistant output
does this — opencode, conduit, aider's TUI mode all apply inline formatting.

**Concrete behavior:**
- `**text**` or `__text__` → rendered with `Modifier::BOLD`
- `*text*` or `_text_` → rendered with `Modifier::ITALIC`
- Backtick inline code → rendered with a distinct color (e.g. `Color::Yellow` or
  `Color::Cyan` on a slightly dimmed background — helix-style inline code)
- Line-level constructs (headers, bullets, code fences) already handled in
  `render_output()` — this extends only to span-level within a line of text

**Implementation approach (constrained by binary size):**
Do NOT add `pulldown-cmark` or `tui-markdown`. Those crates pull in `syntect` for
code highlighting, adding significant binary weight. The required patterns are simple
enough for a hand-written span parser:

```rust
fn parse_inline_markdown(text: &str, base_style: Style) -> Vec<Span<'static>> {
    // Walk text, detect **...**, *...*, `...`
    // Emit Span with appropriate Modifier
    // Handle nesting: **bold with *italic* inside** is edge case, skip it
}
```

A state-machine scanner over the line string produces a `Vec<Span>` replacing the
current `Line::styled(text, style)` call for `LineKind::Assistant`.

**Edge cases to handle explicitly (not silently corrupt):**
- Unmatched `*` or `**` — emit as literal text, do not corrupt remainder of line
- `**` at end of line with no closing — emit as literal
- Escaped asterisks `\*` — not required for v1.1, skip/emit literally
- Multi-line bold/italic — not supported (lines are already split at newlines before
  reaching `render_output()`); single-line only

**Complexity:** Low. ~50-80 lines of Rust in `widgets.rs`. No new dependencies.

**Dependencies on existing system:**
- `render_output()` in `widgets.rs` — replace `LineKind::Assistant` branch's
  `Line::styled()` with a call to `parse_inline_markdown()`
- `OutputLine` struct — no changes needed

**"Done" definition:**
- `**bold**` and `*italic*` render styled in assistant output
- Backtick code spans render with a distinct color
- Malformed markdown (unmatched delimiters) emits literal text without panicking
- No binary size regression (verified with `cargo build --release` + `du -sh`)

---

### 4. Keybindings Help Overlay (`?` key)

**What users expect:** Pressing `?` (or `F1`) shows a full list of keys and slash
commands. This is universal — lazygit (`?` key), gitui (`F1`), helix (`space-?`),
bottom (`?`), neovim (`:help`) all provide this. Users discovering a new TUI tool
immediately reach for `?`.

**Concrete behavior in comparable tools:**
- lazygit: `?` opens a full-screen overlay listing all keybindings for the focused
  panel, grouped by category. Press any key to dismiss.
- gitui: `F1` opens a keybindings popup. Scrollable with arrow keys. Esc to close.
- helix: `space-?` opens a which-key popup showing available bindings.
- bottom: `?` opens a help dialog, press `q` or Esc to close.

**Design for Fin:**
- Full-screen overlay (not partial modal) — takes the entire terminal area
- Two-column layout: left column = keybindings, right column = slash commands
- Sections: Navigation, Workflow, Session, Auto-run, Meta
- Rendering: `Clear` the full area, render a `Block` with `Borders::ALL`, then
  a `Paragraph` with formatted `Line` entries using `Span` styling
- Dismiss: any key press closes the overlay (no specific key required)
- Static content: the keybinding list is a compile-time constant `&[(&str, &str)]`
  — no dynamic discovery needed for v1.1

**Complexity:** Low. This is purely a render pass. Add a `show_help: bool` flag to
app state, toggle on `?`, render the overlay when true, dismiss on any keypress.

**Dependencies on existing system:**
- `src/tui/app.rs` — add `show_help` to `AppState`, handle `?` keypress, add render
  branch in the main render loop
- `src/tui/widgets.rs` — add `render_help_overlay(f, area, keybinds, slash_cmds)`

**"Done" definition:**
- `?` key toggles a full-screen keybindings overlay
- Overlay shows all current keybindings and slash commands (exhaustive, not a sample)
- Any keypress dismisses it (including `?` again, Esc, q, Enter)
- Overlay is readable on an 80-column terminal

---

### 5. Per-Message Token/Cost Display

**What users expect:** Seeing how many tokens and how much each response costs is
standard in AI TUI tools. Opencode shows cumulative cost after each response. Conduit
tracks input/output tokens in real time. Tokscale exists specifically because users
want per-session and per-message tracking.

**Current state in Fin:** Status bar shows session-level cumulative totals
(`in:{tokens_in} out:{tokens_out} | ${cost:.4}`). This is table stakes but the
display is dense and unformatted.

**What "improvement" means for v1.1 (from PROJECT.md):**
> "per-message tracking or cleaner session summary"

**Recommendation — do both at low cost:**

Option A: Per-message inline annotation (differentiator, medium complexity)
- After each assistant response completes streaming, append a dim system line:
  `↳ 1,234 in / 456 out  $0.0087`
- Uses existing `LineKind::System` (already dim/gray styled) — no new LineKind needed
- Delta calculation: record token counts before and after each LLM call, emit the
  difference as a System line

Option B: Cleaner status bar formatting (table stakes, low complexity)
- Current: ` claude-sonnet-4-6 | streaming... | in:12453 out:3421 | $0.1234`
- Improved: ` claude-sonnet-4-6  ↑12.4k  ↓3.4k  $0.12  [streaming]`
- Tighter, uses k-suffix for large numbers, brackets streaming state

**Recommendation:** Implement B first (trivial), then A in the same pass (low effort,
high value). Together they define "done" for this feature.

**Complexity:** Low (B alone) / Low-Medium (A+B together).

**Dependencies on existing system:**
- Status bar: purely a string formatting change in `render_status_bar()`
- Per-message annotation: requires delta tracking in the agent event handler;
  token counts must be captured at start and end of each streaming response

**"Done" definition:**
- Status bar shows tighter, k-suffixed token counts
- Each completed assistant message is followed by a dim cost annotation line
- Session total remains accessible (status bar or side panel)
- No regression in streaming display responsiveness

---

## Differentiators

Features that go beyond what users expect from a basic AI coding TUI. Not required for
"complete," but they differentiate Fin from aider/opencode at the TUI layer.

### 6. Toggle-able Side Info Panel (Ctrl+P)

**What makes this a differentiator:** Most AI coding TUIs do not have a persistent
side panel. bottom has toggleable widget panels; lazygit has fixed panes. For an
autonomous coding agent, a side panel showing live model config, token budget, and
workflow state in a glanceable format is genuinely novel.

**Concrete behavior:**
- Default: off. Full terminal width for output area (preserves current layout).
- Ctrl+P: toggles panel on. Main area shrinks to ~70%, side panel takes ~30%.
- Panel contents (static snapshot, updates each render tick):
  - Model: claude-sonnet-4-6
  - Provider: Anthropic
  - Session tokens: 45,234 in / 12,891 out
  - Session cost: $0.47
  - Workflow: Build — section 3/8, task 12/28
  - Memory: 42% (context window fill)
  - Last commit: abc1234 "feat: implement builder"
- Must work at 80 columns: at minimum terminal width, 80/30 split means the side
  panel gets ~24 columns. Truncate gracefully with `..` suffix.

**Layout implementation:**
```rust
let constraints = if app.side_panel_visible {
    vec![Constraint::Min(50), Constraint::Length(30)]
} else {
    vec![Constraint::Percentage(100)]
};
let h_layout = Layout::horizontal(constraints).split(main_area);
```
Toggle by flipping `side_panel_visible: bool` in app state on Ctrl+P.

**Complexity:** Medium. The layout change is straightforward. The complexity is in
deciding which data to surface and ensuring graceful truncation at narrow widths.
The panel must not cause layout panics when terminal is resized below its minimum.

**Dependencies on existing system:**
- `WorkflowState` — already has section/task counts
- Status bar token/cost values — reuse same data
- Git short hash — same data as auto-run panel
- Memory % — same new counter needed for auto-run panel (single implementation)

**"Done" definition:**
- Ctrl+P toggles panel with no visual artifact (no leftover cells — use `Clear`)
- Panel is readable at 80 columns (no panic, no overflow)
- All listed fields populated when workflow is active; graceful placeholder text
  when no workflow is running ("No active workflow")
- Panel state persists for the session (not reset on model switch etc.)

---

## Anti-Features

Explicitly excluded. Building any of these would be a mistake for v1.1.

### Full Markdown Parser Crate

**Anti-feature:** Adding `tui-markdown`, `pulldown-cmark` + `syntect`, or `ratskin`
as dependencies to get "complete" markdown rendering.

**Why avoid:** Binary size constraint is real. `pulldown-cmark` itself is modest
(~150KB compiled), but it drags in `syntect` when used via `tui-markdown` for code
highlighting — syntect adds ~1-2MB to binary size. Going from 3.7MB to 5MB+ violates
a core constraint. The two patterns needed (bold, italic) take ~70 lines of Rust.

**What to do instead:** Hand-written span parser. Covers the 95% case (bold, italic,
inline code) at zero dependency cost.

---

### Mouse Support for Any Widget

**Anti-feature:** Adding click-to-dismiss on toasts, click-to-toggle side panel,
or drag-to-resize panels via mouse events.

**Why avoid:** PROJECT.md explicitly excludes mouse support. More importantly,
capturing mouse events in crossterm (`EnableMouseCapture`) breaks native terminal
text selection. Users copy output from the TUI — this is critical for an AI coding
agent. `tui-popup` supports mouse dragging but it requires `EnableMouseCapture`.

**What to do instead:** All interactions keyboard-only. Esc dismisses, Ctrl+P toggles,
`?` shows help. This is the right default for a developer tool.

---

### Toast for Every Tool Call

**Anti-feature:** Emitting a toast notification for each individual tool invocation
(read file, grep, bash, edit).

**Why avoid:** The agent can make 50-100+ tool calls per workflow phase. Constant
toasts would bury meaningful information in noise, overflow the toast queue, and
distract from the output area which already shows tool events via `LineKind::Tool`.

**What to do instead:** Toast only for high-signal events: stage transitions, errors,
loop completion, git commits, validation results. Tool-level detail lives in the
scrollable output.

---

### Animated Spinners / ASCII Progress Animations

**Anti-feature:** Braille spinner or animated progress characters that cycle on
every tick for "liveness."

**Why avoid:** The existing `●` (active stage icon) already communicates liveness.
Animations require a dedicated tick rate (typically 10Hz+), increase render
complexity, and provide no additional information. bottom and lazygit use static
icons, not animations, for primary status.

**What to do instead:** The progress bar (`████░░░░`) updates with real task counts.
That is sufficient liveness feedback.

---

### Markdown Table Rendering

**Anti-feature:** Rendering full markdown tables (`| col | col |`) in assistant
output with aligned columns.

**Why avoid:** Table width depends on terminal width. Calculating column widths
requires buffering the entire table before rendering (multiple passes), word-wrap
breaks table structure, and narrow terminals (80 col) make most tables unreadable.
Full-width tables in ratatui require a dedicated `Table` widget with column width
logic.

**What to do instead:** Render table rows as plain text (already happens with current
code). The output area already wraps lines gracefully.

---

## Feature Dependencies

```
Auto-run panel
  └── requires: memory % counter (new, shared with side panel)
  └── requires: last git short hash (new, shared with side panel)
  └── extends: WorkflowState (existing)

Toast system
  └── hooks into: AgentEvent (existing)
  └── uses: Clear widget pattern (ratatui built-in)

Inline markdown
  └── replaces: LineKind::Assistant branch in render_output() (existing)
  └── no new dependencies

Keybindings overlay
  └── adds: show_help flag to AppState
  └── uses: Clear + Block + Paragraph (all existing)

Per-message token display
  └── extends: status bar format string (existing)
  └── adds: delta tracking in agent event handler (new, small)

Side panel
  └── shares: memory % counter (auto-run panel)
  └── shares: git short hash (auto-run panel)
  └── adds: side_panel_visible flag to AppState
  └── modifies: main horizontal layout constraints
```

**Recommended implementation order:**
1. Inline markdown (zero dependencies, isolated, immediate visible payoff)
2. Toast system (low complexity, high user value, enables feedback for other work)
3. Keybindings overlay (isolated, low complexity, good before shipping other features)
4. Per-message token display (data infrastructure for item 5 side panel)
5. Auto-run panel (extends existing widget, needs memory % + git hash infrastructure)
6. Side panel (depends on memory % and git hash from item 5; most layout complexity)

---

## MVP Recommendation

For a shippable v1.1, items 1-4 (inline markdown, toasts, help overlay, token display)
form a coherent "polish pass" that is low risk and high visibility. Each is isolated.

Items 5-6 (auto-run panel expansion and side panel) share infrastructure (memory %
counter, git short hash) and should ship together as a second sub-milestone within
v1.1 to avoid two passes at the same plumbing.

**Ship together as wave 1:** inline markdown, toasts, help overlay, token display
**Ship together as wave 2:** auto-run panel + side panel (shared infra)

---

## Sources

- [Ratatui — Overwrite Regions / Clear widget](https://ratatui.rs/recipes/render/overwrite-regions/)
- [Ratatui — Popup example](https://ratatui.rs/examples/apps/popup/)
- [Ratatui — Constraints / Layout](https://ratatui.rs/concepts/layout/)
- [tui-popup crate (docs.rs)](https://docs.rs/tui-popup)
- [ratatui-toolkit → ratkit (lib.rs)](https://lib.rs/crates/ratatui-toolkit)
- [pulldown-cmark (crates.io)](https://crates.io/crates/pulldown-cmark)
- [tui-markdown (docs.rs)](https://docs.rs/tui-markdown)
- [ratskin — termimad adapter for ratatui](https://docs.rs/ratskin/latest/ratskin/)
- [lazygit status panel deep dive](https://www.oliverguenther.de/2021/04/lazygit-status-panel-deep-dive/)
- [lazygit help bar toggle discussion](https://github.com/jesseduffield/lazygit/discussions/1606)
- [gitui — KEY_CONFIG.md](https://github.com/gitui-org/gitui/blob/master/KEY_CONFIG.md)
- [bottom (btm) — GitHub](https://github.com/ClementTsang/bottom)
- [Ratatui TUI keymap discussion #627](https://github.com/ratatui/ratatui/discussions/627)
- [OpenCode TUI docs](https://opencode.ai/docs/tui/)
- [AI CLI coding agents comparison 2026](https://sanj.dev/post/comparing-ai-cli-coding-assistants)
- [Conduit — multi-agent TUI](https://getconduit.sh/)
- [Tokscale — token usage tracker](https://github.com/junhoyeo/tokscale)
- [Comprehensive ratatui markdown rendering research](https://gist.github.com/nelson-ddatalabs/21290f85c8bd13bb56676560c114980d)
