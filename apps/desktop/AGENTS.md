# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Build / Run

```bash
npm run dev          # Vite dev server on :1420 (frontend only, no Tauri)
npm run tauri dev    # Full Tauri desktop app (starts Vite + Rust backend)
npm run build        # TypeScript check + Vite production build
```

The `npm run tauri` script proxies to `tauri` CLI. Run individual Tauri commands like `npm run tauri -- build`.

## Architecture

This is a **Tauri 2.0 desktop app** ("TUI-to-GUI") — a GUI wrapper around CLI AI coding agents. Rust backend, React/TypeScript frontend.

### Streaming protocol (the backbone)

The Rust backend streams structured events to the frontend via Tauri's `emit("session-output", StreamEvent)`. The `StreamEvent` enum is the **single source of truth for all backend→frontend communication**. It lives in two files that must stay in sync:

- `src-tauri/src/protocol/events.rs` — Rust definition (serde-tagged enum)
- `src/lib/protocol.ts` — TypeScript mirror (discriminated union)

Event lifecycle: `*_start` → `*_chunk` (accumulated) → `*_end`. The frontend store (`src/store/index.ts`) accumulates chunks into `BlockState[]` and persists to IndexedDB.

### Session types

There are two kinds of sessions, both stored in `AppState.sessions: HashMap<String, Session>`:

| Variant | Source | Use case |
|---|---|---|
| `Session::Agent(AgentSession)` | `agent/session.rs` | AI agent (Codex/Codex/Hermes) — API calls + tool execution loop |
| `Session::Cli(CliSession)` | `pty/session.rs` | Raw PTY bash session via `portable-pty` |

### Agent loop (`agent/session.rs`)

User message → add to history → API stream → collect `tool_calls` → execute via `ToolExecutor` → feed `tool_result` back → loop (max 10 rounds). History windowed to 30 turns, preserving tool_use/tool_result pairs.

### AI adapters (`adapters/`)

All providers implement the `AiAdapter` trait (`base.rs`), which has one key method: `stream_message()`. The trait uses `async_trait`. Three adapters: `AnthropicAdapter` (Codex), `OpenAiAdapter` (Codex), and the Codex adapter is reused for Hermes.

### Tool execution & permission gate (`executor/`)

`ToolExecutor` handles: `read_file`, `write_file` (with permission check), `run_shell`/`bash`. For dangerous writes, it emits `ConfirmAsk` to the frontend and **blocks** on a `oneshot` channel until the user clicks Yes/No via the `confirm_response` IPC command. The channel is stored in `AppState.pending_confirms`.

### Plugin system (`plugin_manager/`)

Four plugin types: `McpServer`, `Hook`, `Skill`, `Extension`. Three agent targets: `Codex`, `Codex`, `Hermes`. Plugins are scanned locally and discovered from a registry. Installation is handled by `PluginInstaller`.

### Frontend state (`src/store/index.ts`)

Zustand store. State is persisted across restarts via IndexedDB (`idb-keyval`). Key actions: `hydrate` (load on startup), `dispatchOutputEvent` (the central stream event handler), `addSession`/`removeSession`.

### Frontend IPC (`src/lib/tauri.ts`)

All Tauri `invoke()` calls are wrapped here. The Rust handlers are in `ipc/handlers.rs` and registered in `lib.rs`.

### Component tree

`App` → `AppShell` → `Sidebar` + `SessionView` + `StatusBar`. `SessionView` contains `ChatView` → `MessageList` (virtualized) → per-type block renderers in `components/messages/`.

## Key patterns

- When adding a new stream event type: add it to BOTH `protocol/events.rs` (Rust) and `lib/protocol.ts` (TS), plus handle it in the store's `dispatchOutputEvent` and create a renderer component in `components/messages/`.
- API keys are stored in `~/.forge/config.json` via `settings.rs`. The frontend fetches status via `getApiKeyStatus()`.
- The `@/` path alias maps to `src/` (configured in both `vite.config.ts` and `tsconfig.json`).
- shadcn/ui components live in `src/components/ui/`. The config is in `components.json` (style: "base-nova").

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **forge-v1** (7234 symbols, 15998 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/forge-v1/context` | Codebase overview, check index freshness |
| `gitnexus://repo/forge-v1/clusters` | All functional areas |
| `gitnexus://repo/forge-v1/processes` | All execution flows |
| `gitnexus://repo/forge-v1/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
