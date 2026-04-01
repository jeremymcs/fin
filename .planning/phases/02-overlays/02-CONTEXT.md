# Phase 2: Overlays - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 delivers two keyboard-driven overlay systems on top of the Phase 1 foundation: (1) a full-screen help overlay triggered by `?` that shows keybindings and slash command reference, and (2) an ephemeral toast notification system that surfaces high-signal workflow events at the top-right of the output area. No new layout panels, no workflow engine behavior changes beyond adding `AgentEvent::StageTransition` — overlay rendering and event-routing only.

</domain>

<decisions>
## Implementation Decisions

### Help Overlay

- **D-01:** Single-column grouped layout — keybindings section first, slash commands section second. Directly extends the model picker overlay pattern: `f.render_widget(Clear, area)` + `Paragraph::new(lines).block(Block::bordered().title(...).border_style(Palette::ACCENT))`. Zero new layout primitives required.
- **D-02:** Overlay is full-screen (centered, fills most of terminal area), not a small popup. Analogous to model picker but taller.
- **D-03:** `help_active: bool` state variable in `app.rs`, parallel to `model_picker_active`. When `true`, all keys set `help_active = false` (any key dismisses — HELP-02).
- **D-04:** `?` key is only intercepted when `input_text.is_empty() && !model_picker_active` (HELP-03). When input is non-empty, `?` is inserted as a literal character.
- **D-05:** Content sourced from the existing `/help` command text at `app.rs:2140` and the `SLASH_COMMANDS` const — same canonical list, reused for the overlay.
- **D-06:** Footer line at bottom of overlay: `[any key to close]` rendered dim (DarkGray), using `Palette::DIM` or equivalent.

### Toast Notification System

- **D-07:** Toast state: `VecDeque<(String, std::time::Instant)>` capped at 2 entries. When full, oldest is dropped before pushing new toast. Only the front item renders at any frame. This prevents a `WorkflowComplete` toast being wiped by a simultaneous `ToolEnd { is_error }` in the same `try_recv()` drain cycle — both signals survive.
- **D-08:** Fixed TTL: **5 seconds** (`Duration::from_secs(5)`). After TTL expires, front item is popped at the start of each tick's drain loop. (Tiered TTL deferred — add in Phase 3+ if needed.)
- **D-09:** Toast render position: top-right corner of `layout.output` area. Absolute `Rect` computed each frame from `layout.output` (right-aligned, ~40 cols wide, 3 rows tall). Rendered after all other widgets using `f.render_widget(Clear, toast_area)` then a styled `Paragraph` with colored border.
- **D-10:** Toast border color by event type: `Palette::ERROR` (Red) for errors, `Palette::ACCENT` (Yellow) for success/complete events, `Palette::DIM` (DarkGray) for informational. Follows ANSI named colors only (D-04 from Phase 1).

### Stage Transition Detection

- **D-11:** Add `AgentEvent::StageTransition { from: String, to: String }` variant to `src/io/agent_io.rs`. Emit from `src/workflow/auto_loop.rs` before `WorkflowUnitStart` is sent, using a local `prev_stage: Option<String>` tracker. Skip emit when `from` would be empty (first unit of a run). This is consistent with the existing design (8 specific workflow event variants) and gives Phase 3's auto-run panel a clean signal without detection duplication.
- **D-12:** All `AgentIO` trait implementations (`HeadlessIO`, `HttpIO`, `McpIO`, `PrintIO`) receive the new variant via the existing `emit()` arm — no-op for non-TUI backends is acceptable.

### Toast Signal Set (complete list)

- **D-13:** `AgentEvent::StageTransition { from, to }` → toast: `"{from} → {to}"` (TOAST-01)
- **D-14:** `AgentEvent::WorkflowComplete { blueprint_id, units_run }` → toast: `"✓ {blueprint_id} complete"` (TOAST-02)
- **D-15:** `AgentEvent::WorkflowBlocked { reason, .. }` → toast: `"⏸ Blocked: {reason}"` (TOAST-02)
- **D-16:** `AgentEvent::ToolEnd { name, is_error: true, .. }` → toast: `"✗ {name} failed"` (TOAST-03). Output line still pushed (existing behavior preserved).
- **D-17:** `AgentEvent::WorkflowError { message }` → toast: `"⚠ {message}"` **AND** output line (additive — existing output line push preserved). Error toasts render with `Palette::ERROR` border.
- **D-18:** `AgentEvent::ModelChanged { display_name }` → toast: `"Model: {display_name}"` **AND** keep existing output line push. Infrequent; output line stays as audit trail.
- **D-19:** No toast for: `AgentStart`, `TurnStart/End`, `ToolStart`, `ToolEnd { is_error: false }`, `WorkflowUnitStart/End`, `WorkflowProgress` (too frequent / already surfaced by workflow panel — violates TOAST-05).

### Claude's Discretion

- Toast content truncation: if a message exceeds ~36 chars, truncate with `…` to keep the toast within a 40-col render rect.
- Exact toast `Rect` math (x offset, y offset from layout.output) — implement to be visually comfortable, no strict constraint.
- Whether to add a `render_toast()` function to `widgets.rs` or keep inline in `app.rs` — either is acceptable given small render footprint.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core planning files
- `.planning/REQUIREMENTS.md` — Phase 2 requirements: HELP-01, HELP-02, HELP-03, TOAST-01, TOAST-02, TOAST-03, TOAST-04, TOAST-05
- `.planning/ROADMAP.md` — Phase 2 goal and success criteria (5 criteria)
- `.planning/phases/01-foundation/01-CONTEXT.md` — Phase 1 decisions: Palette constants (D-01–D-04), established model picker overlay pattern (D-09), ANSI-only colors (D-04)

### TUI source files to read before planning
- `src/tui/app.rs` — Main loop, model picker overlay pattern (lines 302–347), key handler structure (lines 563–950), AgentEvent drain loop (lines 360–560), `AppLayout` struct (lines 34–78), `SLASH_COMMANDS` const (lines 81–106), existing `/help` command content (line ~2140)
- `src/tui/widgets.rs` — `Palette` const struct, `render_workflow_panel`, `OutputLine`/`LineKind`, `render_output`, all existing render functions
- `src/io/agent_io.rs` — `AgentEvent` enum (add `StageTransition` here)
- `src/workflow/auto_loop.rs` — Workflow stage dispatch loop (emit `StageTransition` before `WorkflowUnitStart`)

No external specs — requirements fully captured in decisions above.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Model picker overlay pattern** (`app.rs:302–347`): `f.render_widget(Clear, picker_area)` + centered `Rect` computation + `Paragraph::new(items).block(Block::bordered().title(...).border_style(Palette::ACCENT))` — use verbatim for help overlay
- **`Palette` const struct** (`widgets.rs:11`): `ACCENT`, `DIM`, and color constants — use for toast border colors (no new color definitions needed)
- **`SLASH_COMMANDS` const** (`app.rs:81–106`): canonical command list for overlay content
- **Existing `/help` command text** (`app.rs:~2140`): same content, reuse for overlay line list
- **`workflow_state.current_stage`** (`app.rs:495, 515`): already tracked — `auto_loop.rs` emits `StageTransition` before overwriting this

### Established Patterns
- Overlay state: `bool` flag (`model_picker_active`) that intercepts all key events when `true` — same pattern for `help_active`
- Event drain loop: `while let Ok(evt) = agent_event_rx.try_recv()` — toast push happens inside existing match arms
- `AppLayout` named fields (`layout.output`, `layout.input`, etc.) — toast `Rect` computed from `layout.output` each frame

### Integration Points
- **Help overlay**: add `help_active: bool` local var; add render block after input widget render (after line 300); add key intercept before or after model picker intercept block (line 570)
- **Toast render**: add after all widget renders, before cursor placement (before line 349) — toast sits on top of output area
- **Toast push**: inside existing `AgentEvent` match arms for `StageTransition`, `WorkflowComplete`, `WorkflowBlocked`, `ToolEnd { is_error }`, `WorkflowError`, `ModelChanged`
- **`StageTransition` emit**: in `auto_loop.rs` dispatch loop, before the existing `WorkflowUnitStart` emit

</code_context>

<specifics>
## Specific Ideas

- The visual preview selected for help overlay: single-column with `Keybindings` section header and `Slash Commands` section header, dim `[any key to close]` footer line at the bottom right.
- Toast queue behavior confirmed: VecDeque capped at 2, oldest dropped on overflow, only front renders — TOAST-FUT-01 (true stacking with multiple visible toasts) stays deferred.
- `AgentEvent::StageTransition` is additive and forward-compatible — Phase 3 auto-run panel gets the clean signal without TUI-local detection duplication.

</specifics>

<deferred>
## Deferred Ideas

- Tiered TTL (errors 8s, info 4s) — can layer on top of existing toast queue in Phase 3+ if signal differentiation becomes valuable
- TOAST-FUT-01: multiple simultaneous visible toasts (stacking) — deferred to future phase per requirements

None — discussion stayed within Phase 2 scope.

</deferred>

---

*Phase: 02-overlays*
*Context gathered: 2026-04-01*
