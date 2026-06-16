# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in the `apps/desktop` frontend.

## Build / Run

```bash
npm run dev          # Vite dev server on :1420 (frontend only, no Tauri)
npm run tauri dev    # Full Tauri desktop app (starts Vite + Rust backend)
npm run build        # TypeScript check + Vite production build
npm run test:e2e -- e2e/acceptance.spec.ts
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
| `Session::Agent(AgentSession)` | `agent/session.rs` | AI agent (Claude/Codex/Hermes) — API calls + tool execution loop |
| `Session::Cli(CliSession)` | `pty/session.rs` | Raw PTY bash session via `portable-pty` |

### Agent loop (`agent/session.rs`)

User message → add to history → API stream → collect `tool_calls` → execute via `ToolExecutor` → feed `tool_result` back → loop (max 10 rounds). History windowed to 30 turns, preserving tool_use/tool_result pairs.

### AI adapters (`adapters/`)

All providers implement the `AiAdapter` trait (`base.rs`), which has one key method: `stream_message()`. The trait uses `async_trait`. Three adapters: `AnthropicAdapter` (Claude), `OpenAiAdapter` (Codex), and the Claude adapter is reused for Hermes.

### Tool execution & permission gate (`executor/`)

`ToolExecutor` handles: `read_file`, `write_file` (with permission check), `run_shell`/`bash`. For dangerous writes, it emits `ConfirmAsk` to the frontend and **blocks** on a `oneshot` channel until the user clicks Yes/No via the `confirm_response` IPC command. The channel is stored in `AppState.pending_confirms`.

### Plugin system (`plugin_manager/`)

Four plugin types: `McpServer`, `Hook`, `Skill`, `Extension`. Three agent targets: `Claude`, `Codex`, `Hermes`. Plugins are scanned locally and discovered from a registry. Installation is handled by `PluginInstaller`.

### Frontend state (`src/store/index.ts`)

Zustand store. State is persisted across restarts via IndexedDB (`idb-keyval`). Key actions: `hydrate` (load on startup), `dispatchOutputEvent` (the central stream event handler), `addSession`/`removeSession`.

### Frontend IPC (`src/lib/tauri.ts`)

All Tauri `invoke()` calls are wrapped here. The Rust handlers are in `ipc/handlers.rs` and registered in `lib.rs`.

### Component tree

`App` → `AppShell` → `Sidebar` + `main-workbench` (`AppTitlebar` + `SessionView`/`EmptyWorkbench`) + `CapabilityDrawer` + `CommandPalette` + `HubPanelHost`.

- `SessionView` → `ChatView` → `MessageList` (virtualized) → `ConversationLane` → per-type block renderers in `components/messages/`.
- `SessionView` → `InputBar` → `ComposerSurface` + `ComposerMenuLayer` + `ComposerToolbar` + `ComposerChipTray`.
- `SettingsDialog` wraps `SettingsCenterShell` with nav sections: models, workspace, tools, memory, data, diagnostics, scheduler, general/service, and about.
- `HistoryView` is lazy-loaded from the sidebar and uses session-store IPC for search, provider filtering, resume, delete, rename, export, and prune.
- `StatusBar` sits at the bottom of `AppShell` and derives active A2A work, review items, scheduler tasks, and health alerts into compact background status/task rows.
- `HubPanelHost` owns the Agent Workbench, including A2A review queue/history and retained-worktree context.
- `CommandPalette` is the global search/switch surface (Cmd+K).

## Key patterns

- When adding a new stream event type: add it to BOTH `protocol/events.rs` (Rust) and `lib/protocol.ts` (TS), plus handle it in the store's `dispatchOutputEvent` and create a renderer component in `components/messages/`.
- API keys are stored in `~/.forge/config.json` via `settings.rs`. The frontend fetches status via `getApiKeyStatus()`.
- The `@/` path alias maps to `src/` (configured in both `vite.config.ts` and `tsconfig.json`).
- shadcn/ui components live in `src/components/ui/`. The config is in `components.json` (style: "base-nova").
- Product-level runtime smoke coverage lives in `e2e/acceptance.spec.ts`; extend it when Settings, History, diagnostics, permissions, scheduler, A2A review, or background task surfaces change.
- Shared e2e IPC mocks live in `e2e/fixtures/app.ts`. Keep those mocks contract-shaped, not implementation-shaped, so acceptance specs exercise the same UI paths users see.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **forge** (monorepo root: `/Users/cabbos/project/forge`). The desktop app lives under `apps/desktop`. Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

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
| `gitnexus://repo/forge/context` | Codebase overview, check index freshness |
| `gitnexus://repo/forge/clusters` | All functional areas |
| `gitnexus://repo/forge/processes` | All execution flows |
| `gitnexus://repo/forge/process/{name}` | Step-by-step execution trace |

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
