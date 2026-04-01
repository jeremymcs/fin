// Fin — TUI Enhancement v1.1 Research Summary
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Project Research Summary

**Project:** Fin — TUI Enhancement v1.1
**Domain:** Overlay, streaming-text, and layout-extension features for an existing ratatui/crossterm Rust TUI
**Researched:** 2026-04-01
**Confidence:** HIGH

---

## Executive Summary

Fin v1.1 adds seven distinct TUI improvements to an already-working Rust terminal UI: an expanded auto-run workflow panel, ephemeral toast notifications, inline markdown bold/italic rendering, a keybindings help overlay, per-message token/cost display, a visual theme consistency pass, and a toggle-able side info panel. The critical finding across all research is that every one of these features can be built with zero new crate dependencies — ratatui 0.29, crossterm 0.28, pulldown-cmark 0.12 (already present but unused), and std primitives cover all seven. The binary size target of 3.7MB stripped is unaffected.

The recommended approach is to implement in four ordered phases. Phase 1 covers isolated, logic-free widget changes (theme, inline markdown, token formatting). Phase 2 adds keyboard-driven overlays (help overlay, toast system) using the established model-picker pattern as precedent. Phase 3 expands the workflow panel — the most cross-cutting single feature, touching agent_io.rs, git.rs, auto_loop.rs, widgets.rs, and app.rs. Phase 4 adds the side panel, the most structurally invasive change due to its horizontal layout split, and is done last to avoid restructuring the draw closure before other features are stable.

The two dominant risks are layout chunk-index brittleness and scroll-state invalidation on panel toggle. Both stem from the current hardcoded `chunks[N]` index arithmetic in app.rs. The prevention strategy — extracting a named `AppLayout` struct after every `Layout::split()` call — must be applied proactively in Phase 1 before any panel changes land, not reactively after bugs appear. A secondary risk is mid-stream markdown parsing flicker; this is avoided entirely by gating span parsing on `OutputLine::is_final`, a one-field addition with no downstream cost.

---

## Key Findings

### Recommended Stack

**No Cargo.toml changes required for v1.1.** All seven features are buildable with already-present dependencies. The decision to avoid new crates is not a constraint imposed from outside — it is the correct engineering choice. ratkit and tui-markdown were evaluated and rejected on specifics: ratkit's toast widget adds ~30K SLoC of transitive dependencies for functionality achievable in 40 lines; tui-markdown requires pulling pulldown-cmark 0.13 (a version bump from the 0.12.2 already present) plus itertools and tracing, for two inline patterns that take ~70 lines of hand-written Rust.

The one open question is whether `Color::Rgb` values should be introduced during the theme pass. The recommendation is to stay with named ANSI colors (`Color::Cyan`, `Color::DarkGray`, etc.) and avoid `Color::Rgb` unless `COLORTERM=truecolor` is detected at startup. This avoids silent color substitution on 256-color terminals.

**Core technologies already in use (no changes):**
- `ratatui 0.29.0` — all rendering; `Clear` widget, `Layout`, `Span::styled()`, built-in palette constants — covers all overlay and panel work
- `crossterm 0.28.1` — terminal event loop; no changes needed for v1.1 key bindings
- `pulldown-cmark 0.12.2` — already in Cargo.toml, already paying binary cost, zero-cost to use for inline span parsing; provides `Event::Start(Tag::Strong/Emphasis)` for bold/italic extraction
- `std::time::Instant` — toast TTL tracking; immune to agent event flood rate variation
- `git2 0.19.0` — already present; `repo.head()?.peel_to_commit()?.summary()` gives the last commit short hash needed for the auto-run panel

### Expected Features

**Must have (table stakes) — 5 features:**
- Auto-run live panel (expanded workflow widget) — users of lazygit, aider, bottom all expect a continuously-updating status widget during long-running operations
- Toast / ephemeral notification system — universal in modern TUIs (helix, lazygit, bottom); critical for surfacing errors that don't interrupt scrollable output
- Inline markdown bold/italic rendering — every AI chat TUI that renders assistant output does this; raw asterisks feel broken
- Keybindings help overlay (`?` key) — users reaching for `?` or `F1` is universal across lazygit, gitui, helix, bottom; missing = poor discoverability
- Per-message token/cost display — opencode, conduit, tokscale all do this; developers using AI coding agents track spend actively

**Should have (differentiators) — 2 features:**
- Toggle-able side info panel (Ctrl+P) — persistent glanceable model/token/workflow state is not standard in AI coding TUIs; genuinely novel for the autonomous agent use case
- Visual theme consistency pass — not user-visible as a feature, but color inconsistencies are felt; establishes a `Palette` const struct for all future widget work

**Defer (v2+):**
- Full markdown table rendering — requires multi-pass column-width calculation, breaks at narrow terminals; current pass-through is acceptable
- Mouse support — `EnableMouseCapture` breaks native terminal text selection; keyboard-only is the correct default for a developer tool
- Animated spinners — the real-count progress bar is sufficient liveness feedback; animations add render complexity with no information gain
- Full markdown crate integration (tui-markdown / syntect) — binary size impact unacceptable for two inline patterns

### Architecture Approach

All seven features fit within two existing files (`src/tui/app.rs`, `src/tui/widgets.rs`) and three supporting files (`src/io/agent_io.rs`, `src/workflow/auto_loop.rs`, `src/workflow/git.rs`). No new files are needed. The established overlay pattern from the model picker (`Clear` + floating `Rect` + keyboard guard with `continue`) is the canonical approach for all new overlays (toasts, help). The `WorkflowState` struct lives in `widgets.rs` and is mutated from `app.rs` — this coupling is correct and must be preserved; new fields for the auto-run panel expansion follow the same ownership pattern. The draw closure render order matters for z-layering: output/splash → workflow panel → status bar → input → side panel (horizontal split of output area) → toasts → help overlay → model picker.

**Major touch points per feature:**

| Feature | Primary File | Secondary Files |
|---------|-------------|-----------------|
| Theme consistency | `widgets.rs` only | — |
| Inline markdown | `widgets.rs` only | — |
| Token/cost formatting | `widgets.rs` only | — |
| Help overlay | `widgets.rs` (render), `app.rs` (state + key) | — |
| Toast system | `widgets.rs` (render + struct), `app.rs` (state + event hooks) | — |
| Auto-run panel | `widgets.rs` (struct + render), `app.rs` (handler + layout) | `agent_io.rs`, `auto_loop.rs`, `git.rs` |
| Side panel | `widgets.rs` (render), `app.rs` (state + layout split) | — |

### Critical Pitfalls

1. **Chunk index breakage when layouts change** — the current `chunks[3]` / `chunks[2]` index arithmetic in `app.rs` is brittle the moment a third layout variant appears. Prevention: extract a named `struct AppLayout { output: Rect, workflow: Option<Rect>, side: Option<Rect>, status: Rect, input: Rect }` before any panel work begins. Apply in Phase 1 even before the first panel change lands.

2. **Scroll offset invalidation on panel toggle** — when `workflow_state.active` or `side_panel_active` changes, the effective output viewport height changes but `scroll` is not recomputed. Result: blank output area. Prevention: call `auto_scroll()` unconditionally, or re-clamp `scroll` to `max_scroll()`, whenever any panel visibility toggles.

3. **Mid-stream markdown parsing flicker** — `render_output()` is called every frame on partially-buffered lines. A `**foo` without its closing `**` causes one frame of unstyled text followed by styled text when the closing delimiter arrives. Prevention: add `is_final: bool` to `OutputLine`; parse markdown spans only on finalized lines. Raw render for in-progress lines.

4. **Toast timer starvation under agent event flood** — placing toast expiry inside the `AgentEvent` drain causes toasts to fire many times per draw cycle and disappear nearly instantly during heavy streaming. Prevention: always use `Instant::now()` for expiry, never a frame counter. The `while let Ok(evt) = agent_event_rx.try_recv()` drain is unbounded; `Instant` is immune to this.

5. **`?` key fires in non-empty input context** — `?` is a valid chat character. Binding it globally opens the help overlay mid-sentence. Prevention: use a Rust match guard `(KeyCode::Char('?'), _) if input_text.is_empty() && !model_picker_active =>` to open the overlay only when input is empty. Mirror the model-picker guard pattern exactly.

**Additional moderate pitfalls to watch:**
- `render_workflow_panel()` expanded to 5+ rows will produce zero-height `Rect`s on short terminals without a height guard — add `if area.height < REQUIRED_ROWS { render_compact_fallback(); return; }` at panel entry
- `Clear` must be rendered before overlay content, not after — clear last is a common copy-paste mistake that blanks the overlay
- Toasts anchored to the terminal bottom overlap the workflow panel — anchor to top-right of the output area instead
- `Ctrl+P` may be intercepted by tmux/readline before reaching the app — document the conflict and add `/panel` slash command as a fallback

---

## Implications for Roadmap

### Phase 1: Foundation — Widget-Only Changes

**Rationale:** Theme consistency, inline markdown, and token display formatting are purely additive changes confined to `widgets.rs`. No state, no new events, no layout modification. Zero risk of introducing regressions in other features. This phase also establishes the `Palette` const struct and the `AppLayout` named struct that all subsequent phases depend on for safe chunk indexing.

**Delivers:** Visually polished baseline; consistent color semantics; bold/italic text in assistant output; readable token counts in status bar.

**Addresses:** Theme consistency (F6), Inline markdown (F3), Token/cost formatting option A (F5)

**Avoids:** Establishes `AppLayout` struct proactively (Pitfall 1 prevention); sets `is_final` field on `OutputLine` before markdown parser ships (Pitfall 4 prevention)

**Research flag:** Standard patterns. No deeper research needed.

---

### Phase 2: Keyboard-Driven Overlays

**Rationale:** Help overlay and toast system both use the established model-picker overlay pattern (precedent already in production). Each adds a boolean state flag, a keyboard handler, and a new `render_*()` function. Neither changes the layout. They are independently testable and have no shared state. Phase 1 must complete first so the `Palette` const struct is available for overlay styling.

**Delivers:** Discoverable keybindings (critical for new users); ephemeral event feedback for stage transitions and errors.

**Addresses:** Help overlay (F4), Toast system (F2)

**Avoids:** Pitfall 3 (Instant-based TTL, not frame counter); Pitfall 5 (match guard on `input_text.is_empty()`); Pitfall 12 (same guard); Pitfall 13 (Clear before content); Pitfall 14 (toasts anchored top-right of output area)

**Research flag:** Standard patterns. Model picker is the established precedent. No deeper research needed.

---

### Phase 3: Auto-Run Panel Expansion

**Rationale:** This is the most cross-cutting feature — it touches `agent_io.rs`, `git.rs`, `auto_loop.rs`, `widgets.rs`, and `app.rs`. It requires a new `AgentEvent::WorkflowGitStatus` variant, a new `async fn last_commit()` in `git.rs`, a new `AgentEvent::ContextUsage` variant for memory %, and expansion of `WorkflowState` with five new fields. Doing this after Phases 1 and 2 means the `AppLayout` struct and `Palette` constants are already in place, and no overlay code from Phase 2 is at risk from the layout height changes this phase introduces.

**Delivers:** Live blueprint/model/action/git/memory display during autonomous runs; Esc cancel hint functional.

**Addresses:** Auto-run live panel (F1)

**Avoids:** Pitfall 2 (scroll re-clamp after panel height changes); Pitfall 7 (height guard for expanded panel rows); Pitfall 10 (scroll max calculation updated to match new panel height constant)

**Research flag:** Needs verification during planning. The `context_pct` field requires a decision on whether to emit it as a new `AgentEvent::ContextUsage` variant or read synchronously from agent state. The clean path (new event variant) is well-understood but requires coordination across three files. Confirm the event variant approach does not conflict with any in-progress auto_loop.rs changes before building.

---

### Phase 4: Toggle-able Side Panel

**Rationale:** The side panel requires restructuring the main draw layout from a single vertical split into a two-level layout (vertical outer, horizontal inner for the output row). This is the highest-risk structural change in v1.1. It must come last so that Phases 1-3 are fully stable before the layout is modified. It depends on Phase 3's `context_pct` field being available in `WorkflowState`. Token detail display in the side panel is informed by Phase 1's token formatting work.

**Delivers:** Glanceable model/token/workflow state at a keypress; per-message token detail in a dedicated panel; genuinely differentiating feature for autonomous agent workflows.

**Addresses:** Toggle-able side panel (F7); per-message token detail view (F5 option B)

**Avoids:** Pitfall 1 (AppLayout struct from Phase 1 makes the layout split safe); Pitfall 2 (scroll re-clamp on Ctrl+P toggle); Pitfall 8 (Ctrl+P conflict — add `/panel` slash command fallback and document in help overlay built in Phase 2)

**Research flag:** Terminal width handling needs validation during planning. A 30-column fixed side panel at 80-column terminal leaves 50 columns for output — usable but tight. `Constraint::Percentage(25)` with a `Constraint::Min(28)` floor may be preferable to a fixed length. Verify truncation behavior with both `Constraint::Length(30)` and percentage-based approaches before committing to one.

---

### Phase Ordering Rationale

- **Isolation-first:** Phases 1 and 2 are entirely self-contained. They deliver user-visible value with no layout risk. Shipping these first provides a stable foundation and allows integration feedback before the more invasive changes.
- **Infrastructure before consumers:** The `AppLayout` named struct (Phase 1) and `context_pct` field (Phase 3) are shared dependencies. Producing them before their consumers (side panel, scroll recalc) avoids retrofitting.
- **Risk escalation:** Each phase is structurally more invasive than the previous. Phase 1 touches only render functions. Phase 2 adds state and event hooks. Phase 3 adds new event variants and cross-file plumbing. Phase 4 modifies the draw layout itself.
- **Shared infrastructure:** Memory % counter and git short hash are needed by both the auto-run panel (Phase 3) and side panel (Phase 4). Building them once in Phase 3 and reusing in Phase 4 avoids two passes at the same plumbing — consistent with the FEATURES.md MVP recommendation.

### Research Flags

**Needs `/gsd:research-phase` during planning:**
- Phase 3 (Auto-run panel): Confirm `ContextUsage` event variant approach vs. synchronous read from agent state. Verify `auto_loop.rs` `finalize()` is the correct emit point for `WorkflowGitStatus`.
- Phase 4 (Side panel): Validate layout constraint choice (`Constraint::Length(30)` vs `Constraint::Percentage(25)`) against 80-column and 120-column terminal widths. Confirm Ctrl+P is reachable in common macOS terminal environments.

**Standard patterns — skip research:**
- Phase 1 (Foundation): Pure widget changes, well-understood ratatui patterns.
- Phase 2 (Overlays): Model picker precedent is in production code. The pattern is proven.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate decisions verified against official docs and Cargo.toml. Zero new crates confirmed. The one medium-confidence item (ratkit binary size estimate) does not affect the decision since ratkit was rejected. |
| Features | HIGH | Grounded in direct comparison with lazygit, helix, bottom, opencode, aider, conduit. Anti-features are explicitly justified. Wave order matches ARCHITECTURE.md build order independently. |
| Architecture | HIGH | Based on direct source analysis of `app.rs` (2153 lines), `widgets.rs` (527 lines), `agent_io.rs`, `auto_loop.rs`, `tui_io.rs`. All touch points visible. No speculation about file contents. |
| Pitfalls | HIGH (code-grounded) / MEDIUM (ecosystem patterns) | Critical pitfalls 1-5 are directly traceable to specific lines in `app.rs` and `widgets.rs`. Moderate pitfalls 6-10 are grounded in ratatui issue tracker and crossterm behavior. Minor pitfalls are logic-level observations. |

**Overall confidence:** HIGH

### Gaps to Address

- **`context_pct` delivery mechanism:** Research recommends a new `AgentEvent::ContextUsage { pct: u8 }` variant. The exact point in `run_tui_agent` where cumulative usage is available and the correct emit location must be confirmed during Phase 3 planning. If the agent state is not accessible from the async task that drives the TUI loop, an alternative approach (periodic sampling or computing from existing `total_in` + model context window size) must be chosen.

- **Ctrl+P terminal compatibility:** The PITFALLS research flags Ctrl+P as potentially intercepted by readline and some multiplexers. This needs a real validation step — run Fin in iTerm2, macOS Terminal.app, and a tmux session and confirm the keypress reaches the application. If it does not, the fallback `/panel` slash command becomes the primary binding and Ctrl+P becomes secondary.

- **`is_final` field on `OutputLine`:** The pitfalls research identifies this as necessary for correct markdown parsing. The ARCHITECTURE and FEATURES research do not address where in the streaming pipeline `is_final` gets set to `true`. This is straightforward (`TextDelta` handler sets `is_final = false` on lines being built; the `\n` detection or `AgentEnd` sets it `true`) but must be confirmed as part of Phase 1 implementation before the markdown parser ships.

- **80-column side panel layout:** The exact constraint for the side panel (fixed vs. percentage) is unresolved. Research notes both approaches have tradeoffs. This gap should be resolved with a quick prototype in Phase 4 planning before the feature is built.

---

## Sources

### Primary (HIGH confidence)

- `/Users/jeremymcspadden/Github/fin/src/tui/app.rs` — live source analysis, state variables, event loop, overlay pattern
- `/Users/jeremymcspadden/Github/fin/src/tui/widgets.rs` — live source analysis, render functions, WorkflowState, LineKind
- `/Users/jeremymcspadden/Github/fin/src/io/agent_io.rs` — AgentEvent variants, Usage struct
- `/Users/jeremymcspadden/Github/fin/src/workflow/auto_loop.rs` — finalize() hook point for WorkflowGitStatus
- [ratatui Popup Example](https://ratatui.rs/examples/apps/popup/) — Clear + overlay pattern
- [ratatui::style::palette docs](https://docs.rs/ratatui/latest/ratatui/style/palette/index.html) — built-in palette constants
- [Rect docs.rs ratatui 0.29.0](https://docs.rs/ratatui/0.29.0/ratatui/layout/struct.Rect.html) — inner(), offset() helpers
- [tui-markdown Cargo.toml (GitHub raw)](https://raw.githubusercontent.com/joshka/tui-markdown/main/tui-markdown/Cargo.toml) — dependency verification

### Secondary (MEDIUM confidence)

- [ratatui issue tracker](https://github.com/ratatui/ratatui) — scroll viewport assumptions, unicode width edge cases
- [ratkit on lib.rs](https://lib.rs/crates/ratkit) — dependency weight estimate (not a measured binary)
- [ratatui-toolkit ToastManager](https://lib.rs/crates/ratatui-toolkit) — toast duration defaults (3s info, 5s warning, persistent error)
- [lazygit status panel / help bar](https://github.com/jesseduffield/lazygit/discussions/1606) — overlay dismiss behavior reference
- [bottom (btm)](https://github.com/ClementTsang/bottom) — widget toggle patterns

### Tertiary (LOW confidence — needs validation)

- Ctrl+P readline/multiplexer conflict: documented as a known issue in terminal application development; specific behavior in crossterm raw mode on macOS needs hands-on validation
- Binary size estimate for v1.1 at 3.7MB: no new crates added and no profile settings changed, but not measured against v1.1 code yet

---

*Research completed: 2026-04-01*
*Ready for roadmap: yes*
