# Phase 3: Auto-Run Panel - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 expands the existing workflow panel to provide complete live context during autonomous execution. Scope: dynamic panel height that grows in auto mode, plus 5 new inner rows (model+blueprint, stage+section, last git commit, context % bar, keybind footer). Existing pipeline and progress bar rows are preserved and unchanged. No new layout panels — this is an additive expansion of `render_workflow_panel`. New events: `AgentEvent::AutoModeStart`, `AgentEvent::AutoModeEnd`, `AgentEvent::ContextUsage { pct: u8 }`.

</domain>

<decisions>
## Implementation Decisions

### Panel Layout

- **D-01:** Dynamic height — `AppLayout` computes a taller workflow `Rect` when `workflow_state.is_auto` is true. This is a third layout variant alongside the existing "with workflow panel" and "without workflow panel" variants. When auto mode is active, the output area shrinks by approximately 5 rows to accommodate the new inner rows.
- **D-02:** Auto-mode panel inner layout (7-8 rows): pipeline row, progress bar row, model+blueprint row, stage+section row, last commit row, context % bar row, footer hints row.
- **D-03:** Existing pipeline (✓/●/○) and progress bar rows are **unchanged** — auto-run rows are additive below them. AUTO-06 compliance.

### Git Commit Display

- **D-04:** Source: latest repo commit regardless of author — `git log -1 --format='%h %s'`. Shows whatever was last committed (including Fin auto-commits and manual commits). Matches what a developer expects to see in a status bar.
- **D-05:** Fetch timing: async on `WorkflowUnitEnd` — after each unit completes (where auto-commits happen), run `git log -1` and store result in `WorkflowState`. Non-blocking render; last known value shown until next update.
- **D-06:** Fields added to `WorkflowState`: `last_commit_hash: String`, `last_commit_msg: String`. Populated from the async fetch, empty string until first fetch completes.

### Context % Display

- **D-07:** Delivery: new `AgentEvent::ContextUsage { pct: u8 }` variant — add to `src/io/agent_io.rs` alongside `StageTransition`. Emitted once per turn, after `TurnEnd`, using the cumulative `Usage` stats already tracked.
- **D-08:** Emit point: after `TurnEnd` in the agent turn loop. The emitter computes `pct = (input_tokens / context_window_size * 100) as u8` and sends the event.
- **D-09:** Denominator: per-model static lookup table (model id → context window size). Missing models default to 200,000 tokens. No API calls required.
- **D-10:** `WorkflowState` gets `context_pct: u8` field (0 = unknown/not yet received). TUI updates it on each `ContextUsage` event.

### Auto Mode Gating

- **D-11:** Expanded panel rows appear **only when** `workflow_state.is_auto == true` AND `workflow_state.active == true`. Step mode (`LoopMode::Step`) keeps the existing 2-row panel with no layout change.
- **D-12:** `WorkflowState` gets `is_auto: bool` field. Set to `true` on `AgentEvent::AutoModeStart`, cleared on `AgentEvent::AutoModeEnd`.
- **D-13:** New events: `AgentEvent::AutoModeStart` and `AgentEvent::AutoModeEnd` — emitted by `app.rs` immediately before spawning the auto loop task and on completion/cancellation respectively. These are TUI-layer signals (not workflow engine signals).
- **D-14:** Model display: `WorkflowState` gets `model_display: String` field. Updated from the existing `AgentEvent::ModelChanged { display_name }` event (already emitted). No new event needed for model name.

### Render Layout

- **D-15:** Footer hints row: `esc pause  │  ? help` — rendered dim (`Palette::DIM`) at the bottom of the auto-mode panel. Mirrors the Phase 2 decision to show keybind hints contextually.
- **D-16:** Context % row: mini progress bar similar to the existing progress bar row — `ctx {pct}%  ████░░░░░░░░` format using `Palette::ACCENT` fill and `Palette::DIM` empty.

### Claude's Discretion

- Exact column widths for the model+blueprint row (left/right split point) — fit to terminal width.
- Truncation strategy for long blueprint names or commit messages (trim to ~30 chars with `…`).
- Whether `AgentEvent::AutoModeEnd` should also reset `workflow_state.is_auto` immediately or wait for the next render tick.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core planning files
- `.planning/REQUIREMENTS.md` — Phase 3 requirements: AUTO-01, AUTO-02, AUTO-03, AUTO-04, AUTO-05, AUTO-06
- `.planning/ROADMAP.md` — Phase 3 goal and success criteria (5 criteria)
- `.planning/phases/01-foundation/01-CONTEXT.md` — AppLayout struct decisions (D-05, D-06), Palette constants (D-01–D-04)
- `.planning/phases/02-overlays/02-CONTEXT.md` — AgentEvent::StageTransition pattern (D-11, D-12), WorkflowState field additions precedent

### TUI source files to read before planning
- `src/tui/widgets.rs` — `WorkflowState` struct (line 338), `render_workflow_panel` (line 404), `Palette` const, existing pipeline + progress bar render logic
- `src/tui/app.rs` — `AppLayout` struct, layout variant logic, AgentEvent drain loop (lines 600–660), `WorkflowUnitEnd` handler, `ModelChanged` handler, auto loop spawn site (lines ~1591, ~1875–1886)
- `src/io/agent_io.rs` — `AgentEvent` enum (add `AutoModeStart`, `AutoModeEnd`, `ContextUsage` here)
- `src/workflow/auto_loop.rs` — `run_loop()` function, `WorkflowUnitEnd` emit point (for async git fetch hook), `LoopMode` enum
- `src/workflow/git.rs` — `WorkflowGit` methods (add `last_commit()` returning `(hash, msg)`)

No external specs — requirements fully captured in decisions above.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`render_workflow_panel()`** (`widgets.rs:404`): existing pipeline + progress bar render — Phase 3 extends this function with conditional auto-mode rows below the existing 2 rows
- **`WorkflowState`** (`widgets.rs:338`): add `is_auto: bool`, `model_display: String`, `last_commit_hash: String`, `last_commit_msg: String`, `context_pct: u8`
- **`AppLayout`** (`app.rs:34–78`): add third layout variant for auto mode (taller workflow `Rect`)
- **`WorkflowGit`** (`workflow/git.rs`): add `async fn last_commit()` returning `(String, String)` — wraps `git log -1 --format='%h %s'`
- **Progress bar render** (`widgets.rs:447–478`): copy pattern for context % mini-bar

### Established Patterns
- `AgentEvent::StageTransition` addition (Phase 2): precedent for adding new workflow events to `agent_io.rs` + no-op in non-TUI backends
- Layout variant logic in `app.rs`: existing 2-variant approach (with/without workflow panel) extends to 3 variants
- `model_picker_active` / `help_active` bool flags: same pattern for `is_auto` in `WorkflowState`

### Integration Points
- **`AgentEvent::AutoModeStart/End`**: emitted at the auto loop spawn site in `app.rs` (~line 1591) — before spawn and in the `.await` completion handler
- **`AgentEvent::ContextUsage`**: emitted in the turn loop after `TurnEnd` — wherever `AgentEnd { usage }` is processed in the agent runner
- **Git fetch**: triggered in `WorkflowUnitEnd` handler in `app.rs` drain loop — spawn async task, send result back via a dedicated mpsc channel or embed in a new `AgentEvent::GitCommitUpdate { hash, msg }`

</code_context>

<specifics>
## Specific Ideas

- Layout mockup confirmed by user:
  ```
  ┌─ blueprint-name ─────────────────────────┐
  │ Define ✓  Explore ✓  Build ●  Validate ○  │  <- pipeline (unchanged)
  │ ████████░░░░  6/12 tasks                  │  <- progress (unchanged)
  │ claude-opus-4-6  │  blueprint-name        │  <- NEW: model + id
  │ Build › section-03                        │  <- NEW: stage + section
  │ abc1234 initial scaffold                  │  <- NEW: last commit
  │ ctx 34%  ██████░░░░░░░░░░░░               │  <- NEW: context bar
  │ esc pause  │  ? help                      │  <- NEW: footer hints
  └──────────────────────────────────────────┘
  ```
- Git commit format confirmed: `abc1234 feat: scaffold section-03 task files` (short hash + subject)
- Context % bar: same visual style as the existing progress bar (filled/empty block chars)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 3 scope.

</deferred>

---

*Phase: 03-auto-run-panel*
*Context gathered: 2026-04-01*
