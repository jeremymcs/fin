# Phase 3: Auto-Run Panel - Research

**Researched:** 2026-04-01
**Domain:** Rust / ratatui TUI — workflow panel expansion, AgentEvent plumbing, async git fetch
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Dynamic height — `AppLayout` computes a taller workflow `Rect` when `workflow_state.is_auto` is true. Third layout variant alongside existing "with workflow panel" and "without workflow panel" variants. When auto mode is active, the output area shrinks by approximately 7 rows (5 new inner rows + 2 border rows) to accommodate the new rows.
- **D-02:** Auto-mode panel inner layout (7-8 rows): pipeline row, progress bar row, model+blueprint row, stage+section row, last commit row, context % bar row, footer hints row.
- **D-03:** Existing pipeline (✓/●/○) and progress bar rows are **unchanged** — auto-run rows are additive below them. AUTO-06 compliance.
- **D-04:** Source: latest repo commit regardless of author — `git log -1 --format='%h %s'`. Shows last committed (including Fin auto-commits and manual commits).
- **D-05:** Fetch timing: async on `WorkflowUnitEnd` — after each unit completes, run `git log -1` and store in `WorkflowState`. Non-blocking render; last known value shown until next update.
- **D-06:** Fields added to `WorkflowState`: `last_commit_hash: String`, `last_commit_msg: String`. Empty string until first fetch completes.
- **D-07:** Delivery: new `AgentEvent::ContextUsage { pct: u8 }` variant — add to `src/io/agent_io.rs` alongside `StageTransition`. Emitted once per turn, after `TurnEnd`, using cumulative `Usage` stats already tracked.
- **D-08:** Emit point: after `TurnEnd` in the agent turn loop. The emitter computes `pct = (input_tokens / context_window_size * 100) as u8`.
- **D-09:** Denominator: per-model static lookup table (model id → context window size). Missing models default to 200,000 tokens. No API calls required. **Note: `ModelConfig` already has `context_window: u64` — this field should be passed through to the emit site rather than maintaining a separate lookup table.**
- **D-10:** `WorkflowState` gets `context_pct: u8` field (0 = unknown/not yet received). TUI updates it on each `ContextUsage` event.
- **D-11:** Expanded panel rows appear **only when** `workflow_state.is_auto == true` AND `workflow_state.active == true`. Step mode (`LoopMode::Step`) keeps the existing 2-row panel with no layout change.
- **D-12:** `WorkflowState` gets `is_auto: bool` field. Set to `true` on `AgentEvent::AutoModeStart`, cleared on `AgentEvent::AutoModeEnd`.
- **D-13:** New events: `AgentEvent::AutoModeStart` and `AgentEvent::AutoModeEnd` — emitted by `app.rs` immediately before spawning the auto loop task and on completion/cancellation respectively.
- **D-14:** Model display: `WorkflowState` gets `model_display: String` field. Updated from the existing `AgentEvent::ModelChanged { display_name }` event. No new event needed for model name.
- **D-15:** Footer hints row: `esc pause  │  ? help` — rendered dim (`Palette::DIM`) at the bottom of the auto-mode panel.
- **D-16:** Context % row: mini progress bar — `ctx {pct}%  ████░░░░░░░░` format using `Palette::ACCENT` fill and `Palette::DIM` empty.

### Claude's Discretion

- Exact column widths for the model+blueprint row (left/right split point) — fit to terminal width.
- Truncation strategy for long blueprint names or commit messages (trim to ~30 chars with `…`). UI-SPEC provides exact rules — see UI-SPEC.md Row 2–4 truncation contract.
- Whether `AgentEvent::AutoModeEnd` should also reset `workflow_state.is_auto` immediately or wait for the next render tick. UI-SPEC says: reset immediately (do not wait for render tick).

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within Phase 3 scope.

</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTO-01 | During auto-loop execution, workflow panel shows active blueprint ID/name and current model on one row | D-14 + `model_for_display` var already in `run_tui_loop`; `WorkflowState.model_display` field addition |
| AUTO-02 | During auto-loop execution, workflow panel shows current stage name and section/task being executed | `WorkflowUnitStart` already populates `current_stage`, `current_section`, `current_task` in `WorkflowState`; render row 3 is new |
| AUTO-03 | During auto-loop execution, workflow panel shows short git hash and message of last commit | `WorkflowGit::last_commit()` method addition + async fetch on `WorkflowUnitEnd` + D-06 fields |
| AUTO-04 | During auto-loop execution, workflow panel shows context window usage as a percentage | `AgentEvent::ContextUsage` addition + `ModelConfig.context_window` already present + D-07–D-10 |
| AUTO-05 | During auto-loop execution, workflow panel footer shows `esc pause | ? help` keybind hints | Row 6 render addition, display-only, dim style |
| AUTO-06 | Existing stage pipeline and progress bar preserved — auto-run rows are additive | `render_workflow_panel` inner layout extended conditionally; rows 0–1 constraint blocks unchanged |

</phase_requirements>

---

## Summary

Phase 3 is a well-bounded additive expansion of the existing `render_workflow_panel` function and `WorkflowState` struct. All required data either already flows through the system (stage/section/task, model name) or has a clear low-risk plumbing path (context %, git commit). No new external crates are required — the implementation is pure ratatui layout work plus Rust async plumbing.

The two substantive additions are: (1) a new `AgentEvent::ContextUsage { pct: u8 }` variant that must be emitted after each `AgentEnd` event in the auto loop path, computing `pct` from `usage.input_tokens / model.context_window`; and (2) a `WorkflowGit::last_commit()` async method that runs `git log -1 --format='%h %s'` and is triggered in the `WorkflowUnitEnd` handler inside `app.rs`. The rest is render logic following the well-established `Palette` + `Span` + `Layout::split` pattern already used throughout `widgets.rs`.

The key architecture decision confirmed by source inspection: `ModelConfig` already carries `context_window: u64` for all models, so D-09's "lookup table" requirement is already satisfied — the planner should use `model.context_window` directly at the emit site rather than building a new table.

**Primary recommendation:** Implement in two waves — Wave 1: data plumbing (new `AgentEvent` variants, `WorkflowState` field additions, `WorkflowGit::last_commit()`, context % emit, `AppLayout` third variant); Wave 2: render (extend `render_workflow_panel` with rows 2–6 behind `is_auto && active` gate).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.29 | TUI framework — `Block`, `Paragraph`, `Layout`, `Span`, `Line` | Already in `Cargo.toml`; all existing widgets use it |
| crossterm | 0.28 | Terminal backend | Already in `Cargo.toml`; required by ratatui backend |
| tokio | (existing) | Async runtime for git fetch task | Already used throughout codebase |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::process::Command (via existing `WorkflowGit::run_git`) | stdlib | Shell out to `git log -1` | Used inside new `last_commit()` method |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `git log -1` via `WorkflowGit::run_git` | `git2` crate | `git2` adds binary size; shell-out pattern already established in `WorkflowGit` |
| `AgentEvent::ContextUsage` channel delivery | Direct `WorkflowState` mutation from auto loop | Channel is the established pattern; direct mutation would bypass the drain loop |

**Installation:** No new packages needed. All dependencies already in `Cargo.toml`.

---

## Architecture Patterns

### Recommended Project Structure

No new files required. All changes are within existing modules:

```
src/
├── io/agent_io.rs        — add AutoModeStart, AutoModeEnd, ContextUsage variants
├── tui/
│   ├── app.rs            — AppLayout third variant, event handlers, AutoModeStart/End emit sites
│   └── widgets.rs        — WorkflowState fields, render_workflow_panel extension
└── workflow/
    ├── auto_loop.rs      — ContextUsage emit after AgentEnd (or at run_unit boundary)
    └── git.rs            — add last_commit() async method
```

### Pattern 1: AgentEvent Extension (established by Phase 2)

**What:** Add new variants to `AgentEvent` enum in `agent_io.rs`, handle them in the drain loop in `app.rs`.
**When to use:** Any new signal from workflow engine to TUI.
**Example:**
```rust
// In src/io/agent_io.rs — follow StageTransition pattern
/// Auto-loop started.
AutoModeStart,
/// Auto-loop ended (completed, blocked, or cancelled).
AutoModeEnd,
/// Context window utilization snapshot — emitted after each AgentEnd.
ContextUsage { pct: u8 },
```

Handler in `app.rs` drain loop:
```rust
AgentEvent::AutoModeStart => {
    workflow_state.is_auto = true;
}
AgentEvent::AutoModeEnd => {
    workflow_state.is_auto = false;
}
AgentEvent::ContextUsage { pct } => {
    workflow_state.context_pct = pct;
}
```

### Pattern 2: AppLayout Third Variant

**What:** `AppLayout::compute()` gains a second parameter or a more expressive enum to distinguish normal/auto workflow modes.
**When to use:** When layout height differs based on runtime state.
**Example:**
```rust
// Current signature:
fn compute(area: Rect, wf_active: bool) -> Self

// New signature — add is_auto flag:
fn compute(area: Rect, wf_active: bool, wf_auto: bool) -> Self

// Auto variant allocates Constraint::Length(11) for workflow panel:
// 2 border rows + 2 existing rows + 5 new rows = 9 inner cells + 2 border = 11
Constraint::Length(11), // workflow panel (auto mode)
```

The call site in the main render block must pass `workflow_state.is_auto && workflow_state.active` as `wf_auto`.

### Pattern 3: WorkflowGit::last_commit() Async Method

**What:** New method on existing `WorkflowGit` struct that shells out to `git log -1 --format='%h %s'` and returns `(String, String)`.
**When to use:** Called from async task spawned in `WorkflowUnitEnd` handler.
**Example:**
```rust
// In src/workflow/git.rs
pub async fn last_commit(&self) -> anyhow::Result<(String, String)> {
    let output = self.run_git(&["log", "-1", "--format=%h %s"]).await?;
    let line = output.trim();
    // split at first space
    if let Some(pos) = line.find(' ') {
        Ok((line[..pos].to_string(), line[pos + 1..].to_string()))
    } else {
        Ok((line.to_string(), String::new()))
    }
}
```

Git fetch is triggered from `WorkflowUnitEnd` handler in the drain loop. To deliver the result back to `WorkflowState`, the cleanest approach (consistent with existing patterns) is a dedicated `AgentEvent::GitCommitUpdate { hash: String, msg: String }` variant — or reuse the existing channel by spawning a small async block that sends the event after the git call completes.

### Pattern 4: render_workflow_panel Extension

**What:** `render_workflow_panel` conditionally extends its inner `Layout` from 2 rows to 7 rows when `state.is_auto && state.active`.
**When to use:** Whenever `is_auto` gate is true.
**Example:**
```rust
// In src/tui/widgets.rs
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
```

### Pattern 5: Context % Computation

**What:** After each `AgentEnd` in a unit run, compute utilization from `usage.input_tokens` and `model.context_window`.
**When to use:** Only when `LoopMode::Auto` is active (i.e., in `run_unit` or in the `run_loop` call boundary).
**Example:**
```rust
// After AgentEnd usage is received:
let pct = if model.context_window > 0 {
    ((usage.input_tokens as f64 / model.context_window as f64) * 100.0).min(100.0) as u8
} else {
    0u8
};
let _ = io.emit(AgentEvent::ContextUsage { pct }).await;
```

The emit point: the best location is immediately after `run_unit` returns `Ok(artifacts)` in `run_loop`, since `run_unit` delegates to `runner.run()` which internally calls `run_agent_loop`. The `Usage` from the agent run is not directly returned by `run_unit` — see "Open Questions" for options.

### Anti-Patterns to Avoid

- **Inline `Color::` literals:** The codebase enforces `Palette::` constants for all colors (Phase 1 D-03/D-04). Never use `Color::Yellow` directly in render code — use `Palette::ACCENT`.
- **Blocking git calls in the drain loop:** The drain loop is synchronous; spawning blocking git I/O inside it would freeze rendering. Always spawn an async task or use `tokio::spawn`.
- **Using `chunks[N]` indexing:** `AppLayout` was extracted precisely to prevent this. All layout areas accessed via named struct fields.
- **Applying Accent color to git or model rows:** UI-SPEC reserves `Palette::ACCENT` for exactly 3 elements — active pipeline `●`, context bar fill, current stage in panel title. Not for body text rows.
- **Emitting `AutoModeStart/End` from inside `auto_loop.rs`:** These are TUI-layer signals (D-13). They must be emitted from `app.rs` at the spawn site, not from inside the loop, to keep the loop backend-agnostic.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Git log parsing | Custom git-format parser | `git log -1 --format='%h %s'` via `WorkflowGit::run_git` | Already provides exact fields; no parsing ambiguity |
| Context window sizes | Static lookup table in Phase 3 code | `ModelConfig.context_window` field | Already defined for all 10+ models in `models.rs`; single source of truth |
| Progress bar rendering | Custom block-char renderer | Copy existing progress bar pattern from `widgets.rs:447–478` | Pattern already handles bar width calculation; copy + adjust colors |
| Unicode ellipsis truncation | `format!("{}...", s)` | `format!("{}…", s)` with U+2026 | UI-SPEC mandates single-char ellipsis; `…` is one grapheme cluster, `...` is three |

**Key insight:** The hardest problem (context window denominator) is already solved. `ModelConfig.context_window` was designed for exactly this use case.

---

## Common Pitfalls

### Pitfall 1: run_unit Does Not Return Usage

**What goes wrong:** `run_unit` returns `anyhow::Result<Vec<StageArtifact>>` — not `(Vec<StageArtifact>, Usage)`. The `Usage` from the inner `run_agent_loop` call is swallowed inside `runner.run()`.
**Why it happens:** The stage runner trait returns only artifacts; usage tracking was not in scope for the stage runner interface.
**How to avoid:** Two options:
  1. Emit `ContextUsage` from within the stage runner/agent loop itself (where `AgentEnd { usage }` is already processed). This requires the stage runner to call `io.emit(AgentEvent::ContextUsage { pct })` after computing pct from `model.context_window`.
  2. Extend `run_unit` return type to `(Vec<StageArtifact>, Usage)` and emit from `run_loop`. Option 1 is lower risk (no signature change).
**Warning signs:** `context_pct` stays at 0 throughout auto run.

### Pitfall 2: AppLayout Height Mismatch

**What goes wrong:** Auto-mode panel allocated `Constraint::Length(4)` (existing) instead of `Constraint::Length(11)`. Inner layout splits 7 rows into a 2-row space, producing invisible/clipped content.
**Why it happens:** `AppLayout::compute` has the existing 2-variant switch; easy to forget the third variant or pass wrong boolean.
**How to avoid:** The third variant must use `Constraint::Length(11)` (or equivalent: 2 borders + 7 inner rows). Verify by counting: pipeline(1) + progress(1) + model(1) + stage(1) + commit(1) + ctx(1) + footer(1) = 7 inner + 2 border = 9. UI-SPEC says output area shrinks by 7 rows — reconcile: existing panel is 4 rows total (2 inner + 2 border), auto panel is 9 rows total (7 inner + 2 border) — net difference is 5 rows.
**Warning signs:** New rows not visible, or output area is negative height and panics.

### Pitfall 3: is_auto Not Cleared on Workflow End

**What goes wrong:** `workflow_state.is_auto` stays `true` after auto loop ends, permanently showing the expanded panel even in non-auto state.
**Why it happens:** `AutoModeEnd` handler is missing or fires before `WorkflowComplete`/`WorkflowBlocked` clears `workflow_state.active`, so the gate `is_auto && active` appears satisfied.
**How to avoid:** Emit `AutoModeEnd` before the loop future completes (in the completion handler in `app.rs`). In the drain loop handler, set `is_auto = false` unconditionally. The `WorkflowComplete` handler already sets `active = false` — so `is_auto && active` will be false regardless, but `is_auto` should still be cleared for correctness.
**Warning signs:** Expanded panel visible after workflow completes.

### Pitfall 4: Git Fetch Race with render_workflow_panel

**What goes wrong:** Git fetch async task completes and sends result, but the channel delivery races with multiple drain-loop iterations. If `GitCommitUpdate` event arrives during rapid flooding of other events, the last known value might briefly display stale data.
**Why it happens:** The drain loop drains all pending events per frame tick — order is non-deterministic when multiple events queued.
**How to avoid:** This is acceptable per D-05 — "last known value shown until next update." No special handling needed. Empty state shows `—` (em dash), not an error.
**Warning signs:** None needed — by design.

### Pitfall 5: model_display Not Populated Before AutoModeStart

**What goes wrong:** `workflow_state.model_display` is empty string when panel first renders, showing a blank model row.
**Why it happens:** `ModelChanged` event is only emitted when the model changes — if the user never changed models, the event never fires before auto mode starts.
**How to avoid:** Initialize `workflow_state.model_display` from `model_for_display` (the local variable in `run_tui_loop`) when `AutoModeStart` is handled — not just from `ModelChanged` events. Or pre-populate it at startup.
**Warning signs:** Model+blueprint row shows `  │  blueprint-name` with empty left side.

### Pitfall 6: Context % Exceeds 100 Due to Cache Tokens

**What goes wrong:** `input_tokens` in `Usage` may include cache read/write tokens which can push the sum above the context window size, resulting in `pct > 100`.
**Why it happens:** Anthropic token counting includes cache tokens in input_tokens in some API responses.
**How to avoid:** Clamp with `.min(100.0) as u8` in the pct calculation. Already included in the recommended pattern above.
**Warning signs:** Context bar shows more than 100% fill.

---

## Code Examples

Verified patterns from existing source:

### Existing Progress Bar (Copy Target for Context % Bar)
```rust
// Source: src/tui/widgets.rs lines 447–478
let bar_width = layout[1].width.saturating_sub(label.len() as u16 + 12);
let filled = if total > 0 {
    ((done as f64 / total as f64) * bar_width as f64) as u16
} else {
    0
};
let empty = bar_width.saturating_sub(filled);
let progress_spans = vec![
    Span::styled(&label, Style::default().fg(Palette::TEXT)),
    Span::styled("█".repeat(filled as usize), Style::default().fg(Palette::SUCCESS)),
    Span::styled("░".repeat(empty as usize), Style::default().fg(Palette::DIM)),
    Span::styled(format!("  {}/{} tasks", done, total), Style::default().fg(Palette::TEXT)),
];
```

Context % bar adaptation:
```rust
// Row 5: context % bar
let ctx_label = if state.context_pct == 0 {
    "ctx  ?%".to_string()
} else {
    format!("ctx {:>2}%", state.context_pct)
};
let bar_width = layout[5].width.saturating_sub(10);
let filled = ((state.context_pct as f64 / 100.0) * bar_width as f64) as u16;
let empty = bar_width.saturating_sub(filled);
let ctx_spans = vec![
    Span::styled(format!("{:<8}  ", ctx_label), Style::default().fg(Palette::TEXT)),
    Span::styled("█".repeat(filled as usize), Style::default().fg(Palette::ACCENT)),
    Span::styled("░".repeat(empty as usize), Style::default().fg(Palette::DIM)),
];
f.render_widget(Paragraph::new(Line::from(ctx_spans)), layout[5]);
```

### Model + Blueprint Row
```rust
// Row 2: model + blueprint
let model_str = &state.model_display;
let bp_str = truncate_str(&state.blueprint_id, 30);
let model_width = model_str.chars().count() as u16;
let sep = "  │  ";
let model_spans = vec![
    Span::styled(model_str.as_str(), Style::default().fg(Palette::TEXT)),
    Span::styled(sep, Style::default().fg(Palette::DIM)),
    Span::styled(bp_str, Style::default().fg(Palette::TEXT)),
];
f.render_widget(Paragraph::new(Line::from(model_spans)), layout[2]);
```

### Truncation Helper
```rust
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}
```

### WorkflowState Field Additions
```rust
// In src/tui/widgets.rs — WorkflowState struct additions
pub struct WorkflowState {
    // ... existing fields ...
    pub is_auto: bool,
    pub model_display: String,
    pub last_commit_hash: String,
    pub last_commit_msg: String,
    pub context_pct: u8,
}
```

### AgentEvent New Variants
```rust
// In src/io/agent_io.rs — add to AgentEvent enum
/// Auto-loop execution started.
AutoModeStart,
/// Auto-loop execution ended (completed, blocked, or cancelled).
AutoModeEnd,
/// Context window utilization — emitted once per agent turn completion.
ContextUsage { pct: u8 },
/// Git commit update — result of async git log fetch after WorkflowUnitEnd.
GitCommitUpdate { hash: String, msg: String },
```

### AppLayout Third Variant
```rust
// In src/tui/app.rs — AppLayout::compute() extended
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
            output:   chunks[0],
            workflow: Some(chunks[1]),
            status:   chunks[3],
            input:    chunks[4],
        }
    } else if wf_active {
        // existing 2-row panel variant ...
    } else {
        // existing no-panel variant ...
    }
}
```

---

## Runtime State Inventory

Phase 3 is not a rename/refactor/migration phase.

None — not applicable.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `chunks[N]` indexing | Named `AppLayout` struct fields | Phase 1 (complete) | Safe to add 3rd layout variant without index breakage |
| Inline `Color::` literals | `Palette::` constants | Phase 1 (complete) | All new render code must use `Palette::` only |
| No workflow events | `AgentEvent::WorkflowUnitStart/End/Progress` | Phase 1/2 | Established pattern for Phase 3 additions |
| No stage transitions | `AgentEvent::StageTransition` | Phase 2 | Template for `AutoModeStart/End/ContextUsage` additions |

---

## Open Questions

1. **Where exactly does `ContextUsage` get emitted?**
   - What we know: `run_unit` calls `runner.run()` which calls `run_agent_loop`. The `Usage` is tracked in `AgentState.cumulative_usage` inside `run_agent_loop`. `AgentEnd { usage }` is already emitted by the agent loop via `io.emit()`.
   - What's unclear: Should `ContextUsage` be emitted from inside `run_agent_loop` (where `AgentEnd` fires) or from `run_loop` after `run_unit` returns?
   - Recommendation: Emit from inside the existing `AgentEnd` handler in `run_agent_loop` — the `model.context_window` is available there. This requires passing `model.context_window` to the emit logic. Alternatively, add a post-`AgentEnd` hook in `auto_loop.rs::run_unit` that emits after the inner agent run if a usage channel is plumbed through. **Option A (emit from agent loop) is lower-risk and requires no signature changes.** The planner should decide and document.

2. **GitCommitUpdate delivery mechanism**
   - What we know: git fetch must be async, non-blocking to the drain loop.
   - What's unclear: whether to use a new `AgentEvent::GitCommitUpdate` variant (clean, matches existing pattern) or a separate dedicated mpsc channel.
   - Recommendation: Add `AgentEvent::GitCommitUpdate { hash: String, msg: String }` — keeps all TUI state updates flowing through one channel, consistent with established pattern. CONTEXT.md D-95 (code_context section) mentions this exact option.

---

## Environment Availability

Step 2.6: All dependencies are existing in-project Rust crates. No external tools required beyond `git` (already used by `WorkflowGit`). Git is verified available since `WorkflowGit::run_git` is exercised in existing phases.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| git CLI | `WorkflowGit::last_commit()` | Assumed present (used in all existing phases) | — | Empty string `—` display (D-06 empty state) |
| ratatui 0.29 | Panel render | Present in `Cargo.toml` | 0.29 | — |
| crossterm 0.28 | TUI backend | Present in `Cargo.toml` | 0.28 | — |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test` |
| Config file | `Cargo.toml` (no separate test config) |
| Quick run command | `cargo test` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTO-01 | `WorkflowState` with `is_auto=true` holds `model_display` string | unit | `cargo test test_workflow_state_auto_fields` | Wave 0 |
| AUTO-02 | `WorkflowState` `current_stage` + `current_section` populated from `WorkflowUnitStart` | unit | `cargo test test_workflow_state_unit_start` | Wave 0 |
| AUTO-03 | `WorkflowGit::last_commit()` returns `(hash, msg)` tuple split correctly | unit | `cargo test test_last_commit_parse` | Wave 0 |
| AUTO-04 | `context_pct` calculation: 50k tokens / 200k window = 25% | unit | `cargo test test_context_pct_calculation` | Wave 0 |
| AUTO-04 | `context_pct` clamped at 100 when tokens exceed window | unit | `cargo test test_context_pct_clamped` | Wave 0 |
| AUTO-05 | Footer hints row renders dim text `esc pause  │  ? help` | unit | `cargo test test_render_footer_hints` | Wave 0 |
| AUTO-06 | Pipeline and progress bar render identically whether `is_auto` is true or false | unit | `cargo test test_pipeline_row_unchanged_in_auto` | Wave 0 |

All tests in `src/tui/widgets.rs` under `#[cfg(test)]` — following the existing pattern (lines 638–711).

### Sampling Rate
- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full `cargo test` green + `cargo build --features tui` clean before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/tui/widgets.rs` — `test_workflow_state_auto_fields`: verify `WorkflowState` default for new fields (`is_auto=false`, `model_display=""`, `last_commit_hash=""`, `last_commit_msg=""`, `context_pct=0`)
- [ ] `src/tui/widgets.rs` — `test_context_pct_calculation`: pure function unit test — `50_000u64 / 200_000u64 * 100 = 25u8`
- [ ] `src/tui/widgets.rs` — `test_context_pct_clamped`: verify result does not exceed 100 when input_tokens > context_window
- [ ] `src/workflow/git.rs` — `test_last_commit_parse`: unit test for output line parsing logic extracted into a pure function (separate from async git call)
- [ ] `src/tui/widgets.rs` — `test_pipeline_row_unchanged_in_auto`: snapshot/equality test confirming pipeline spans are identical regardless of `is_auto`

---

## Sources

### Primary (HIGH confidence)
- Direct source inspection — `src/tui/widgets.rs`, `src/tui/app.rs`, `src/io/agent_io.rs`, `src/workflow/auto_loop.rs`, `src/workflow/git.rs`, `src/llm/models.rs`, `src/agent/state.rs`
- `.planning/phases/03-auto-run-panel/03-CONTEXT.md` — all decisions (D-01 through D-16) are user-confirmed locked choices
- `.planning/phases/03-auto-run-panel/03-UI-SPEC.md` — row-by-row render contract, truncation rules, color contract

### Secondary (MEDIUM confidence)
- `cargo test` output — 188 lib tests + 17 integration tests all pass, confirming baseline stability
- `Cargo.toml` — confirmed ratatui 0.29, crossterm 0.28

### Tertiary (LOW confidence)
- None — all claims verified from source code.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified from `Cargo.toml`; no new dependencies
- Architecture: HIGH — all patterns verified from existing source code; no speculative design
- Pitfalls: HIGH — identified from direct reading of `run_unit` return type, `AppLayout` constraint math, `model_for_display` initialization path
- Open questions: MEDIUM — one emit-point ambiguity requires planner decision (Option A vs B)

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable Rust codebase; no external API surface)

## Project Constraints (from CLAUDE.md)

Per the global `~/.claude/CLAUDE.md`:

1. Read relevant files before making changes — done (all source files read).
2. Build and test after major code changes — plan must include `cargo build --features tui` and `cargo test` steps at each wave boundary.
3. All files must include the copyright header: `// Fin — [File Purpose] / // Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>` — all modified files must have this.
4. Keep changes simple and minimal — avoid over-engineering. The phase adds 5 new fields to `WorkflowState`, 3–4 new `AgentEvent` variants, 1 new git method, and extends 1 render function. This is the minimal surface.
5. Save plan files in `.plans` folder — gsd saves to `.planning/phases/` which satisfies this intent.
6. No Color::Rgb — ANSI named colors only (Phase 1 D-04 / STATE.md decision).
