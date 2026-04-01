---
phase: 03-auto-run-panel
plan: 01
subsystem: tui/data-model
tags: [agent-events, workflow-state, data-model, wave-0, tests]
dependency_graph:
  requires: []
  provides: [AgentEvent::AutoModeStart, AgentEvent::AutoModeEnd, AgentEvent::ContextUsage, AgentEvent::GitCommitUpdate, WorkflowState::is_auto, WorkflowState::model_display, WorkflowState::last_commit_hash, WorkflowState::last_commit_msg, WorkflowState::context_pct, compute_context_pct, parse_git_log_line]
  affects: [src/io/agent_io.rs, src/tui/widgets.rs, src/workflow/git.rs]
tech_stack:
  added: []
  patterns: [pure-functions, data-model-extension, wave-0-test-stubs]
key_files:
  created: []
  modified:
    - src/io/agent_io.rs
    - src/tui/widgets.rs
    - src/workflow/git.rs
decisions:
  - "StageTransition variant added to AgentEvent — referenced in plan interface spec but missing from actual file; added as part of the contract layer"
  - "test_workflow_state_unit_start searches for 'Build' (capitalized) not 'build' to match actual stage_pipeline display key names"
metrics:
  duration: ~8 minutes
  completed: 2026-04-01
  tasks_completed: 2
  files_modified: 3
---

# Phase 03 Plan 01: Data Model — AgentEvent Variants and WorkflowState Fields Summary

**One-liner:** Extended AgentEvent with 5 new variants and WorkflowState with 5 new fields plus two pure helper functions, all covered by 7 passing Wave 0 tests.

## What Was Built

### Task 1: AgentEvent variants + WorkflowState fields (commit c9a6e53)

Added to `src/io/agent_io.rs`:
- `StageTransition { from: String, to: String }` — was referenced in plan interface spec but missing from the actual file
- `AutoModeStart` — TUI-layer signal when auto-loop starts (D-13)
- `AutoModeEnd` — TUI-layer signal when auto-loop ends (D-13)
- `ContextUsage { pct: u8 }` — context window utilization per turn (D-07)
- `GitCommitUpdate { hash: String, msg: String }` — async git log result after WorkflowUnitEnd

Added to `src/tui/widgets.rs`:
- `WorkflowState.is_auto: bool` (default: false)
- `WorkflowState.model_display: String` (default: "")
- `WorkflowState.last_commit_hash: String` (default: "")
- `WorkflowState.last_commit_msg: String` (default: "")
- `WorkflowState.context_pct: u8` (default: 0)
- `compute_context_pct(input_tokens: u64, context_window: u64) -> u8` — pure, clamped at 100
- `parse_git_log_line(line: &str) -> (String, String)` — pure, splits hash from subject

### Task 2: Wave 0 test stubs (commit 0fff93f)

Added 6 tests to `src/tui/widgets.rs` `#[cfg(test)] mod tests` (new module — none existed before):
- `test_workflow_state_auto_fields` — verifies all 5 new field defaults
- `test_workflow_state_unit_start` — simulates WorkflowUnitStart drain-loop state update
- `test_context_pct_calculation` — covers 0%, 25%, 50%, 100% cases
- `test_context_pct_clamped` — verifies clamping above 100% (cache token inflation)
- `test_context_pct_zero_window` — verifies zero-division guard returns 0
- `test_pipeline_row_unchanged_in_auto` — verifies is_auto has no effect on pipeline status

Added 1 test to `src/workflow/git.rs`:
- `test_last_commit_parse` — covers normal, no-subject, multi-space-subject, and empty input cases

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed case-sensitive stage name mismatch in test**
- **Found during:** Task 2
- **Issue:** Plan's test code searched `stage_pipeline` for `name == "build"` but the pipeline stores display names with capitals ("Build", "Define", etc.). The search would always return `None`, causing `assert!(build_status.is_some())` to fail.
- **Fix:** Changed search key from `"build"` to `"Build"` to match the actual pipeline data.
- **Files modified:** src/tui/widgets.rs
- **Commit:** 0fff93f

**2. [Rule 2 - Missing variant] Added StageTransition to AgentEvent**
- **Found during:** Task 1
- **Issue:** Plan's interface spec listed `StageTransition` as part of the existing enum, but the actual file did not contain it. Downstream plans reference it.
- **Fix:** Added `StageTransition { from: String, to: String }` to the enum alongside the 4 plan-specified variants.
- **Files modified:** src/io/agent_io.rs
- **Commit:** c9a6e53

## Verification Results

- `cargo build` — exit 0, no warnings from new code
- `cargo test` — 205 tests passed (188 unit + 17 integration), 0 failed

## Known Stubs

None — this plan establishes the data contract layer only. No rendering or wiring stubs were created. Pure functions are fully implemented and tested.

## Self-Check: PASSED
