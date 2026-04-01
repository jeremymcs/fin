---
phase: 3
slug: auto-run-panel
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-01
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `cargo test` |
| **Config file** | `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && cargo build --features tui` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && cargo build --features tui`
- **Before `/gsd:verify-work`:** Full suite must be green + clean build
- **Max feedback latency:** ~15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 3-W0-01 | Wave 0 | 0 | AUTO-01 | unit | `cargo test test_workflow_state_auto_fields` | ❌ W0 | ⬜ pending |
| 3-W0-02 | Wave 0 | 0 | AUTO-04 | unit | `cargo test test_context_pct_calculation` | ❌ W0 | ⬜ pending |
| 3-W0-03 | Wave 0 | 0 | AUTO-04 | unit | `cargo test test_context_pct_clamped` | ❌ W0 | ⬜ pending |
| 3-W0-04 | Wave 0 | 0 | AUTO-03 | unit | `cargo test test_last_commit_parse` | ❌ W0 | ⬜ pending |
| 3-W0-05 | Wave 0 | 0 | AUTO-06 | unit | `cargo test test_pipeline_row_unchanged_in_auto` | ❌ W0 | ⬜ pending |
| 3-01-01 | P01 | 1 | AUTO-01 | unit | `cargo test test_workflow_state_auto_fields` | ✅ W0 | ⬜ pending |
| 3-01-02 | P01 | 1 | AUTO-02 | unit | `cargo test test_workflow_state_unit_start` | ❌ W0 | ⬜ pending |
| 3-02-01 | P02 | 2 | AUTO-03 | unit | `cargo test test_last_commit_parse` | ✅ W0 | ⬜ pending |
| 3-03-01 | P03 | 3 | AUTO-04 | unit | `cargo test test_context_pct_calculation` | ✅ W0 | ⬜ pending |
| 3-04-01 | P04 | 4 | AUTO-05 | unit | `cargo test test_render_footer_hints` | ❌ W0 | ⬜ pending |
| 3-04-02 | P04 | 4 | AUTO-06 | unit | `cargo test test_pipeline_row_unchanged_in_auto` | ✅ W0 | ⬜ pending |
| 3-04-03 | P04 | 4 | ALL | build | `cargo build --features tui` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/tui/widgets.rs` — `test_workflow_state_auto_fields`: verify `WorkflowState` default for new fields (`is_auto=false`, `model_display=""`, `last_commit_hash=""`, `last_commit_msg=""`, `context_pct=0`)
- [ ] `src/tui/widgets.rs` — `test_context_pct_calculation`: pure function unit test — 50_000u64 / 200_000u64 * 100 = 25u8
- [ ] `src/tui/widgets.rs` — `test_context_pct_clamped`: verify result does not exceed 100 when input_tokens > context_window
- [ ] `src/workflow/git.rs` — `test_last_commit_parse`: unit test for output line parsing logic in a pure function (separate from async git call)
- [ ] `src/tui/widgets.rs` — `test_pipeline_row_unchanged_in_auto`: equality test confirming pipeline spans are identical regardless of `is_auto`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Auto panel expands on AutoModeStart | AUTO-01 | TUI render — no headless harness | Run `fin --auto`, confirm panel grows to 7 rows |
| Auto panel collapses on AutoModeEnd | AUTO-01 | TUI render | Complete/cancel run, confirm panel returns to 2 rows |
| Git commit row updates after unit end | AUTO-03 | Async timing | Watch commit row refresh after each workflow unit |
| Context bar fills as tokens grow | AUTO-04 | Real agent call needed | Run multi-turn session, verify bar fills progressively |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
