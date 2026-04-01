---
phase: 03-auto-run-panel
plan: "04"
subsystem: tui/render
tags: [auto-run-panel, widgets, layout, tui, wave-3]
dependency_graph:
  requires: ["03-01", "03-02", "03-03"]
  provides: [render_workflow_panel_auto_mode, AppLayout_auto_variant, FOOTER_HINTS, truncate_str]
  affects: [src/tui/widgets.rs, src/tui/app.rs]
tech_stack:
  added: []
  patterns: [conditional-row-layout, truncation-with-ellipsis, palette-constants, context-bar-fill]
key_files:
  created: []
  modified:
    - src/tui/widgets.rs
    - src/tui/app.rs
decisions:
  - "AppLayout::compute third variant uses Constraint::Length(9) for auto mode (7 inner rows + 2 border)"
  - "Rows 0-1 pipeline and progress bar left completely unchanged per AUTO-06"
  - "truncate_str uses U+2026 ellipsis and operates on char boundary to handle multibyte correctly"
  - "Pre-existing cargo fmt and clippy issues in unrelated files (llm/mod.rs, etc.) are out of scope per deviation rules"
metrics:
  duration: "~20 minutes"
  completed: "2026-04-01"
  tasks_completed: 1
  tasks_total: 2
  files_modified: 2
  status: checkpoint-reached
---

# Phase 03 Plan 04: Render Layer — Auto-Run Panel Rows Summary

**One-liner:** Conditional 7-row auto-mode expansion of render_workflow_panel with model+blueprint, stage+section, git commit, context bar, and footer hints rows; AppLayout third variant allocates Constraint::Length(9) for auto mode.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add AppLayout third variant, extend render_workflow_panel, add test_render_footer_hints | ee27093 | src/tui/app.rs, src/tui/widgets.rs |

## What Was Built

### Task 1: AppLayout third variant + render_workflow_panel auto-mode rows (commit ee27093)

**Part A — app.rs:**
- `AppLayout::compute` signature extended from `(area, wf_active)` to `(area, wf_active, wf_auto)`
- New first branch: `if wf_active && wf_auto` — allocates `Constraint::Length(9)` for the workflow panel (7 inner rows + 2 border rows)
- Existing `wf_active` (standard) branch unchanged
- Existing no-panel branch unchanged
- Single call site updated: `AppLayout::compute(f.area(), wf_active, workflow_state.is_auto)`

**Part B — widgets.rs:**
- `FOOTER_HINTS: &str = "esc pause  │  ? help"` constant (AUTO-05, testable)
- `truncate_str(s: &str, max: usize) -> String` helper using U+2026 ellipsis at char boundary
- `render_workflow_panel` inner layout is now conditional on `state.is_auto && state.active`:
  - Normal mode: 2-row layout (rows 0-1, unchanged — AUTO-06)
  - Auto mode: 7-row layout (rows 0-6)
- Row 0: Stage pipeline — untouched (AUTO-06)
- Row 1: Progress bar — untouched (AUTO-06)
- Row 2: Model display + blueprint ID (Palette::TEXT, Palette::DIM separator) — AUTO-01
- Row 3: Stage (Palette::ACCENT) + "›" + section/task (Palette::TEXT) — AUTO-02
- Row 4: Git commit hash (Palette::DIM) + subject (Palette::TEXT); em dash (—) when hash empty — AUTO-03
- Row 5: Context % bar — `ctx_label` prefix + filled blocks (Palette::ACCENT) + empty blocks (Palette::DIM) — AUTO-04
- Row 6: Footer hints via FOOTER_HINTS constant (Palette::DIM) — AUTO-05
- All new render code uses only Palette:: constants (zero inline Color:: literals)

**Part C — test:**
- `test_render_footer_hints` added to `#[cfg(test)] mod tests` — verifies FOOTER_HINTS value and content

## Verification Results

- `cargo build` — exit 0, no new errors or warnings
- `cargo test` — 197 tests passed, 0 failed
- `cargo test test_render_footer_hints` — passes (exit 0)
- `cargo fmt` applied to src/tui/app.rs and src/tui/widgets.rs — clean
- Pre-existing clippy issue in src/llm/mod.rs (print-literal in unrelated file) — out of scope per deviation rules, exists before this plan's changes

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree missing P01-P03 changes**
- **Found during:** Pre-task setup
- **Issue:** The worktree branch (worktree-agent-aaba3ce2) was behind main and lacked all P01-P03 commits. WorkflowState had no is_auto/model_display/etc. fields. The app.rs had neither AppLayout struct nor drain loop handlers.
- **Fix:** Ran `git merge main` (fast-forward) before starting Task 1. All P01-P03 changes landed cleanly.
- **Files modified:** All P01-P03 files (merge, not new edits)
- **Commit:** 50d5073 (pre-existing merge commit, now in worktree)

**2. [Note] Pre-existing cargo fmt failures in merged files**
- **Found during:** Post-build verification
- **Issue:** Merging main brought in files with pre-existing cargo fmt issues (agent_loop.rs, prompt.rs, cli.rs, llm/mod.rs, etc.)
- **Action:** Ran `cargo fmt` only on the two files this plan modifies (app.rs, widgets.rs). Out-of-scope files not touched.

**3. [Note] Pre-existing clippy error in llm/mod.rs**
- **Found during:** `cargo clippy -- -D warnings`
- **Issue:** `src/llm/mod.rs:54` has a `print-literal` warning that existed before this plan
- **Action:** Not fixed — out of scope per deviation rules. Confirmed pre-existing by stash test.

## Checkpoint Status

**Task 2 (checkpoint:human-verify)** reached — execution paused for human visual verification.

## Known Stubs

None — all render rows consume live data from WorkflowState fields populated by P01-P03 drain loop handlers. No hardcoded placeholder values.

## Self-Check: PASSED

- `src/tui/app.rs` contains `fn compute(area: Rect, wf_active: bool, wf_auto: bool)` — confirmed
- `src/tui/app.rs` contains `Constraint::Length(9)` — confirmed
- `src/tui/widgets.rs` contains `fn truncate_str(s: &str, max: usize) -> String` — confirmed
- `src/tui/widgets.rs` contains `state.is_auto && state.active && layout.len() >= 7` — confirmed
- `src/tui/widgets.rs` contains `pub const FOOTER_HINTS: &str = "esc pause  │  ? help"` — confirmed
- `src/tui/widgets.rs` contains `fn test_render_footer_hints` — confirmed
- `src/tui/widgets.rs` contains `ctx_label` — confirmed
- `src/tui/widgets.rs` contains `"—"` for empty git state — confirmed
- ee27093 commit confirmed in git log
- cargo build exit 0 — confirmed
- cargo test 197 passed — confirmed
