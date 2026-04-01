# Requirements: Fin TUI Enhancement

**Defined:** 2026-04-01
**Milestone:** v1.1
**Core Value:** A fast, self-contained AI coding agent that runs a full workflow autonomously from a single terminal command.

## v1.1 Requirements

### Visual Theme

- [x] **THEME-01**: User sees a consistent color palette across all TUI widgets (output, status bar, workflow panel, input, splash)
- [x] **THEME-02**: Named palette constants are defined in `widgets.rs` so future color changes require editing one place

### Markdown Rendering

- [x] **MD-01**: User sees **bold** text rendered with bold styling in assistant output (not raw `**asterisks**`)
- [x] **MD-02**: User sees *italic* text rendered with italic styling in assistant output (not raw `*asterisks*`)
- [x] **MD-03**: User sees `inline code` rendered with distinct styling in assistant output (not raw backticks)
- [x] **MD-04**: Markdown rendering does not flicker on partial/streaming lines (parser gated behind `is_final` flag)

### Token Display

- [x] **TOK-01**: User sees per-message cost annotation as a dim line after each completed assistant response
- [x] **TOK-02**: User sees a cleaner token/cost summary in the status bar (formatted counts, not raw numbers)

### Toast Notifications

- [ ] **TOAST-01**: User sees an ephemeral toast notification when a workflow stage transitions (e.g. "Build → Validate")
- [ ] **TOAST-02**: User sees an ephemeral toast notification when the auto-loop completes or is blocked
- [ ] **TOAST-03**: User sees an ephemeral toast notification when a tool call produces an error
- [ ] **TOAST-04**: Toast notifications auto-dismiss after a fixed duration (Instant-based, not frame-count-based)
- [ ] **TOAST-05**: Toast notifications do not appear for every individual tool call (only high-signal events)

### Help Overlay

- [x] **HELP-01**: User can press `?` to open a full-screen keybindings and slash command reference
- [x] **HELP-02**: Help overlay is dismissed by pressing any key
- [x] **HELP-03**: `?` key is only intercepted when the input field is empty (does not block typing `?` in prompts)

### Auto-Run Panel

- [x] **AUTO-01**: During auto-loop execution, the workflow panel shows the active blueprint ID and name alongside the current model
- [x] **AUTO-02**: During auto-loop execution, the workflow panel shows the current stage and section/task being executed
- [ ] **AUTO-03**: During auto-loop execution, the workflow panel shows the short git hash and message of the last commit
- [x] **AUTO-04**: During auto-loop execution, the workflow panel shows context window usage as a percentage
- [ ] **AUTO-05**: During auto-loop execution, the workflow panel footer shows `esc pause | ? help` keybind hints
- [x] **AUTO-06**: Existing stage pipeline (✓/●/○) and progress bar are preserved — auto-run rows are additive

### Side Info Panel

- [ ] **SIDE-01**: User can press `Ctrl+P` to toggle a right-side info panel showing model, cumulative tokens, cost, and workflow state
- [ ] **SIDE-02**: Side panel auto-hides when terminal width is below 100 columns
- [ ] **SIDE-03**: Toggling the side panel does not corrupt scroll state or cursor position

## Future Requirements

### Extended Markdown

- **MD-FUT-01**: Full code block syntax highlighting (deferred — syntect adds ~1-2MB binary size)

### Extended Toasts

- **TOAST-FUT-01**: Toast stacking (multiple concurrent toasts) — deferred until single-toast system is stable

### Multi-Session

- **MULTI-01**: Split view showing two sessions side-by-side — deferred, too complex for v1.1

## Out of Scope

| Feature | Reason |
|---------|--------|
| Mouse support | TUI is keyboard-driven; mouse capture breaks native text selection |
| GUI / Electron | Violates binary size and zero-dependency goals |
| Plugin marketplace | Agent extension system exists; marketplace is premature |
| Full markdown parser crate (tui-markdown, syntect) | Binary size constraint; 2 inline patterns need ~70 lines custom code |
| Per-tool-call toasts | Signal-to-noise ratio — only high-signal events should surface |
| Ctrl+P side panel below 100 columns | Layout math breaks at narrow widths; graceful degradation is auto-hide |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| THEME-01 | Phase 1: Foundation | Complete |
| THEME-02 | Phase 1: Foundation | Complete |
| MD-01 | Phase 1: Foundation | Complete |
| MD-02 | Phase 1: Foundation | Complete |
| MD-03 | Phase 1: Foundation | Complete |
| MD-04 | Phase 1: Foundation | Complete |
| TOK-01 | Phase 1: Foundation | Complete |
| TOK-02 | Phase 1: Foundation | Complete |
| TOAST-01 | Phase 2: Overlays | Pending |
| TOAST-02 | Phase 2: Overlays | Pending |
| TOAST-03 | Phase 2: Overlays | Pending |
| TOAST-04 | Phase 2: Overlays | Pending |
| TOAST-05 | Phase 2: Overlays | Pending |
| HELP-01 | Phase 2: Overlays | Complete |
| HELP-02 | Phase 2: Overlays | Complete |
| HELP-03 | Phase 2: Overlays | Complete |
| AUTO-01 | Phase 3: Auto-Run Panel | Complete |
| AUTO-02 | Phase 3: Auto-Run Panel | Complete |
| AUTO-03 | Phase 3: Auto-Run Panel | Pending |
| AUTO-04 | Phase 3: Auto-Run Panel | Complete |
| AUTO-05 | Phase 3: Auto-Run Panel | Pending |
| AUTO-06 | Phase 3: Auto-Run Panel | Complete |
| SIDE-01 | Phase 4: Side Panel | Pending |
| SIDE-02 | Phase 4: Side Panel | Pending |
| SIDE-03 | Phase 4: Side Panel | Pending |
