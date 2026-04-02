// Fin — Contributing Guide
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Contributing to Fin

## Build & Run

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release

# Run in TUI mode
cargo run

# Run in print mode
cargo run -- -p "hello"

# Run tests
cargo test
```

Requires Rust 1.85+ (edition 2024).

## Project Structure

```
src/
  agent/       Core agent loop, state, compaction, handoff
  llm/         LLM providers (Anthropic, OpenAI, Google, Vertex, Bedrock)
  tools/       Built-in tools (bash, read, write, edit, grep, glob, git)
  workflow/    Workflow engine — stages, dispatch, CRUD, git automation
  db/          SQLite persistence, .planning/ directory, session storage
  io/          I/O adapters (TUI, headless, print, HTTP, RPC/MCP)
  tui/         Terminal UI (ratatui)
  config/      Preferences, auth, path resolution
  agents/      Sub-agent delegation (discovery, registry, delegation tool)
  extensions/  Plugin system
  skills/      Markdown-based skill loading
```

## Terminology

Fin uses its own terminology distinct from its GSD predecessor:

| Concept       | Fin Term     | ID Format |
|---------------|-------------|-----------|
| Major release | Blueprint   | B###      |
| Work unit     | Section     | S##       |
| Atomic work   | Task        | T##       |

### Workflow Stages

| Stage       | Purpose                        |
|-------------|--------------------------------|
| Define      | Discover requirements          |
| Explore     | Research feasibility           |
| Architect   | Design solution                |
| Build       | Implement                      |
| Validate    | Test & verify                  |
| SealSection | Finalize & document            |
| Advance     | Mark complete, prepare next    |

### Artifact Files

| File            | Purpose                |
|-----------------|------------------------|
| VISION.md       | Blueprint goals        |
| BRIEF.md        | Section context        |
| FINDINGS.md     | Research results       |
| SPEC.md         | Implementation plan    |
| REPORT.md       | Completion summary     |
| ACCEPTANCE.md   | Validation criteria    |
| STATUS.md       | Current state          |
| LEDGER.md       | Decision log           |
| handoff.md      | Session resume context |

### Blueprint Wizard (`/blueprint`)

The `/blueprint` command is the single entry point for workflow management:

| Command | Action |
|---------|--------|
| `/blueprint <name>` | Create new blueprint (blocked if one is active) |
| `/blueprint` | Show blueprint list + usage (if no active blueprint) |
| `/blueprint` | Health check + resume (if blueprint is active) |
| `/blueprint list` | List all blueprints with status |
| `/blueprint complete` | Mark active blueprint as done |

**One active blueprint at a time.** Creating a new blueprint is blocked while one is in progress. The wizard runs a health check on resume to fix state inconsistencies.

### Workflow Agents (`.fin/agents/`)

Fin uses specialized agents for stage delegation. Agents are seeded to `.fin/agents/` on `fin init`.

| Agent | Role | Used By |
|-------|------|---------|
| `fin-researcher` | researcher | Explore stage |
| `fin-planner` | planner | Architect stage |
| `fin-builder` | builder | Build stage |
| `fin-reviewer` | reviewer, tester | Validate, SealSection stages |

**Key rules:**
- Workflow agents are loaded exclusively from `.fin/agents/` — external agents cannot participate
- Only `fin-*` prefixed agent IDs are accepted (enforced by `find_workflow_role()`)
- Agents are workers — they cannot override workflow prompts or stage sequence
- Delegation is optional — stages work inline if no agents are available
- Each agent has its own model tier (defined in the `.md` frontmatter)

Agent definition format (`.fin/agents/fin-*.md`):
```yaml
---
name: fin-researcher
description: "Codebase and technology research"
color: blue
tools: Read, Grep, Glob, Bash
model: sonnet
roles: researcher
---

System prompt here...
```

## Code Style

- `cargo fmt` before committing
- `cargo clippy -- -D warnings` must pass
- All files must include the copyright header:
  ```rust
  // Fin — <File Purpose>
  // Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>
  ```

## Testing

- Unit tests go in `#[cfg(test)] mod tests` at the bottom of each source file
- Integration tests go in `tests/integration_test.rs`
- Run `cargo test` to run all tests
- Run `scripts/e2e_blueprint.sh --mode offline` for deterministic end-to-end blueprint workflow validation
- Run `scripts/e2e_blueprint.sh --mode online` for live provider-backed workflow validation (requires API credentials)

## Commit Messages

Follow conventional commits:

```
feat: add agent delegation tool
fix: handle empty SSE data lines
refactor: rename Phase to Stage across codebase
test: add unit tests for model registry
docs: update CONTRIBUTING.md with terminology
```

Keep the first line under 72 characters. Use the body for details if needed.

## Feature Flags

| Flag      | Default | Description           |
|-----------|---------|-----------------------|
| `tui`     | on      | Terminal UI (ratatui) |
| `http`    | on      | HTTP API (axum)       |
| `mcp`     | off     | MCP protocol support  |
| `browser` | off     | Playwright CDP bridge |
