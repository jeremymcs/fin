# Phase 3: Auto-Run Panel - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.

**Date:** 2026-04-01
**Phase:** 3 — Auto-Run Panel

---

## Areas Selected

User selected all 4 gray areas: Panel layout & row count, Git commit source, Context % emit point, Auto mode gating.

---

## Panel Layout & Row Count

**Q: How should the panel grow to accommodate new rows?**
Options: Grow taller (dynamic height), Compact overlay existing rows, Separate auto-run bar
**Selected:** Grow the panel taller (dynamic height)
Preview confirmed: 7-8 inner rows including existing pipeline + progress bar rows.

**Q: Dynamic or fixed height?**
Options: Dynamic height (active when is_auto), Fixed tall always
**Selected:** Dynamic height — AppLayout computes taller Rect when workflow_state.is_auto=true. Output area shrinks by ~5 rows during auto mode.

---

## Git Commit Source

**Q: What should 'last commit' show?**
Options: Latest repo commit (git log -1), Fin-only commits this run, Don't show commit hash
**Selected:** Latest repo commit — `git log -1 --format='%h %s'`
Preview: `abc1234 feat: scaffold section-03 task files`

**Q: How should git commit info be fetched (non-blocking)?**
Options: On WorkflowUnitEnd, Periodic background poll, On StageTransition
**Selected:** On WorkflowUnitEnd — async fetch after each unit completes, stored in WorkflowState.

---

## Context % Emit Point

**Q: When should AgentEvent::ContextUsage { pct: u8 } be emitted?**
Options: Per turn after TurnEnd, Per WorkflowUnitEnd, On every TextDelta
**Selected:** Per turn, after TurnEnd — using cumulative Usage stats already tracked.

**Q: What's the denominator for context %?**
Options: Per-model lookup table, Track from API response, Fixed 200k denominator
**Selected:** Per-model lookup table — model id → context window size, default 200k for unknowns. No API calls.

---

## Auto Mode Gating

**Q: When does the expanded panel show?**
Options: Auto mode only once workflow.active (recommended), Whenever workflow active, Always show expanded
**Selected:** Auto mode only, once workflow.active=true — LoopMode::Auto + workflow_state.is_auto=true.

**Q: How does WorkflowState know it's in auto mode?**
Options: New AgentEvent::AutoModeStart/End, Pass through WorkflowUnitStart, State tracked in app.rs only
**Selected:** New AgentEvent::AutoModeStart/End — emitted by app.rs when spawning/completing the auto loop.

---

*Discussion log generated: 2026-04-01*
