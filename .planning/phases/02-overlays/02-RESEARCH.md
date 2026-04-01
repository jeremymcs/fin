// Fin — Phase 02: Overlays Research
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Phase 2: Overlays - Research

**Researched:** 2026-04-01
**Domain:** Ratatui TUI overlay rendering, Rust std::time::Instant, AgentEvent enum extension
**Confidence:** HIGH

## Summary

Phase 2 is a pure TUI overlay phase. No new dependencies, no new crates, no new layout panels. The
work divides cleanly into two independent features — a help overlay and a toast notification queue —
both built on patterns already established in Phase 1. The model picker overlay in `app.rs:302–347`
is the exact template for the help overlay. The existing `AgentEvent` drain loop at `app.rs:360–561`
is the exact insertion point for toast pushes. The only new data structure is a `VecDeque<(String,
std::time::Instant)>` for the toast queue, and the only new event variant is
`AgentEvent::StageTransition { from: String, to: String }` in `agent_io.rs`.

Research confirms that all locked decisions in CONTEXT.md are directly implementable against the
current codebase with no blockers. The ratatui 0.29 API (`Clear` widget, `Rect` math, `Paragraph`,
`Block::bordered()`, `f.render_widget`) matches the patterns already in production use. The
`std::time::Instant` approach for TTL is the correct Rust idiom and is immune to frame-rate and
event-flood effects as required by TOAST-04 and TOAST-05.

**Primary recommendation:** Implement in two sequential tasks — (1) help overlay first (self-contained
bool flag + render block), then (2) toast system (VecDeque state + StageTransition variant + render
block). Both tasks are independently verifiable before merge.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Single-column grouped layout for help overlay. Extend model picker pattern verbatim:
  `f.render_widget(Clear, area)` + `Paragraph::new(lines).block(Block::bordered().title(...).border_style(Palette::ACCENT))`. Zero new layout primitives.
- **D-02:** Help overlay is full-screen (centered, fills most of terminal area), not a small popup. Taller than model picker.
- **D-03:** `help_active: bool` local variable in `app.rs`, parallel to `model_picker_active`. When `true`, any key sets `help_active = false`.
- **D-04:** `?` key is only intercepted when `input_text.is_empty() && !model_picker_active`. When input is non-empty, `?` is inserted as a literal character.
- **D-05:** Content sourced from existing `/help` command text at `app.rs:2140` and `SLASH_COMMANDS` const at `app.rs:81–106`.
- **D-06:** Footer line: `[any key to close]` rendered dim (`Palette::DIM`).
- **D-07:** Toast state: `VecDeque<(String, std::time::Instant)>` capped at 2. Oldest dropped on overflow. Only front item renders. TTL expiry pops front at start of each drain cycle.
- **D-08:** Fixed TTL: 5 seconds (`Duration::from_secs(5)`). Tiered TTL deferred.
- **D-09:** Toast render position: top-right corner of `layout.output`. `Rect` computed each frame from `layout.output` — right-aligned, ~40 cols wide, 3 rows tall. Rendered after all other widgets using `f.render_widget(Clear, toast_area)` then styled `Paragraph` with colored border.
- **D-10:** Toast border color by event type: `Palette::ERROR` (Red) for errors, `Palette::ACCENT` (Yellow) for success/complete, `Palette::DIM` (DarkGray) for informational. ANSI named colors only.
- **D-11:** Add `AgentEvent::StageTransition { from: String, to: String }` to `src/io/agent_io.rs`. Emit from `auto_loop.rs` before `WorkflowUnitStart`. Skip emit when `from` is empty (first unit). Use local `prev_stage: Option<String>` tracker.
- **D-12:** All `AgentIO` implementations (`HeadlessIO`, `ChannelIO`, `PrintIO`, `HttpIO`, `TuiIO`) receive new variant via existing `emit()` — no-op for non-TUI backends.
- **D-13:** `StageTransition { from, to }` → toast: `"{from} → {to}"` (TOAST-01)
- **D-14:** `WorkflowComplete { blueprint_id, .. }` → toast: `"✓ {blueprint_id} complete"` (TOAST-02)
- **D-15:** `WorkflowBlocked { reason, .. }` → toast: `"⏸ Blocked: {reason}"` (TOAST-02)
- **D-16:** `ToolEnd { name, is_error: true, .. }` → toast: `"✗ {name} failed"` (TOAST-03). Existing output line push preserved.
- **D-17:** `WorkflowError { message }` → toast: `"⚠ {message}"` AND output line (additive). Error border: `Palette::ERROR`.
- **D-18:** `ModelChanged { display_name }` → toast: `"Model: {display_name}"` AND keep existing output line push.
- **D-19:** No toast for: `AgentStart`, `TurnStart/End`, `ToolStart`, `ToolEnd { is_error: false }`, `WorkflowUnitStart/End`, `WorkflowProgress`.

### Claude's Discretion

- Toast content truncation: messages exceeding ~36 chars truncated with `…` to fit 40-col render rect.
- Exact toast `Rect` math (x offset, y offset from `layout.output`) — implement to be visually comfortable.
- Whether to add `render_toast()` to `widgets.rs` or keep inline in `app.rs` — either acceptable given small render footprint.

### Deferred Ideas (OUT OF SCOPE)

- Tiered TTL (errors 8s, info 4s) — Phase 3+
- TOAST-FUT-01: multiple simultaneous visible toasts (stacking) — future phase
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HELP-01 | User can press `?` to open a full-screen keybindings and slash command reference | `help_active: bool` + render block mirrors model picker pattern at app.rs:302–347 |
| HELP-02 | Help overlay dismissed by pressing any key | Key intercept block sets `help_active = false` + `continue` for any key code when `help_active` is true |
| HELP-03 | `?` key only intercepted when input field is empty | Guard: `input_text.is_empty() && !model_picker_active` before setting `help_active = true` |
| TOAST-01 | Ephemeral toast when workflow stage transitions | New `AgentEvent::StageTransition` variant; emitted from `auto_loop.rs` before `WorkflowUnitStart` |
| TOAST-02 | Ephemeral toast when auto-loop completes or is blocked | Push toast inside existing `WorkflowComplete` and `WorkflowBlocked` match arms |
| TOAST-03 | Ephemeral toast when a tool call produces an error | Push toast inside existing `ToolEnd { is_error: true }` match arm |
| TOAST-04 | Toasts auto-dismiss after fixed duration (Instant-based, not frame-count-based) | `std::time::Instant` TTL checked at start of each drain cycle; immune to event flood |
| TOAST-05 | No toast for routine individual tool calls | No-toast list: `AgentStart`, `TurnStart/End`, `ToolStart`, `ToolEnd { is_error: false }`, `WorkflowUnitStart/End`, `WorkflowProgress` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.29 (in Cargo.toml) | TUI rendering — `Clear`, `Paragraph`, `Block`, `Rect`, `Layout` | Already in production use; all overlay APIs verified present |
| crossterm | 0.28 (in Cargo.toml) | Terminal events — `KeyCode`, `KeyModifiers`, `event::poll` | Already in production use; key handling patterns established |
| tokio | 1 (full features) | Async runtime; `mpsc::UnboundedSender<AgentEvent>` channel | Already in production use |
| std::collections::VecDeque | stdlib | Toast queue (capped FIFO, O(1) push/pop) | No new dependency; exactly right for bounded queue with oldest-drop |
| std::time::Instant / Duration | stdlib | TTL tracking — Instant::elapsed() >= Duration::from_secs(5) | Monotonic clock; immune to system clock skew and frame rate |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ratatui::widgets::Clear | 0.29 | Erase background before overlay render | Required before every overlay Paragraph to prevent bleed-through |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::time::Instant TTL | Frame counter | Frame counter is flood-sensitive — an event burst that fills the drain loop in one tick could effectively freeze the timer; Instant is immune (TOAST-04, TOAST-05) |
| VecDeque capped at 2 | Single Option<(String, Instant)> | Single slot loses simultaneous WorkflowComplete + ToolEnd events; VecDeque preserves both signals |

**Installation:** No new dependencies required.

## Architecture Patterns

### Recommended Project Structure

No new files required. All changes land in existing files:

```
src/
├── io/
│   └── agent_io.rs          # Add AgentEvent::StageTransition variant
├── workflow/
│   └── auto_loop.rs         # Emit StageTransition before WorkflowUnitStart
└── tui/
    ├── app.rs               # help_active bool, toast VecDeque, render blocks, key intercepts
    └── widgets.rs           # Optional: render_toast() helper (discretionary)
```

Non-TUI backends that need exhaustive match coverage for the new variant:
```
src/io/headless.rs           # Add no-op or serialization arm for StageTransition
src/io/channel_io.rs         # Forwards event as-is via channel — no arm needed (passes through)
src/io/print_io.rs           # Add no-op arm
src/io/http.rs               # Add no-op arm (if AgentIO is implemented)
```

### Pattern 1: Overlay Bool Flag + Key Intercept (Help Overlay)

**What:** A local bool that when true: (a) intercepts all keyboard input before the normal key handler, setting itself to false and calling `continue`; (b) triggers a render block that draws `Clear` + `Paragraph` over the entire frame.

**When to use:** Any full-screen modal that dismisses on any key. Established by `model_picker_active`.

**Example (verbatim model picker pattern, adapted for help):**
```rust
// Source: src/tui/app.rs:302-347 (model picker) — replicate for help overlay

// --- Render block (inside terminal.draw closure, after input widget) ---
if help_active {
    let area = f.area();
    let overlay_width  = (area.width.saturating_sub(4)).min(80);
    let overlay_height = area.height.saturating_sub(4);
    let overlay_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(overlay_width)) / 2,
        y: (area.height.saturating_sub(overlay_height)) / 2,
        width:  overlay_width,
        height: overlay_height,
    };
    f.render_widget(ratatui::widgets::Clear, overlay_area);

    // Build content lines from SLASH_COMMANDS const + keybindings literals
    // Footer: dim "[any key to close]"
    let help = ratatui::widgets::Paragraph::new(lines)
        .block(
            ratatui::widgets::Block::bordered()
                .title(" Keybindings & Commands ")
                .border_style(ratatui::style::Style::default().fg(Palette::ACCENT)),
        );
    f.render_widget(help, overlay_area);
}

// --- Key intercept block (inside Event::Key handler, before normal match) ---
if help_active {
    help_active = false;
    continue;
}

// --- ? key activation (inside normal key handler, after model_picker check) ---
(KeyCode::Char('?'), _) if input_text.is_empty() && !model_picker_active => {
    help_active = true;
}
```

### Pattern 2: Toast VecDeque with Instant TTL

**What:** A `VecDeque<(String, std::time::Instant)>` local variable. At the start of each tick's
drain cycle, expired entries are popped. During event processing, new toasts are pushed (oldest
dropped if queue is full). After all widgets are rendered, if the queue is non-empty, the front
item is rendered in the top-right corner of `layout.output`.

**When to use:** Ephemeral notification system where events must survive simultaneous arrival.

**Example:**
```rust
// Source: Pattern derived from D-07/D-08/D-09 decisions + std::time::Instant idioms

// --- State variable (declared alongside other local vars) ---
let mut toasts: std::collections::VecDeque<(String, std::time::Instant)> = Default::default();
const TOAST_TTL: std::time::Duration = std::time::Duration::from_secs(5);
const TOAST_MAX: usize = 2;
const TOAST_WIDTH: u16 = 40;
const TOAST_HEIGHT: u16 = 3;

// --- TTL expiry (start of drain loop, before try_recv) ---
if toasts.front().map(|(_, t)| t.elapsed() >= TOAST_TTL).unwrap_or(false) {
    toasts.pop_front();
}

// --- Push helper (inline or fn) ---
fn push_toast(toasts: &mut VecDeque<(String, Instant)>, msg: String, max: usize) {
    if toasts.len() >= max {
        toasts.pop_front(); // drop oldest to make room
    }
    let truncated = if msg.chars().count() > 36 {
        format!("{}…", msg.chars().take(36).collect::<String>())
    } else {
        msg
    };
    toasts.push_back((truncated, Instant::now()));
}

// --- Render block (after all other widgets, before cursor placement) ---
if let Some((msg, _)) = toasts.front() {
    let out = layout.output;
    let toast_area = Rect {
        x: out.x + out.width.saturating_sub(TOAST_WIDTH),
        y: out.y,
        width: TOAST_WIDTH.min(out.width),
        height: TOAST_HEIGHT,
    };
    f.render_widget(ratatui::widgets::Clear, toast_area);
    let border_color = // determined by event type stored alongside message
        Palette::ACCENT; // default; error events use Palette::ERROR
    let toast_widget = Paragraph::new(msg.as_str())
        .block(
            Block::bordered()
                .border_style(Style::default().fg(border_color)),
        );
    f.render_widget(toast_widget, toast_area);
}
```

### Pattern 3: Adding AgentEvent Variant + Emit Point

**What:** Extend the enum with a new variant, add exhaustive match arms in all AgentIO
implementations, and emit the new event from `auto_loop.rs` before `WorkflowUnitStart`.

**When to use:** Adding a new signal to the agent event bus.

**Example:**
```rust
// Source: src/io/agent_io.rs — add to AgentEvent enum
StageTransition {
    from: String,
    to: String,
},

// Source: src/workflow/auto_loop.rs — before existing WorkflowUnitStart emit
// prev_stage declared as: let mut prev_stage: Option<String> = None;
// Inside DispatchResult::Unit(unit) arm:
if let Some(ref from) = prev_stage {
    if *from != unit.stage.label() {
        let _ = io.emit(AgentEvent::StageTransition {
            from: from.clone(),
            to: unit.stage.label().to_string(),
        }).await;
    }
}
prev_stage = Some(unit.stage.label().to_string());

// Then emit WorkflowUnitStart as before...
```

### Toast Border Color — Event Type Tracking

Since `VecDeque` stores `(String, Instant)`, the border color (which varies by event type) must
also be stored. The tuple should be extended to `(String, Instant, ToastKind)` or an equivalent.
This is a small discretionary call left to the implementer. Simplest approach:

```rust
#[derive(Clone)]
enum ToastKind { Info, Success, Error }

let mut toasts: VecDeque<(String, Instant, ToastKind)> = Default::default();
```

This keeps border color logic local to `app.rs` without touching `widgets.rs`.

### Anti-Patterns to Avoid

- **Don't use a frame counter for TTL:** Frame rate varies (50ms tick, but drain loop is variable under load). `Instant::elapsed()` is monotonic and immune. Required by TOAST-04.
- **Don't intercept `?` when input is non-empty:** HELP-03 is a hard requirement. Guard must check `input_text.is_empty() && !model_picker_active` before setting `help_active = true`.
- **Don't render toast before `Clear`:** Ratatui draws in order; without `Clear` the previous frame's content bleeds through the overlay. Required for both help overlay and toast.
- **Don't add `help_active` as a struct field on `AppLayout`:** It's event-driven app state, not layout state. Keep it as a local variable in `run_app()`, parallel to `model_picker_active`.
- **Don't skip exhaustive match arm for `StageTransition` in non-TUI backends:** Rust will fail to compile with a non-exhaustive match. Add no-op arms to `headless.rs`, `print_io.rs`, and any other `AgentIO` implementor. `channel_io.rs` forwards the event as-is and requires no arm change.
- **Don't push toast for `ToolEnd { is_error: false }`:** Per TOAST-05, routine tool calls must not surface. Only `is_error: true` triggers a toast.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Modal overlay background erasure | Custom cell-clearing loop | `ratatui::widgets::Clear` | Built-in, handles partial renders, correct z-order |
| Monotonic elapsed timer | Frame counter or chrono dependency | `std::time::Instant::elapsed()` | stdlib, zero-cost, monotonic, flood-immune |
| Bounded FIFO queue with oldest-drop | Custom ring buffer | `std::collections::VecDeque` with manual cap check | stdlib, O(1) all ops, no allocation overhead |
| String truncation to terminal width | Custom grapheme-aware truncator | Simple `.chars().count()` + take(36) + `…` | ASCII-dominant content; full grapheme cluster splitting is overkill for these short status strings |

**Key insight:** This phase adds no new crates. Every primitive needed (Clear, Instant, VecDeque,
Paragraph, Block) is already in scope or in stdlib.

## Common Pitfalls

### Pitfall 1: Non-Exhaustive Match on AgentEvent After Adding StageTransition

**What goes wrong:** `cargo build` fails with "non-exhaustive patterns" in every file that matches
on `AgentEvent` — `headless.rs`, `print_io.rs`, `http.rs`, and potentially `commands.rs`.

**Why it happens:** Rust requires exhaustive pattern matching on enums. Adding a variant without
updating all match sites is a compile error, not a warning.

**How to avoid:** After adding the variant to `agent_io.rs`, run `cargo check` immediately. Grep
all files for `AgentEvent::` to find every match site and add the new arm before compiling the
full build.

**Warning signs:** `cargo check` output containing "non-exhaustive patterns: `StageTransition` not covered".

### Pitfall 2: Toast Rect Overflows layout.output Bounds

**What goes wrong:** If `layout.output.width < TOAST_WIDTH`, the `Rect { x: ..., width: TOAST_WIDTH }` extends beyond the terminal right edge, causing ratatui to panic or silently clip incorrectly.

**Why it happens:** Ratatui panics in debug builds if a Rect extends beyond the terminal area.

**How to avoid:** Always use `.min(out.width)` for toast width: `width: TOAST_WIDTH.min(out.width)`.
For x position: `x: out.x + out.width.saturating_sub(TOAST_WIDTH)`.

**Warning signs:** Panic with "attempt to add with overflow" or visual corruption at narrow terminal widths.

### Pitfall 3: Help Overlay `?` Triggered While Model Picker Is Active

**What goes wrong:** If `help_active` is set while `model_picker_active` is true, both overlays
render simultaneously. The model picker's key intercept runs first (it's checked first), so the
help overlay renders beneath the picker but the help state leaks into the next frame.

**Why it happens:** Key handler order matters. The guard `!model_picker_active` must be part of
the `?` activation condition, not just the `help_active` render block.

**How to avoid:** Activation guard (D-04): `input_text.is_empty() && !model_picker_active`. This
is already a locked decision — do not relax it.

**Warning signs:** Pressing `?` while model picker is open causes both overlays to appear simultaneously.

### Pitfall 4: Toast Push During `WorkflowComplete` Conflicts with Existing Output Line Push

**What goes wrong:** The existing `WorkflowComplete` arm pushes an output line. Adding a toast
push in the same arm is additive and correct — but if the output line push is accidentally removed,
the audit trail in the output area is lost.

**Why it happens:** D-14/D-17/D-18 are explicitly additive. Both the output line AND the toast
must be preserved.

**How to avoid:** Read D-14 through D-18 carefully: "AND output line" means both operations coexist.
Do not replace the existing `output_lines.push(...)` call — append the `push_toast(...)` call after it.

**Warning signs:** Missing output lines in conversation area after workflow events.

### Pitfall 5: `prev_stage` Tracker Emits Spurious StageTransition on First Unit

**What goes wrong:** On the first iteration of `run_loop`, `prev_stage` is `None`. If the emit
is not guarded, a `StageTransition { from: "", to: "build" }` is emitted with an empty `from`,
causing a malformed toast: `" → build"`.

**Why it happens:** The guard `if let Some(ref from) = prev_stage` is required (D-11 locked).

**How to avoid:** Use `Option<String>` for `prev_stage`. Only emit when `Some(from)` is matched
and `from != unit.stage.label()` (to skip no-op same-stage transitions).

**Warning signs:** Toast showing `" → build"` (leading arrow with no left-hand stage name).

## Code Examples

Verified patterns from source code inspection:

### Rect Math for Top-Right Corner Overlay
```rust
// Source: derived from app.rs:307–312 (model picker Rect pattern)
let out = layout.output;
let toast_area = ratatui::layout::Rect {
    x: out.x + out.width.saturating_sub(TOAST_WIDTH),
    y: out.y,
    width: TOAST_WIDTH.min(out.width),
    height: TOAST_HEIGHT,
};
```

### Clear + Paragraph Overlay Render (model picker verbatim)
```rust
// Source: src/tui/app.rs:315–346
f.render_widget(ratatui::widgets::Clear, picker_area);
let picker = ratatui::widgets::Paragraph::new(items).block(
    ratatui::widgets::Block::bordered()
        .title(" Select Model (↑↓ Enter Esc) ")
        .border_style(ratatui::style::Style::default().fg(Palette::ACCENT)),
);
f.render_widget(picker, picker_area);
```

### Instant TTL Check (stdlib idiomatic)
```rust
// Source: stdlib std::time::Instant docs — standard elapsed() pattern
use std::time::{Duration, Instant};
const TTL: Duration = Duration::from_secs(5);

// At start of drain cycle:
if toasts.front().map(|(_, t, _)| t.elapsed() >= TTL).unwrap_or(false) {
    toasts.pop_front();
}
```

### Exhaustive Match Arm for Non-TUI Backends (no-op pattern)
```rust
// Source: src/io/headless.rs:98 (workflow events handled via _ catch-all or explicit arms)
// Add alongside existing workflow event arms:
AgentEvent::StageTransition { .. } => {
    // No-op for headless/print/http backends — TUI-only signal
    return Ok(());
}
```

### Help Content Lines Construction
```rust
// Source: app.rs:81–106 (SLASH_COMMANDS), app.rs:2140–2168 (/help text)
// Build Vec<Line> from two sections:
let mut lines: Vec<ratatui::text::Line> = vec![
    Line::styled(" Keybindings", Style::default().fg(Palette::ACCENT).bold()),
    Line::raw(""),
    Line::raw("  Ctrl+C      Quit"),
    Line::raw("  ?           This help overlay"),
    Line::raw("  /model      Switch model"),
    // ... etc
    Line::raw(""),
    Line::styled(" Slash Commands", Style::default().fg(Palette::ACCENT).bold()),
    Line::raw(""),
];
for cmd in SLASH_COMMANDS {
    lines.push(Line::raw(format!("  /{cmd}")));
}
lines.push(Line::raw(""));
// Footer — dim, right-aligned suggestion
lines.push(Line::styled("  [any key to close]", Style::default().fg(Palette::DIM)));
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Frame-count TTL for overlays | `std::time::Instant::elapsed()` | Phase 2 design (D-08) | Immune to event flood; no drift under load |
| Single-slot Option<String> for notifications | `VecDeque<(String, Instant, ToastKind)>` capped at 2 | Phase 2 design (D-07) | Simultaneous events (WorkflowComplete + ToolError) both survive |

**Deprecated/outdated:**
- Frame-counter-based UI timers: susceptible to flood — any event burst that fills the drain loop
  faster than 50ms/frame causes the counter to stall. `Instant` is the correct replacement.

## Open Questions

1. **Toast border color storage in VecDeque tuple**
   - What we know: D-10 specifies three colors by event type (ERROR, ACCENT, DIM). The tuple needs to carry this.
   - What's unclear: Whether to use an enum `ToastKind` (Info/Success/Error) or store `Color` directly. Both compile fine. The CONTEXT.md left this to discretion.
   - Recommendation: Introduce a minimal `ToastKind` enum local to `app.rs` (not exported). Cleaner than storing `Color` (which isn't `Copy`-free and ties render logic to event kind at push time).

2. **HeadlessIO `StageTransition` serialization**
   - What we know: `HeadlessIO` serializes workflow events as JSONL. The new variant needs an arm.
   - What's unclear: Whether external tooling reads headless output and expects a `stage_transition` event type, or whether a no-op/`return Ok(())` is sufficient.
   - Recommendation: Add a `workflow_stage_transition` event type serialization (consistent with existing workflow event serialization style). Low-cost and forward-compatible.

3. **`render_toast` helper location**
   - What we know: CONTEXT.md explicitly leaves this to discretion. The toast render is ~15 lines.
   - What's unclear: Whether the planner should prescribe a location.
   - Recommendation: Keep inline in `app.rs` for Phase 2. If Phase 3 adds more overlay types, extract to `widgets.rs` then. Premature extraction adds indirection without benefit at this size.

## Environment Availability

Step 2.6: SKIPPED — Phase 2 is purely code changes to existing Rust source files. No external
tools, services, CLIs, databases, or runtimes beyond the existing `cargo` build toolchain are
required.

## Validation Architecture

> config.json does not set `workflow.nyquist_validation` — treated as enabled.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | `Cargo.toml` (workspace root) |
| Quick run command | `cargo test --lib 2>&1 \| tail -20` |
| Full suite command | `cargo test 2>&1 \| tail -40` |
| Build check command | `cargo build --features tui 2>&1 \| head -30` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | Notes |
|--------|----------|-----------|-------------------|-------|
| HELP-01 | `?` opens overlay when input empty | Manual (TUI visual) | `cargo build --features tui` | No unit test; visual only |
| HELP-02 | Any key dismisses overlay | Manual (TUI visual) | `cargo build --features tui` | State-machine trivial |
| HELP-03 | `?` in non-empty input → literal char, no overlay | Unit | `cargo test tui::app::tests::help_key_guard` | Test: set `input_text = "hello"`, send `?`, assert `help_active == false` and `input_text == "hello?"` |
| TOAST-01 | StageTransition event pushes toast | Unit | `cargo test tui::app::tests::toast_stage_transition` | Test: push `StageTransition` event, assert toast queue non-empty |
| TOAST-02 | WorkflowComplete/Blocked push toast | Unit | `cargo test tui::app::tests::toast_workflow_terminal` | Test: push each event, assert queue non-empty |
| TOAST-03 | ToolEnd is_error:true pushes toast | Unit | `cargo test tui::app::tests::toast_tool_error` | Test: push `ToolEnd { is_error: true }`, assert queue non-empty |
| TOAST-04 | Toast auto-dismisses after 5s (Instant-based) | Unit | `cargo test tui::app::tests::toast_ttl_expiry` | Test: push toast with `Instant::now() - Duration::from_secs(6)`, run expiry check, assert queue empty |
| TOAST-05 | No toast for ToolStart / ToolEnd is_error:false / TextDelta | Unit | `cargo test tui::app::tests::toast_no_fire_routine` | Test: push routine events, assert toast queue stays empty |

**Note:** TUI rendering tests (HELP-01, HELP-02) are manual verification only — ratatui has no
headless render assertion API in the project's current test setup. Compile-pass plus manual
spot-check is the acceptance bar for those two. The remaining six requirements map to unit-testable
state transitions.

### Sampling Rate
- **Per task commit:** `cargo build --features tui` (compile gate — catches exhaustive match failures)
- **Per wave merge:** `cargo test 2>&1 | tail -40`
- **Phase gate:** Full suite green + manual TUI smoke test before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/tui/app.rs` — no existing unit test module for `help_active` / toast state; Wave 0 should add `#[cfg(test)] mod tests` with helper to simulate event drain
- [ ] `ToastKind` enum definition (local to `app.rs`) — needed before toast push logic can be tested

*(If no unit test module exists for app.rs today, Wave 0 adds the module scaffold. Existing `cargo test` passes — no regression risk.)*

## Sources

### Primary (HIGH confidence)
- Direct source read: `src/tui/app.rs` (lines 1–354, 355–590, 575–780, 2100–2180) — model picker pattern, key handler, drain loop, help text
- Direct source read: `src/tui/widgets.rs` (lines 1–300) — Palette constants, render functions
- Direct source read: `src/io/agent_io.rs` (complete) — AgentEvent enum, AgentIO trait
- Direct source read: `src/workflow/auto_loop.rs` (lines 1–220) — dispatch loop, emit sites
- Direct source read: `src/io/headless.rs` (lines 1–120) — non-TUI AgentIO implementation pattern
- Direct source read: `src/io/channel_io.rs` (complete) — pass-through emit pattern
- Direct source read: `src/io/tui_io.rs` (lines 1–50) — TUI channel bridge
- Direct source read: `Cargo.toml` — ratatui 0.29, crossterm 0.28, tokio 1 confirmed
- Direct source read: `.planning/config.json` — `nyquist_validation` key absent (treated as enabled)

### Secondary (MEDIUM confidence)
- ratatui 0.29 widget API (`Clear`, `Block::bordered()`, `Paragraph`, `Rect`) — verified by existing usage patterns in `app.rs` and `widgets.rs` (all patterns already in production)

### Tertiary (LOW confidence)
- None — all findings verified against source code directly.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions confirmed from Cargo.toml; all APIs verified by existing production use in app.rs/widgets.rs
- Architecture patterns: HIGH — model picker overlay pattern exists verbatim at app.rs:302–347; drain loop at app.rs:360–561; all integration points identified by line number
- Pitfalls: HIGH — each pitfall derived from direct code inspection of the match sites and Rect math in production code, not from general knowledge
- Validation architecture: MEDIUM — test module gaps identified but exact test harness structure depends on what `#[cfg(test)]` infrastructure the implementer adds in Wave 0

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable Rust crates; internal code only changes via this phase's own PRs)
