---
phase: 1
slug: foundation
status: ready
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-01
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | None — standard Cargo test runner |
| **Quick run command** | `cargo test --lib 2>&1` |
| **Full suite command** | `cargo test 2>&1` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1`
- **After every plan wave:** Run `cargo test 2>&1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01-01 | 1 | MD-01, MD-02, MD-03, MD-04, TOK-02, THEME-02 | unit (TDD) | `cargo test --lib 2>&1` | ❌ W0 | ⬜ pending |
| 1-02-01 | 01-02 | 1 | THEME-01, THEME-02 | structural | `cargo build 2>&1 && grep -c "AppLayout" src/tui/app.rs` | ✅ | ⬜ pending |
| 1-03-01 | 01-03 | 2 | THEME-01, THEME-02, MD-01, MD-02, MD-03 | structural | `cargo build 2>&1 && cargo test --lib 2>&1` | ✅ | ⬜ pending |
| 1-03-02 | 01-03 | 2 | MD-01, MD-02, MD-03, MD-04, TOK-02 | unit+integration | `cargo test --lib 2>&1` | ✅ | ⬜ pending |
| 1-03-03 | 01-03 | 2 | MD-04, TOK-01, TOK-02 | unit+manual | `cargo build 2>&1 && cargo test --lib 2>&1` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/tui/widgets.rs` or `src/tui/widgets_test.rs` — add `#[cfg(test)] mod tests` block covering:
  - `test_parse_inline_spans_bold()` — input: `"foo **bar** baz"`, assert span 1 has `Modifier::BOLD`
  - `test_parse_inline_spans_italic()` — input: `"foo *bar* baz"`, assert span 1 has `Modifier::ITALIC`
  - `test_parse_inline_spans_code()` — input: `` "foo `bar` baz" ``, assert span 1 has `Modifier::REVERSED`
  - `test_parse_inline_spans_no_final()` — `is_final: false` line renders as single raw span, no markdown parsing
  - `test_format_token_count_below_threshold()` — `format_token_count(999)` == `"999"`
  - `test_format_token_count_above_threshold()` — `format_token_count(1243)` == `"1.2k"`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| All widgets use Palette colors | THEME-01 | Visual rendering — no automated color output check | Launch `cargo run`, observe output/status/workflow panels use consistent amber accent and cyan tool colors |
| `↳` cost annotation after turn | TOK-01 | Integration — requires live API turn | Send a message, confirm dim `↳ N in / M out  $X.XXXX` line appears after response completes, not during streaming |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (TDD in plan 01-01 Task 1)
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready
