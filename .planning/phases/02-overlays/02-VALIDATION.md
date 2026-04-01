---
phase: 02
slug: overlays
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-01
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | `Cargo.toml` (workspace root) |
| **Quick run command** | `cargo test --lib 2>&1 \| tail -20` |
| **Full suite command** | `cargo test 2>&1 \| tail -40` |
| **Build check command** | `cargo build --features tui 2>&1 \| head -30` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test 2>&1 | tail -40`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| StageTransition variant | W0 | 0 | TOAST-01 | unit | `cargo test tui::app::tests::toast_stage_transition` | ❌ W0 | ⬜ pending |
| help_key_guard | W0 | 0 | HELP-03 | unit | `cargo test tui::app::tests::help_key_guard` | ❌ W0 | ⬜ pending |
| toast_workflow_terminal | W0 | 0 | TOAST-02 | unit | `cargo test tui::app::tests::toast_workflow_terminal` | ❌ W0 | ⬜ pending |
| toast_tool_error | W0 | 0 | TOAST-03 | unit | `cargo test tui::app::tests::toast_tool_error` | ❌ W0 | ⬜ pending |
| toast_ttl_expiry | W0 | 0 | TOAST-04 | unit | `cargo test tui::app::tests::toast_ttl_expiry` | ❌ W0 | ⬜ pending |
| toast_no_fire_routine | W0 | 0 | TOAST-05 | unit | `cargo test tui::app::tests::toast_no_fire_routine` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/tui/app.rs` — `#[cfg(test)] mod tests` block with stubs for all 6 unit tests above
- [ ] Tests must compile (can `todo!()` initially) before Wave 1 execution begins

*All Wave 0 test stubs go in `src/tui/app.rs` — no new test files needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `?` opens full-screen overlay when input empty | HELP-01 | ratatui has no headless render assertion API | Run `cargo run --features tui`, type nothing, press `?`, verify overlay renders |
| Any key dismisses overlay | HELP-02 | Visual TUI interaction | With overlay open, press any key, verify overlay closes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
