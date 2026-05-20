# Forge

Forge is a local-first desktop AI agent for building and maintaining software projects.

It is built with Tauri 2, React, TypeScript, and Rust. The product direction is simple: open a local project, describe what you want, and let Forge gather the right context, act inside the project boundary, show evidence, and keep enough project records to continue later.

Forge is currently an early product build. Expect fast iteration and breaking changes.

## What Forge Does

- Works against a selected local project folder.
- Runs multi-turn coding agent loops with tool calls.
- Streams thinking, tool activity, shell output, diffs, confirmations, and delivery summaries into a desktop UI.
- Supports provider/model selection, including DeepSeek, Anthropic, OpenAI, and OpenRouter.
- Uses project records, saved background, selected files, and connector context as hidden turn context.
- Lets users reference project files with `@file` and scopes file search to the active session project.
- Requires confirmation for risky shell commands, file writes, and connector actions.
- Tracks turn state, context sources, tool evidence, verification results, checkpoints, and delivery state.
- Supports MCP resources/prompts/tools, hooks, skills, and local capability management.

## Product Shape

Forge keeps the user-facing model intentionally small:

| Product Layer | Meaning |
| --- | --- |
| Current Task | What Forge believes the user is trying to do right now. |
| Project Archive | Durable project notes, decisions, sources, task logs, and saved background. |
| Delivery | Preview/runtime state, checkpoint state, verification, and next action. |

Internal terms such as workflow routing, context activation, memory, auto compact, and wiki storage are implementation details. The UI should speak in product language.

## Quick Start

```bash
npm install
npm run tauri dev
```

For frontend-only development:

```bash
npm run dev
```

Vite runs on `http://localhost:1420`. The full desktop app is launched through Tauri.

## API Keys

Set provider keys from Settings (`Cmd+,`) or write them to `~/.forge/config.json`:

```json
{
  "api_keys": {
    "deepseek": "sk-...",
    "anthropic": "sk-ant-...",
    "openai": "sk-...",
    "openrouter": "sk-or-..."
  }
}
```

Environment variables are also detected for common providers:

```bash
DEEPSEEK_API_KEY=...
ANTHROPIC_API_KEY=...
OPENAI_API_KEY=...
OPENROUTER_API_KEY=...
```

DeepSeek is the default provider. The default model is `deepseek-v4-flash[1m]`.

## Development Commands

```bash
npm run dev          # Vite dev server only
npm run tauri dev    # Full Tauri desktop app
npm run build        # TypeScript check + Vite production build
npm run tauri:build  # Desktop bundle
npm run test:e2e     # Playwright e2e tests
```

Rust tests:

```bash
cd src-tauri
cargo test
```

## Architecture

```text
React frontend (Vite + TypeScript)
  -> Tauri IPC commands
  <- StreamEvent protocol
Rust backend (Tokio)
  - AgentSession: agent loop, context assembly, compaction, verification
  - ContextBuilder: system prompt, summaries, selected files, project records, saved background, connectors, history
  - ToolExecutor / Harness: file, shell, MCP, hooks, skills, permissions
  - Project Archive: local markdown-like project records and writeback proposals
  - Snapshot storage: session, turn, workflow, delivery, and resume state
```

The streaming protocol is the backbone of the app. When adding a backend-to-frontend event, update both:

- `src-tauri/src/protocol/events.rs`
- `src/lib/protocol.ts`

Then update the Zustand store and add or adjust a renderer under `src/components/messages/`.

## Local Context Model

Forge assembles context per turn from typed sources:

- System prompt and project instructions
- Compacted conversation summary
- User-selected `@file` references
- Saved background
- Project Archive records
- MCP connector context
- Recent conversation history

Selected files are read only from the active workspace, size-limited, and blocked from escaping the workspace through absolute paths or symlinks.

## Project Instructions

Forge reads project-level instructions from files such as:

- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`

These are used as project guidance when the selected workspace contains them.

## Repository Hygiene

Local tool state and development workflow notes should not be committed:

- `.forge/`
- `.agents/`
- `.claude/`
- `.superpowers/`
- `docs/superpowers/`
- `test-results/`

Use Obsidian or another external knowledge base for long-running product planning notes.

## Status

Forge is not yet a polished public release. The current focus is:

- making the local agent loop reliable,
- keeping workspace boundaries clear,
- improving context and resume behavior,
- making the desktop UI feel calm and professional,
- supporting non-programmers without slowing down professional developers.
