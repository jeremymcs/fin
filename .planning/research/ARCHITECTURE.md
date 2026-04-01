# Architecture Patterns — Fin v1.1 TUI Enhancement

**Project:** Fin
**Researched:** 2026-04-01
**Confidence:** HIGH — based on direct source analysis of app.rs, widgets.rs, agent_io.rs, auto_loop.rs, tui_io.rs

---

## Existing Architecture Map

### Data Flow (current)

```
auto_loop.rs ──emit()──> AgentEvent ──mpsc──> app.rs event loop
                                                    │
                                             match evt branch
                                                    │
                                       mutates app-local state vars
                                                    │
                                          terminal.draw() closure
                                                    │
                                         widgets::render_*() calls
```

### State Variables in app.rs (run_tui_loop)

| Variable | Type | Purpose |
|----------|------|---------|
| `output_lines` | `Vec<OutputLine>` | Scrollable conversation history |
| `scroll` | `u16` | Current scroll offset |
| `scroll_pinned` | `bool` | Auto-scroll to bottom flag |
| `is_streaming` | `bool` | Whether agent is actively streaming |
| `workflow_state` | `WorkflowState` | Workflow panel data |
| `show_splash` | `bool` | Splash screen visibility |
| `model_for_display` | `String` | Current model name |
| `total_in/out/cost` | numeric | Session token/cost accumulators |
| `model_picker_active` | `bool` | Model picker overlay visibility |
| `model_picker_index` | `usize` | Model picker selection cursor |

### Layout (current)

```
Without workflow panel:          With workflow panel (wf_active):
┌──────────────────┐             ┌──────────────────┐
│  Output (Min:3)  │             │  Output (Min:3)  │
├──────────────────┤             ├──────────────────┤
│  Spacer (1)      │             │ WorkflowPanel(4) │
├──────────────────┤             ├──────────────────┤
│  StatusBar (1)   │             │  Spacer (1)      │
├──────────────────┤             ├──────────────────┤
│  Input (2)       │             │  StatusBar (1)   │
└──────────────────┘             ├──────────────────┤
                                 │  Input (2)       │
                                 └──────────────────┘
```

### Existing AgentEvent Variants

```rust
// Core streaming
AgentStart, AgentEnd, TurnStart, TurnEnd
TextDelta, ThinkingDelta, ToolStart, ToolEnd, ModelChanged

// Workflow lifecycle
WorkflowUnitStart, WorkflowUnitEnd, WorkflowProgress
WorkflowComplete, WorkflowBlocked, WorkflowError
```

### Overlay Pattern (model picker, established precedent)

The model picker is a floating overlay rendered last in the draw closure using `ratatui::widgets::Clear` to erase behind it. It intercepts keyboard events with a guard at the top of the key match block. This pattern is the canonical approach for new overlays.

---

## Integration Points Per Feature

### Feature 1: Auto-Run Panel Expansion

**What changes:** `WorkflowState` struct + `render_workflow_panel()` in widgets.rs.

**Current state:** The workflow panel shows blueprint_id, current_stage, stage pipeline (✓/●/○), and a progress bar. It renders in 4 lines (border block + 2 inner lines). The panel only appears when `workflow_state.active == true`.

**New fields needed in `WorkflowState` (widgets.rs):**
```rust
pub model_display: String,         // e.g. "claude-opus-4-5"
pub last_commit_sha: String,       // e.g. "a3f9b12" (7-char short SHA)
pub last_commit_msg: String,       // e.g. "feat: implement auth"
pub context_pct: Option<u8>,       // 0-100, None if unknown
pub loop_mode: Option<String>,     // "auto" | "step"
```

**Where fields are populated:**
- `model_display`: set in the `WorkflowUnitStart` handler in app.rs, from `model_for_display`
- `last_commit_sha/msg`: require a new `AgentEvent::WorkflowGitStatus` variant OR read synchronously via `WorkflowGit::last_commit()` — a new `async fn last_commit()` returning `(sha, msg)` in git.rs. The auto_loop `finalize()` function already calls git ops; it can emit a new event after `commit_task` or `commit_artifacts` succeeds.
- `context_pct`: the agent state in run_tui_agent has `state.cumulative_usage` — the TUI cannot read this directly. Options: emit it as a field on `AgentEnd` or add a new `AgentEvent::ContextUsage { pct: u8 }` variant. The latter is cleaner.
- `loop_mode`: set in the `WorkflowUnitStart` handler; the app.rs already knows the loop mode when it calls `run_loop()`.

**render_workflow_panel changes:** The current inner area is 2 lines. Expanding to ~5-6 lines requires increasing `Constraint::Length(4)` in the layout from app.rs (the workflow panel height) to `Constraint::Length(7)` or similar. The inner layout in `render_workflow_panel` grows accordingly.

**Hook locations:**
- `widgets.rs`: `WorkflowState` struct (add fields), `render_workflow_panel()` (add lines)
- `app.rs`: `WorkflowUnitStart` handler (set model_display, loop_mode), layout constraints
- `agent_io.rs`: new variant `WorkflowGitStatus { sha: String, message: String }` (optional but clean)
- `auto_loop.rs`: emit `WorkflowGitStatus` after git commit in `finalize()`
- `git.rs`: new `async fn last_commit(&self) -> (String, String)` helper

**Confidence:** HIGH — all touch points visible in source.

---

### Feature 2: Toast Notification System

**What it is:** Ephemeral banners appearing on top of the output area, auto-dismissing after N milliseconds.

**Integration approach:** Floating overlay, same pattern as model picker. Rendered last in the draw closure over the output area, bottom-aligned.

**New state in app.rs:**
```rust
struct Toast {
    text: String,
    kind: ToastKind,        // Info, Warning, Error, Success
    expires_at: std::time::Instant,
}
let mut toasts: Vec<Toast> = Vec::new();
```

**Triggering toasts:** The event loop already handles all `AgentEvent` variants. Toasts hook into `ToolStart`, `ToolEnd` (with `is_error`), `WorkflowUnitStart`, `WorkflowUnitEnd`, `WorkflowError`, `WorkflowBlocked`, and `WorkflowComplete`. No new AgentEvent variants are needed — the existing variants carry enough information.

**Dismissal:** At the top of each `terminal.draw()` call, drain expired toasts from `toasts.retain(|t| t.expires_at > Instant::now())`. Toast duration should be configurable in code (e.g. `const TOAST_DURATION_MS: u64 = 3000`).

**Rendering:** New `render_toasts(f, area, toasts)` function in widgets.rs. Takes the output area rect, positions toasts at the bottom of it (bottom 3 rows), renders them as overlapping `Paragraph` widgets with `Clear` behind each. Maximum 3 toasts visible at once (oldest dropped).

**Hook locations:**
- `widgets.rs`: new `render_toasts()` function, `Toast` struct, `ToastKind` enum
- `app.rs`: `toasts: Vec<Toast>` state variable, toast push in AgentEvent handlers, toast expiry drain in draw loop, `render_toasts()` call in draw closure (after output, before overlays)

**No new AgentEvent variants required.** This feature is purely in the TUI layer.

**Confidence:** HIGH.

---

### Feature 3: Inline Markdown Rendering (bold/italic)

**What it is:** `**bold**` and `*italic*` in `LineKind::Assistant` lines rendered with ratatui `Style::bold()` / `Style::italic()` spans instead of raw asterisks.

**Current state:** `render_output()` in widgets.rs has a large `match line.kind` block. The `LineKind::Assistant` arm already does line-level style dispatch (headers, bullets, code fences, questions). Inline bold/italic are not yet handled at the span level — the whole line is one `Span`.

**Integration approach:** Add a `parse_inline_spans(text: &str) -> Vec<Span<'static>>` helper function in widgets.rs. This function scans for `**...**` (bold) and `*...*` (italic, non-nested) patterns and produces a span list. The function replaces the current `Span::styled(text.clone(), ...)` calls in the plain-text fallback branch of the Assistant arm.

**Scope constraint:** Only the plain-text fallback branch needs changing. The header, code fence, bullet, and numbered list branches can remain single-span for now (markdown headers don't embed bold/italic in practice).

**No new AgentEvent variants.** No new state. No layout change.

**Binary size impact:** Zero — this is a pure string-scanning function with no new crate dependencies. The patterns needed are two: `**...**` and `*...*`. A simple state-machine scanner (no regex) keeps it minimal.

**Hook locations:**
- `widgets.rs` only: new `parse_inline_spans()` helper, modified plain-text fallback in `LineKind::Assistant` arm of `render_output()`

**Confidence:** HIGH.

---

### Feature 4: Help Overlay (`?` key)

**What it is:** Pressing `?` shows a modal overlay listing keybindings and slash commands. `?` or `Esc` dismisses it.

**Integration approach:** Same pattern as model picker overlay. Boolean state variable, keyboard guard, `Clear` + `Paragraph` in draw closure.

**New state in app.rs:**
```rust
let mut help_overlay_active = false;
```

**Keyboard hook:** Add to the `match (key.code, key.modifiers)` block:
```rust
(KeyCode::Char('?'), _) => {
    help_overlay_active = !help_overlay_active;
}
```
Add a guard at the top of the key match (after model picker guard) to catch `Esc` when active.

**Content:** The content is static — keybindings and slash commands. It can be a `const &[(&str, &str)]` array in app.rs or widgets.rs listing `(key, description)` pairs. No dynamic data required.

**Rendering:** New `render_help_overlay(f, area)` in widgets.rs. Renders a centered bordered box with the keybindings table. Uses the same `Clear` + centered rect pattern as the model picker.

**`SLASH_COMMANDS` already exists** in app.rs as a `const &[&str]` — the help overlay can derive its slash command list from this. A parallel `const SLASH_COMMAND_DESCRIPTIONS: &[(&str, &str)]` can pair each command with a description string.

**Hook locations:**
- `app.rs`: `help_overlay_active` state, keyboard handling, `render_help_overlay()` call in draw closure
- `widgets.rs`: new `render_help_overlay()` function

**Confidence:** HIGH.

---

### Feature 5: Token/Cost Display Improvements

**What it is:** Per-message token tracking or a cleaner session summary in the status bar.

**Current state:** `AgentEnd` carries `Usage { input_tokens, output_tokens, cost }`. The TUI accumulates these into `total_in`, `total_out`, `total_cost`. The status bar renders `in:{total_in} out:{total_out} | ${total_cost:.4}`. The per-turn breakdown is printed to `output_lines` as a system line after each turn: `└─ N in / M out ────`.

**Options:**

Option A (minimal): Improve the status bar formatting only. Replace raw counts with abbreviated display (e.g., `12k in / 4k out | $0.0142`). No structural changes. Touch only `render_status_bar()` in widgets.rs and the format strings in app.rs.

Option B (per-message history): Track per-message usage in a `Vec<(u64, u64, f64)>` in app.rs. The per-turn system line already provides this visually in output_lines. A summary panel showing the last N turns with token counts would require a new widget and more state. This is scope for the side info panel (Feature 7) rather than a standalone change.

**Recommendation:** Implement Option A first (formatting improvement only). Defer the per-message detail to the side panel feature where it fits naturally as one of its info rows.

**Hook locations (Option A):**
- `widgets.rs`: `render_status_bar()` — formatting only
- `app.rs`: optional — the `${cost:.4}` format string is in `render_status_bar`, not in app.rs

**Confidence:** HIGH.

---

### Feature 6: Visual Theme Consistency Pass

**What it is:** Audit and align the color palette and border styles across all widget render functions.

**Current palette (observed):**
- User text: `Color::Green` bold
- Assistant text: `Color::White`
- Thinking: `Color::DarkGray` italic
- Tool events: `Color::Yellow` dim
- Tool results: `Color::Green` dim
- Errors: `Color::Red` bold
- System/separators: `Color::DarkGray`
- Workflow panel active: `Color::Cyan`
- Status bar: `Color::White` on `Color::DarkGray`
- Borders: `Color::DarkGray`
- Input prompt: `Color::Cyan`
- Splash logo: `Color::DarkGray`
- Splash labels: `Color::Cyan` bold

**Inconsistencies to address:**
- ToolResult uses `Color::Green dim` — this is the same green as user text but dimmed, potentially confusing
- Thinking uses `Color::DarkGray italic` — low contrast; could use `Color::Gray italic` if terminal supports it
- The workflow panel border `Color::DarkGray` matches all other borders consistently

**No new state, no new AgentEvent variants, no layout changes.** Pure render function modifications in widgets.rs. Touch `render_output()`, `render_status_bar()`, `render_workflow_panel()`, and `render_splash()`.

**Hook locations:**
- `widgets.rs` only: all `render_*` functions — style constants only

**Recommendation:** Extract a `mod theme` block at the top of widgets.rs with `const` color definitions to make future consistency passes trivial.

**Confidence:** HIGH.

---

### Feature 7: Toggle-able Side Info Panel (Ctrl+P)

**What it is:** A right-side panel (off by default) showing model, tokens, and workflow state. Toggled with Ctrl+P.

**Integration approach:** This is the most structurally invasive feature. It requires a horizontal split of the output area.

**Layout change:** When side panel is visible, the layout becomes a horizontal split at the top level:

```
Without panel (current):         With panel (Ctrl+P active):
┌──────────────────┐             ┌──────────────┬────────────┐
│  Output (Min:3)  │             │ Output(Min)  │ SidePanel  │
├──────────────────┤             │              │ (Fixed:30) │
│ [WorkflowPanel]  │             ├──────────────┴────────────┤
├──────────────────┤             │ [WorkflowPanel]           │
│  StatusBar (1)   │             ├───────────────────────────┤
├──────────────────┤             │ StatusBar (1)             │
│  Input (2)       │             ├───────────────────────────┤
└──────────────────┘             │ Input (2)                 │
```

**Layout restructuring in app.rs:** The current layout is a single vertical `Layout::split()`. With the side panel, the top constraint (currently `Constraint::Min(3)` for output) becomes a sub-layout that splits horizontally. The workflow panel, spacer, status bar, and input remain full-width below.

Concretely:
1. Outer vertical layout: `[Constraint::Min(3), ...]` — same as now
2. When side panel active: `chunks[0]` is itself split horizontally into `[Constraint::Min(20), Constraint::Length(30)]`
3. The output widget renders in the left sub-chunk; the side panel renders in the right sub-chunk

**New state in app.rs:**
```rust
let mut side_panel_active = false;
```

**Keyboard hook:**
```rust
(KeyCode::Char('p'), KeyModifiers::CONTROL) => {
    side_panel_active = !side_panel_active;
}
```

**Side panel content:** Model name, provider, session token totals, cost, current workflow state summary (blueprint, section, task), context usage %. This is a read-only display of state already available in app.rs. No new events needed.

**New widget:** `render_side_panel(f, area, model, total_in, total_out, total_cost, workflow)` in widgets.rs.

**Interaction with other features:**
- Token/cost display (Feature 5): the side panel is the natural home for per-message token detail
- Auto-run panel expansion (Feature 1): `context_pct` from `WorkflowState` also shows here
- This feature depends on Feature 1 if `context_pct` is to be shown in the side panel

**Hook locations:**
- `app.rs`: `side_panel_active` state, `Ctrl+P` handler, layout restructuring in draw closure, `render_side_panel()` call
- `widgets.rs`: new `render_side_panel()` function

**Confidence:** HIGH on approach. MEDIUM on exact width (30 cols may need tuning for 80-col terminals — use `Constraint::Percentage(25)` or `Constraint::Min(28)` rather than fixed).

---

## New vs Modified Components Summary

| Component | Action | Scope |
|-----------|--------|-------|
| `src/io/agent_io.rs` | Modify | Add `WorkflowGitStatus` variant (Feature 1, optional); add `ContextUsage` variant (Feature 1, optional) |
| `src/workflow/git.rs` | Modify | Add `async fn last_commit()` helper (Feature 1) |
| `src/workflow/auto_loop.rs` | Modify | Emit `WorkflowGitStatus` in `finalize()` after commit (Feature 1) |
| `src/tui/widgets.rs` | Modify | `WorkflowState` new fields (F1); `render_workflow_panel()` expansion (F1); new `render_toasts()` (F2); `parse_inline_spans()` helper + render_output change (F3); new `render_help_overlay()` (F4); `render_status_bar()` formatting (F5); theme consistency pass (F6); new `render_side_panel()` (F7) |
| `src/tui/app.rs` | Modify | `WorkflowUnitStart` handler additions (F1); `toasts: Vec<Toast>` + toast push logic (F2); `help_overlay_active` + `?` key (F4); `side_panel_active` + Ctrl+P + layout split (F7) |

**New files:** None. All work fits in existing files.

---

## Build Order and Dependencies

### Dependency Graph

```
F6 (theme)        ← no deps, can go first
F3 (inline MD)    ← no deps, pure widgets.rs
F5 (token/cost)   ← no deps, formatting only
F4 (help overlay) ← no deps (uses existing SLASH_COMMANDS)
F2 (toasts)       ← no deps (uses existing AgentEvent)
F1 (auto-run panel) ← agent_io.rs changes; git.rs new fn; auto_loop.rs emit
F7 (side panel)   ← F1 (for context_pct display); F5 (token detail goes here)
```

### Recommended Phase Order

**Phase 1 — Foundation and zero-risk changes**
- F6: Theme consistency pass (widgets.rs only, no logic)
- F3: Inline markdown rendering (widgets.rs only, no state change)
- F5: Token/cost display formatting (widgets.rs only)

Rationale: These three are purely additive widget changes. No state, no events, no layout. Zero risk of regressions. Fast to build, fast to verify visually.

**Phase 2 — New overlays (keyboard-driven UI additions)**
- F4: Help overlay (new bool state + key handler + new widget)
- F2: Toast system (new state struct + event hooks + new widget)

Rationale: Both use the established overlay pattern (model picker precedent). They add state to app.rs and new render functions to widgets.rs but don't change layout. Each is self-contained and independently testable.

**Phase 3 — Workflow panel expansion**
- F1: Auto-run panel expansion (new AgentEvent variants, WorkflowState fields, git.rs helper, auto_loop.rs emit, larger panel layout)

Rationale: This is the most cross-cutting single feature. It touches agent_io.rs, git.rs, auto_loop.rs, widgets.rs (struct + render), and app.rs (event handler + layout constant). Doing it after F6/F3/F5 means the theme pass has already stabilized widget code.

**Phase 4 — Side panel (structurally invasive)**
- F7: Toggle-able side info panel (horizontal layout split in app.rs, new widget)

Rationale: This requires restructuring the draw layout, which is the highest-risk change. Doing it last means all other features are stable before the layout is touched. F1 is a prerequisite if context_pct is to appear in the side panel. F5's improved token display should be reflected here as the detailed token view.

---

## Critical Integration Notes

### The WorkflowState coupling

`WorkflowState` is defined in `widgets.rs` and mutated in `app.rs`. This is a pattern to preserve — do not move WorkflowState to app.rs. New fields go into the widgets.rs struct definition; the handlers in app.rs populate them.

### The draw closure owns rendering order

Overlays (model picker, toasts, help) must be rendered last in the `terminal.draw()` closure to appear on top. The current order is: output OR splash, [workflow panel], status bar, input, model picker. New overlays slot in before or after model picker — the exact order affects z-ordering (last rendered = on top).

Recommended final order:
1. Output / splash
2. Workflow panel (if active)
3. Status bar
4. Input
5. Side panel (if active) — replaces step 1's output area width
6. Toasts (float over output area)
7. Help overlay (full-screen modal)
8. Model picker (full-screen modal)

### Cursor positioning is layout-sensitive

The cursor position calculation at the bottom of the draw closure (`input_chunk` index) depends on which layout is active. After adding the side panel (Feature 7), the `input_chunk` index does not change because the side panel only affects the output area row, not the input row. However, the existing `wf_active` conditional `chunks[3]` vs `chunks[2]` must be preserved.

### The `?` key does not conflict

Scanning the existing key match block: `?` is a `Char('?')` with no modifiers. It is currently caught by the generic `Char(c)` arm and appended to input. The help overlay handler must be placed before the `Char(c)` arm, not after it. The same applies to any other new key bindings.

### Toast rendering in the draw closure

Toasts render as floating boxes over `chunks[0]` (the output area). They do not require a layout slot. The toast render call reads the current time and the `toasts` vec; the expiry drain happens before `terminal.draw()` each iteration so toasts are already pruned when the draw runs.

---

## Sources

All findings are from direct code analysis of the following files (HIGH confidence):
- `/Users/jeremymcspadden/Github/fin/src/tui/app.rs` (lines 1–1900+)
- `/Users/jeremymcspadden/Github/fin/src/tui/widgets.rs` (lines 1–527)
- `/Users/jeremymcspadden/Github/fin/src/io/agent_io.rs` (lines 1–108)
- `/Users/jeremymcspadden/Github/fin/src/workflow/auto_loop.rs` (lines 1–519)
- `/Users/jeremymcspadden/Github/fin/src/tui/tui_io.rs` (lines 1–91)
- `/Users/jeremymcspadden/Github/fin/.planning/PROJECT.md`
