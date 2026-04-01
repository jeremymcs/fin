# Phase 1: Foundation - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 delivers visual theme consistency and polished output rendering across all TUI widgets. Scope: named Palette constant in `widgets.rs`, `AppLayout` struct replacing `chunks[N]` indexing in `app.rs`, inline markdown rendering (**bold**, *italic*, `code`) in assistant output, and token/cost display improvements. No new layout panels, no new event types, no behavior changes — widget rendering only.

</domain>

<decisions>
## Implementation Decisions

### Color Palette

- **D-01:** Amber accent — `Color::Yellow` becomes the primary accent color (labels, borders, active indicators, splash title). This replaces the current cyan-as-accent role.
- **D-02:** `Color::Cyan` becomes the tool-call highlight color (replaces Yellow dim for `LineKind::Tool`). This is a semantic swap: Yellow → accent, Cyan → tools.
- **D-03:** All color references must go through a single named `Palette` const struct in `widgets.rs`. No inline `Color::` literals scattered across render functions after Phase 1.
- **D-04:** ANSI named colors only (`Color::Yellow`, `Color::Cyan`, `Color::White`, `Color::DarkGray`, `Color::Green`, `Color::Red`). No `Color::Rgb` or `Color::Indexed` — terminal compatibility required.

### AppLayout Struct

- **D-05:** Extract a named `AppLayout` struct (or equivalent named bindings) that replaces all `chunks[N]` index arithmetic in `app.rs`. This is a Phase 1 prerequisite for Phases 3 and 4 — both depend on it being safe before they add new layout variants.
- **D-06:** The struct must accommodate the two existing layout variants (with workflow panel active, without) without raw index offsets.

### Inline Markdown Rendering

- **D-07:** Use `pulldown-cmark` 0.12.2 (already in `Cargo.toml`, currently unused) for span parsing. No new crates.
- **D-08:** Parser is gated behind an `is_final: bool` flag on `OutputLine`. Streaming (in-progress) lines render as plain text to prevent per-frame flicker. Parser only runs on finalized lines.
- **D-09:** Three patterns in scope: `**bold**` → `Modifier::BOLD`, `*italic*` → `Modifier::ITALIC`, `` `code` `` → Claude's discretion (prefer `Modifier::REVERSED` for helix-style conventional code block feel; fall back to `Modifier::BOLD | Modifier::DIM` if terminal theme issues are found during implementation).
- **D-10:** Only `LineKind::Assistant` lines are markdown-parsed. Other kinds (User, Tool, System, Error, Thinking) render as plain text.

### Token/Cost Display

- **D-11:** Status bar format stays as segmented pipes — clean up labels and formatting but preserve the structure: `{model} | {state}{scroll}{workflow} | in:{n} out:{n} | ${cost:.4}`. Formatting improvement: abbreviate large numbers (e.g., `1.2k` instead of `1243`).
- **D-12:** Per-message cost annotation: append a new `LineKind::System` `OutputLine` after each completed assistant response (on `AgentEvent::TurnComplete` or equivalent). Format: `  ↳ {in} in / {out} out  ${cost:.4}` rendered dim.
- **D-13:** Per-message annotation is not emitted during streaming — only after the turn completes.

### Claude's Discretion

- Inline code (`code`) visual modifier: REVERSED preferred (helix convention, immediately recognizable), BOLD+DIM acceptable fallback — choose based on visual testing during implementation.
- Number abbreviation threshold for token counts: suggest ≥1000 → `1.2k` format, but exact breakpoint is implementation detail.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core planning files
- `.planning/REQUIREMENTS.md` — Phase 1 requirements: THEME-01, THEME-02, MD-01–MD-04, TOK-01, TOK-02
- `.planning/ROADMAP.md` — Phase 1 success criteria (5 criteria)
- `.planning/research/ARCHITECTURE.md` — Integration points for all Phase 1 features
- `.planning/research/PITFALLS.md` — Critical pitfalls: chunks[N] fragility, streaming markdown flicker, scroll offset

### TUI source files to read before planning
- `src/tui/widgets.rs` — All current render functions, LineKind, WorkflowState, existing Color:: usage
- `src/tui/app.rs` — Main loop, layout structure (lines 218–343), AgentEvent handlers, cursor placement

### Cargo dependency
- `Cargo.toml` line 47 — `pulldown-cmark = "0.12"` (already present, no new crates needed)

No external specs — requirements fully captured in decisions above.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `render_output()` in `widgets.rs:149` — current per-line rendering loop; markdown parsing inserts here with `is_final` gate
- `OutputLine` / `LineKind` in `widgets.rs:472–527` — add `is_final: bool` field to `OutputLine`
- `render_status_bar()` in `widgets.rs:262` — token/cost format lives here
- `AgentEvent::TurnComplete` (or nearest equivalent) in agent IO — trigger for per-message annotation

### Established Patterns
- All styling uses `Style::default().fg(Color::X)` — Palette const wraps these; render functions call `Palette::ACCENT` etc.
- Two-variant layout in `app.rs:222–243` — `chunks[N]` indexing is the fragility; `AppLayout` struct fixes this
- Model picker overlay pattern (app.rs ~291) — precedent for overlay rendering; REVERSED modifier may reuse this pattern

### Integration Points
- `AppLayout` struct: `terminal.draw()` closure in `app.rs:220` — refactor layout assignments here
- Markdown renderer: `render_output()` in `widgets.rs:149` — replaces per-`LineKind::Assistant` match arm logic
- Per-message annotation: agent event handler in `app.rs` around line 349+ — append `OutputLine` after `TurnComplete`

</code_context>

<specifics>
## Specific Ideas

- User confirmed they want the amber/yellow accent feel — deliberate differentiation from standard cyan terminal tools
- The `↳` dim annotation after each response mirrors the opencode pattern the user is familiar with

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 1 scope.

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-04-01*
