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
- API keys are stored in `~/.tui-to-gui/config.json` via `settings.rs`. The frontend fetches status via `getApiKeyStatus()`.
- The `@/` path alias maps to `src/` (configured in both `vite.config.ts` and `tsconfig.json`).
- shadcn/ui components live in `src/components/ui/`. The config is in `components.json` (style: "base-nova").
