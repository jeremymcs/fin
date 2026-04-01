---
phase: 01-foundation
plan: 01
subsystem: ui
tags: [ratatui, pulldown-cmark, tui, widgets, markdown, color-palette]

# Dependency graph
requires: []
provides:
  - "Palette struct with 7 ANSI color constants in src/tui/widgets.rs"
  - "OutputLine.is_final: bool field (assistant=false, all others=true)"
  - "parse_inline_spans() function using pulldown-cmark for **bold**, *italic*, `code`"
  - "format_token_count() abbreviating >= 1000 to '1.2k' format"
  - "flush_span() private helper for pulldown-cmark event loop"
  - "9 unit tests for parse_inline_spans and format_token_count"
affects:
  - "01-02 (AppLayout) — depends on Palette constants and OutputLine.is_final"
  - "01-03 (integration wiring) — uses parse_inline_spans in render_output"

# Tech tracking
tech-stack:
  added: ["pulldown-cmark 0.12 (already in Cargo.toml, now actively imported)"]
  patterns:
    - "Palette const struct as single source of truth for all TUI colors (D-03)"
    - "is_final gate pattern: streaming lines render plain, finalized lines get markdown parsing (D-08)"
    - "pulldown-cmark Event iteration with Tag::Strong/Emphasis and Event::Code (D-07)"

key-files:
  created: []
  modified:
    - "src/tui/widgets.rs — Palette, OutputLine.is_final, parse_inline_spans, format_token_count, flush_span, unit tests"

key-decisions:
  - "ANSI named colors only in Palette (Color::Yellow, Color::Cyan, etc.) — no Color::Rgb per D-04"
  - "assistant() constructor defaults is_final=false; all other constructors default is_final=true per D-08"
  - "Inline code uses Modifier::REVERSED (helix convention) per D-09"

patterns-established:
  - "Palette::ACCENT (Yellow), Palette::TOOL (Cyan), Palette::TEXT (White), Palette::DIM (DarkGray), Palette::SUCCESS (Green), Palette::ERROR (Red), Palette::STATUS_BG (DarkGray)"
  - "parse_inline_spans() returns Vec<Span<'static>> — call only on is_final == true lines"
  - "format_token_count() threshold: count >= 1000 -> '{:.1}k' format"

requirements-completed: [THEME-02, MD-01, MD-02, MD-03, TOK-02]

# Metrics
duration: 5min
completed: 2026-04-01
---

# Phase 01 Plan 01: Foundation Types and Utilities Summary

**Palette const struct with 7 ANSI colors, OutputLine.is_final streaming gate, parse_inline_spans via pulldown-cmark, and format_token_count with 9 passing unit tests**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-04-01T18:04:00Z
- **Completed:** 2026-04-01T18:09:17Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 1

## Accomplishments

- Added `Palette` struct with 7 ANSI color constants — single source of truth for all TUI widget colors (D-03/D-04)
- Added `is_final: bool` field to `OutputLine`; `assistant()` defaults to false, all others to true (D-08)
- Implemented `parse_inline_spans()` using pulldown-cmark 0.12 — handles **bold**, *italic*, `code` (D-07/D-09)
- Implemented `format_token_count()` abbreviating >= 1000 to '1.2k' format (D-11)
- 9 unit tests all passing; cargo build and cargo test both clean (182 total tests pass)

## Task Commits

Each task was committed atomically (TDD pattern):

1. **Task 1 RED: Failing tests** - `4dcbe0b` (test)
2. **Task 1 GREEN: Implementation** - `2325211` (feat)

_Note: TDD task produced two commits — test (RED) then implementation (GREEN)_

## Files Created/Modified

- `src/tui/widgets.rs` — Added: Palette struct (7 constants), OutputLine.is_final field, flush_span(), parse_inline_spans(), format_token_count(), #[cfg(test)] mod tests with 9 tests

## Decisions Made

- ANSI named colors only in Palette — no Color::Rgb per D-04 (terminal compatibility)
- assistant() defaults is_final=false per D-08 (streaming lines render plain to prevent per-frame flicker)
- Inline code uses Modifier::REVERSED (helix convention) per D-09
- "Unused" warnings for Palette, is_final, parse_inline_spans, format_token_count are expected — Plan 02 wires them into render_output()

## Deviations from Plan

None - plan executed exactly as written. TDD RED/GREEN cycle followed. All acceptance criteria met.

## Issues Encountered

- `cargo test --lib` fails with "no library targets found" since fin is a binary crate — used `cargo test` instead (covers the same unit tests via the binary target). This is documented in the PLAN.md verify step and is a known crate structure difference.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Palette struct ready for Plan 02 (AppLayout) to reference via `Palette::ACCENT` etc.
- `OutputLine.is_final` field ready for Plan 03 to gate markdown parsing in `render_output()`
- `parse_inline_spans()` ready for Plan 03 integration into the Assistant line render path
- No blockers — all type contracts established, all tests pass

---
*Phase: 01-foundation*
*Completed: 2026-04-01*
