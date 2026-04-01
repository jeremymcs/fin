# Fin

## What This Is

Fin is a 3.7MB Rust AI coding agent — one command, walk away, come back to a built project with clean git history. It supports TUI, print, headless, HTTP API, and MCP modes, with zero runtime dependencies and instant startup. Built for developers who want agentic coding without cloud tooling overhead.

## Core Value

A fast, self-contained AI coding agent that runs a full workflow (Define → Build → Validate) autonomously from a single terminal command.

## Current Milestone: v1.1 TUI Enhancement

**Goal:** Enrich the Fin TUI with better live visibility during auto runs and polish across the rest of the interface.

**Target features:**
- Auto-run panel expansion (more context during auto mode: blueprint/model, current action, git commit, memory %)
- Toast notification system (ephemeral banners for tool events, errors, stage transitions)
- Inline markdown rendering (**bold**, *italic* in assistant output)
- Keybindings/slash command help overlay (? key)
- Token/cost display improvements
- Visual theme consistency pass
- Toggle-able side info panel (Ctrl+P)

## Requirements

### Validated

<!-- Shipped and confirmed valuable in prior work. -->

- ✓ Interactive TUI mode (ratatui-based, alt-screen, keyboard nav) — v1.0
- ✓ Print mode (`-p` flag, stdout streaming) — v1.0
- ✓ Headless mode (JSONL stdin/stdout) — v1.0
- ✓ HTTP API mode (`fin serve`, REST + SSE) — v1.0
- ✓ MCP mode (`fin mcp`, stdio JSON-RPC) — v1.0
- ✓ Multi-provider LLM support (Anthropic, OpenAI, Google, Vertex, Bedrock, Ollama) — v1.0
- ✓ Built-in tools: bash, read, write, edit, grep, glob, git — v1.0
- ✓ Extension tools: web search (Brave/Tavily), resolve_library (Context7) — v1.0
- ✓ Workflow engine: Define → Explore → Architect → Build → Validate → SealSection → Advance — v1.0
- ✓ Blueprint wizard (`/blueprint` command, one active at a time) — v1.0
- ✓ Agent delegation (fin-researcher, fin-planner, fin-builder, fin-reviewer) — v1.0
- ✓ Session persistence and resume (`--continue` flag) — v1.0
- ✓ Splash screen with model/provider/extensions status — v1.0
- ✓ Workflow progress panel (stage pipeline + progress bar) — v1.0
- ✓ Auto-loop mode (step and auto, with cancellation) — v1.0
- ✓ Model picker overlay (in-TUI model switching) — v1.0
- ✓ Tab completion for slash commands — v1.0
- ✓ Input history (Up/Down arrow recall) — v1.0

### Active

<!-- Current scope for v1.1. -->

- [ ] Auto-run panel shows blueprint id/name, current stage/action, last git commit, context memory %, keybind hints
- ✓ Toast notifications for tool events, errors, and stage transitions (ephemeral, auto-dismiss) — Validated in Phase 2: Overlays
- [ ] Auto-run panel shows blueprint id/name, current stage/action, last git commit, context memory %, keybind hints
- ✓ Assistant output renders **bold** and *italic* and `inline code` markdown (not raw asterisks) — Validated in Phase 1: Foundation
- ✓ `?` key shows keybindings and slash command help overlay — Validated in Phase 2: Overlays
- ✓ Token/cost display improvements (per-message cost annotation + abbreviated status bar counts) — Validated in Phase 1: Foundation
- ✓ Visual theme consistency pass (Palette constants, amber accent, no inline Color:: in render functions) — Validated in Phase 1: Foundation
- [ ] Toggle-able side info panel (Ctrl+P: model, tokens, workflow state)

### Out of Scope

- Mouse support — TUI is keyboard-driven by design; mouse breaks native text selection
- GUI / Electron app — binary size and zero-dependency goals would be violated
- Plugin marketplace — agent extension system exists; a marketplace is premature
- Multi-session split view — too complex for v1.1; assess after side panel lands

## Context

- **Codebase**: 86 Rust source files, ~21K lines, Rust 2024 edition, Rust 1.85+
- **TUI stack**: ratatui + crossterm, alt-screen mode, no mouse capture
- **Current TUI files**: `src/tui/app.rs` (2153 lines, main loop), `src/tui/widgets.rs` (527 lines), `src/tui/tui_io.rs`, `src/tui/mod.rs`
- **Workflow panel**: Already exists — `render_workflow_panel()` in widgets.rs shows stage pipeline (✓/●/○) + progress bar. The v1.1 work extends this for auto mode, it does not replace it.
- **LineKind system**: Output lines are typed (Assistant, User, Thinking, Tool, ToolResult, Error, System) — markdown rendering adds span-level parsing within Assistant lines
- **Agent events**: `AgentEvent` enum drives TUI updates via mpsc channel — toast system hooks into this stream
- **Feature flags**: `tui` (on), `http` (on), `mcp` (off), `browser` (off)

## Constraints

- **Tech stack**: Rust only — no JS, no native bindings, no new heavy deps
- **Binary size**: Stay near 3.7MB stripped — avoid pulling in large crates (e.g. full markdown parsers)
- **Terminal compat**: Must work in standard 80-col terminals; side panel must be toggleable
- **No breaking changes**: Existing workflow panel behavior preserved — auto-mode expansion is additive

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Keep existing stage pipeline panel, extend for auto mode | User confirmed they like the current design | — Pending |
| Implement bold/italic/code with span parsing via pulldown-cmark 0.12 | Binary size constraint; already in Cargo.toml | ✓ Delivered Phase 1 |
| Toast overlays render on top of output area | Avoids layout shift in the main panels | — Pending |
| Side panel is toggle-off by default | Preserves full output width for normal use | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-01 — Phase 1: Foundation complete*
