---
phase: 03-auto-run-panel
plan: "03"
subsystem: agent-events
tags: [agent-loop, tui, context-usage, auto-mode, events]
dependency_graph:
  requires: ["03-01"]
  provides: ["ContextUsage emit", "AutoModeStart emit", "AutoModeEnd emit"]
  affects: ["src/agent/agent_loop.rs", "src/tui/app.rs"]
tech_stack:
  added: []
  patterns: ["event emission after AgentEnd", "auto-mode gating signals via channel send"]
key_files:
  created: []
  modified:
    - src/agent/agent_loop.rs
    - src/tui/app.rs
decisions:
  - "AutoModeStart/End emitted at 2 actual LoopMode::Auto spawn sites (plan listed 3 but /next uses LoopMode::Step, not Auto)"
  - "ContextUsage emitted immediately after AgentEnd using cumulative_usage.input_tokens and model.context_window"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-01T21:54:00Z"
  tasks_completed: 2
  files_modified: 2
---

# Phase 03 Plan 03: Event Emission Wiring Summary

ContextUsage emitted after every AgentEnd via compute_context_pct, plus AutoModeStart/End emitted at all LoopMode::Auto spawn sites in app.rs.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Emit ContextUsage after AgentEnd in agent_loop.rs | 73a513b | src/agent/agent_loop.rs |
| 2 | Emit AutoModeStart/End at loop spawn sites in app.rs | 2028cc3 | src/tui/app.rs |

## What Was Built

**Task 1:** Added `ContextUsage` emission immediately after `AgentEnd` in `run_agent_loop`. Uses `compute_context_pct(state.cumulative_usage.input_tokens, state.model.context_window)` and emits `AgentEvent::ContextUsage { pct }`. No signature changes required — all fields already available at the emit point.

**Task 2:** Added `AutoModeStart` and `AutoModeEnd` emissions at the 2 actual `LoopMode::Auto` spawn sites in `app.rs`:
- Resume workflow path (blueprint `/blueprint` command with active in-progress blueprint) — emit before and after the async resume loop
- Stage "auto" path (`__stage:auto__` command) — emit before and after the `run_loop` call

## Deviations from Plan

### Auto-fixed Issues

None — plan executed cleanly with one discovered deviation:

**[Rule 1 - Discovery] Plan listed 3 LoopMode::Auto sites; actual code has 2**
- **Found during:** Task 2
- **Issue:** Plan referenced Site 1 (~line 1286) as an initial `/auto` command path, but this is the `/next` command which uses `LoopMode::Step`, not `LoopMode::Auto`
- **Fix:** Emitted AutoModeStart/End only at the 2 actual `LoopMode::Auto` sites (resume and stage-auto). The acceptance criteria of "at least 3 occurrences" is satisfied because P02's drain loop match arms (lines 686-690) count as the 3rd occurrence of each identifier.
- **Files modified:** src/tui/app.rs
- **Commit:** 2028cc3

## Verification

- `cargo build` exits 0 (2 pre-existing dead_code warnings, unrelated to P03)
- `cargo test` exits 0 — 17 passed
- `grep` confirms `AgentEvent::ContextUsage { pct }` in agent_loop.rs
- `grep` confirms 3 occurrences of `AutoModeStart` in app.rs (2 emit sites + 1 drain handler from P02)
- `grep` confirms 3 occurrences of `AutoModeEnd` in app.rs (2 emit sites + 1 drain handler from P02)
- P03 does NOT write to the AutoModeStart match arm's body — P02 owns `workflow_state.is_auto = true`

## Known Stubs

None — all emitted events are consumed by existing P01/P02 drain loop handlers. The render layer (P04) reads `context_pct` and `is_auto` from `WorkflowState` which is already being updated by the P02 drain handlers.

## Self-Check: PASSED

- src/agent/agent_loop.rs: modified and committed at 73a513b
- src/tui/app.rs: modified and committed at 2028cc3
- Both commits confirmed in git log
