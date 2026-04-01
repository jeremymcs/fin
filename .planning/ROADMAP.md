# Roadmap: Fin v1.1 TUI Enhancement

## Overview

Four phases deliver the full v1.1 TUI Enhancement milestone in risk-escalating order. Phase 1 establishes the widget foundation — palette constants, AppLayout struct, inline markdown rendering, and token formatting — with zero layout risk. Phase 2 layers keyboard-driven overlays (help and toasts) on top of that foundation using the proven model-picker pattern. Phase 3 expands the auto-run workflow panel, the most cross-cutting feature, touching agent_io, git, auto_loop, and app state. Phase 4 closes with the side panel, the most structurally invasive change, which depends on Phase 3's context_pct field and Phase 1's AppLayout struct to be safe.

## Milestones

- ✅ **v1.0 Initial Release** — Shipped (pre-milestone tracking)
- 🚧 **v1.1 TUI Enhancement** — Phases 1–4 (in progress)

## Phases

- [ ] **Phase 1: Foundation** — Palette constants, AppLayout struct, inline markdown rendering, token/cost formatting
- [ ] **Phase 2: Overlays** — Help overlay (? key) and toast notification system
- [ ] **Phase 3: Auto-Run Panel** — Expanded workflow panel with live blueprint/model/stage/git/context display
- [ ] **Phase 4: Side Panel** — Toggle-able Ctrl+P side info panel with layout split

## Phase Details

### Phase 1: Foundation
**Goal**: The TUI has a consistent visual theme and polished output rendering across all widgets
**Depends on**: Nothing (first phase)
**Requirements**: THEME-01, THEME-02, MD-01, MD-02, MD-03, MD-04, TOK-01, TOK-02
**Success Criteria** (what must be TRUE):
  1. Assistant output displays **bold**, *italic*, and `inline code` with distinct visual styling — raw asterisks and backticks are never visible to the user
  2. A dim cost annotation line appears after each completed assistant response (not on in-progress streaming lines)
  3. The status bar shows formatted token counts and cost (e.g., "1.2k / $0.004") rather than raw integers
  4. All TUI widgets (output, status bar, workflow panel, input, splash) use colors drawn from a single named Palette constant — changing one color constant updates the whole UI
  5. A named AppLayout struct replaces all `chunks[N]` index arithmetic in app.rs, making future layout changes safe
**Plans**: TBD
**UI hint**: yes

### Phase 2: Overlays
**Goal**: Users can discover keybindings and receive high-signal event feedback through ephemeral notifications
**Depends on**: Phase 1
**Requirements**: HELP-01, HELP-02, HELP-03, TOAST-01, TOAST-02, TOAST-03, TOAST-04, TOAST-05
**Success Criteria** (what must be TRUE):
  1. Pressing `?` with an empty input field opens a full-screen keybindings and slash command reference; pressing any key dismisses it
  2. `?` typed into a non-empty input field is inserted as a literal character — the overlay does not open mid-sentence
  3. An ephemeral toast appears at the top-right of the output area when a workflow stage transitions, when the auto-loop completes or is blocked, or when a tool call produces an error
  4. Toast notifications auto-dismiss after a fixed real-time duration (Instant-based) and do not appear for routine individual tool calls
  5. No toast appears during a heavy streaming sequence where agent events are draining rapidly — timer is immune to event flood rate
**Plans**: TBD
**UI hint**: yes

### Phase 3: Auto-Run Panel
**Goal**: During autonomous execution, the workflow panel provides complete live context — blueprint, model, stage, last commit, context usage, and cancel hint
**Depends on**: Phase 2
**Requirements**: AUTO-01, AUTO-02, AUTO-03, AUTO-04, AUTO-05, AUTO-06
**Success Criteria** (what must be TRUE):
  1. During auto-loop execution the workflow panel shows the active blueprint ID/name and current model on one row
  2. During auto-loop execution the workflow panel shows the current stage name and section/task being executed
  3. During auto-loop execution the workflow panel shows the short git hash and summary of the last commit
  4. During auto-loop execution the workflow panel shows context window usage as a percentage (e.g., "ctx 34%")
  5. The workflow panel footer shows `esc pause | ? help` keybind hints during auto-loop; the existing stage pipeline (✓/●/○) and progress bar remain visible and unchanged
**Plans**: TBD
**UI hint**: yes

### Phase 4: Side Panel
**Goal**: Users can toggle a persistent side panel that shows model, cumulative tokens, cost, and workflow state at a glance
**Depends on**: Phase 3
**Requirements**: SIDE-01, SIDE-02, SIDE-03
**Success Criteria** (what must be TRUE):
  1. Pressing `Ctrl+P` (or `/panel`) toggles a right-side info panel showing current model, cumulative input/output tokens, cost, and workflow state
  2. When terminal width is below 100 columns the side panel is hidden automatically — the main output area occupies full width
  3. Toggling the side panel does not shift scroll position or corrupt cursor placement in the input field
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.1 | 0/TBD | Not started | - |
| 2. Overlays | v1.1 | 0/TBD | Not started | - |
| 3. Auto-Run Panel | v1.1 | 0/TBD | Not started | - |
| 4. Side Panel | v1.1 | 0/TBD | Not started | - |
