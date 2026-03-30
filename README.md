# fin

AI coding agent — one command, walk away, come back to a built project with clean git history.

**3.7MB binary. Zero runtime dependencies. Instant startup.**

## Quick Start

```bash
# Build
cargo build --release

# Set your API key
export OPENAI_API_KEY=sk-...
# or: export ANTHROPIC_API_KEY=sk-ant-...
# or: export GOOGLE_API_KEY=...

# Single-shot mode
fin -p "read the README and summarize this project"

# Interactive TUI
fin

# List available models
fin models
```

## Modes

| Mode | Command | Transport | Use Case |
|------|---------|-----------|----------|
| **Print** | `fin -p "prompt"` | stdout streaming | Scripts, CI/CD |
| **Interactive** | `fin` | TUI (ratatui) | Developer at terminal |
| **Headless** | `fin headless "prompt"` | JSONL stdin/stdout | Piped into tools |
| **HTTP API** | `fin serve` | REST + SSE | Web clients |
| **MCP** | `fin mcp` | stdio JSON-RPC | Claude Code, Cursor |

## Tools

7 built-in tools + 2 extension tools:

| Tool | Description |
|------|-------------|
| `bash` | Execute shell commands |
| `read` | Read files with line numbers |
| `write` | Create/overwrite files |
| `edit` | String replacement editing |
| `grep` | Regex content search (ripgrep) |
| `glob` | File pattern matching |
| `git` | Version control operations |
| `web_search` | Brave/Tavily web search (extension) |
| `resolve_library` | Context7 library docs (extension) |

## LLM Providers

All via raw HTTP — no SDKs, no bloat:

| Provider | Models | Auth |
|----------|--------|------|
| Anthropic | Claude Opus/Sonnet/Haiku 4.x | `ANTHROPIC_API_KEY` |
| OpenAI | GPT-4.1, o3 | `OPENAI_API_KEY` |
| Google | Gemini 2.5 Pro/Flash | `GOOGLE_API_KEY` |
| Google Vertex AI | Claude Sonnet/Haiku (Vertex) | `GOOGLE_APPLICATION_CREDENTIALS` |
| AWS Bedrock | Claude Sonnet/Haiku (Bedrock) | `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` |
| Ollama | Any locally running model | *(auto-discovered)* |

## Project Stats

```
86 Rust source files
~21,000 lines of code
3.7MB release binary (stripped, LTO)
7 integration tests
0 runtime dependencies
```

## Development

```bash
cargo build          # Debug build
cargo test           # Run tests
cargo build --release  # Optimized release build
cargo clippy         # Lint
cargo fmt            # Format
```

## License

MIT — Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

---

Proudly built with [GSD](https://github.com/gsd-build/get-shit-done)
