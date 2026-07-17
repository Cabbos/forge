# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build / Run

```bash
npm run dev          # Vite dev server on :1420 (frontend only, no Tauri)
npm run tauri dev    # Full Tauri desktop app (starts Vite + Rust backend)
npm run build        # TypeScript check + Vite production build (runs check:conversation-style first)
npm run check:backend   # cargo fmt --check + clippy -D warnings + cargo test
npm run test:e2e        # Playwright E2E
```

The `npm run tauri` script proxies to `tauri` CLI. Run individual Tauri commands like `npm run tauri -- build`.

Useful contract gates: `check:protocol` (StreamEvent sync), `check:conversation-style` (design-token/static style contract), `check:security-config` (CSP/capability), `check:desktop-boundary` (product boundary), `check:frontend-architecture` (React Query migration + boundary).

## Architecture

Forge is a **local-first AI agent workbench** built as a Tauri 2 desktop app: React/TypeScript frontend, Rust/Tokio backend. It runs its own agent loop (not a wrapper around external CLI agents): pick a local project, describe a goal, and Forge assembles project context, executes file/shell work inside workspace boundaries, streams process evidence, and preserves checkpoints so work can resume.

### Streaming protocol (the backbone)

The Rust backend streams structured events to the frontend via Tauri's `emit("session-output", StreamEvent)`. The `StreamEvent` enum is the **single source of truth for all backend→frontend communication**. It lives in two files that must stay in sync:

- `src-tauri/src/protocol/events.rs` — Rust definition (serde-tagged enum, `#[serde(tag = "event_type")]`)
- `src/lib/protocol.ts` — TypeScript mirror (discriminated union)

`npm run check:protocol` (`scripts/check-protocol-sync.mjs`) enforces the sync structurally: every Rust variant must exist in the TS union with matching field names, `skip_serializing_if` fields must stay optional on the TS side, and every event type must be handled by the dispatcher or the `eventToBlock` fallback. Field-level exceptions need a justified entry in `ALLOWED_FIELD_MISMATCH`.

Event lifecycle: `*_start` → `*_chunk` (accumulated) → `*_end`. The frontend store accumulates chunks into `BlockState[]` and persists to IndexedDB (`idb-keyval`).

### Sessions

`AppState.sessions: RwLock<HashMap<String, Arc<AgentSession>>>` (`src-tauri/src/state.rs`). All sessions are agent sessions built by `ipc/session_builder.rs`; there is no separate raw-PTY session type. `agent/session/` splits the loop into `lifecycle.rs` (start/stop/resume), `loop.rs` (round execution, loop-guard round limits), `tools.rs` (tool-call orchestration), `compact.rs` (auto/manual compaction), and `a2a.rs` (subagent delegation).

Turn flow: user input → `send_input_context` assembles context (system prompt, project docs, memory recall, project records, selected files, connector context, compacted history) → `turn_prepared` event with context-budget buckets → provider stream → tool calls via `ToolExecutor` → results fed back → next round, until final answer or the loop guard stops runaway rounds. Snapshots (`agent/snapshot.rs`) persist session/turn/delivery/resume state so interrupted turns can resume.

### Providers and credentials (`adapters/`)

All providers implement the `AiAdapter` trait (`adapters/base.rs`, key method `stream_message()`). `adapters/provider_registry.rs` catalogs built-in providers (Anthropic, DeepSeek, Kimi/Moonshot, GLM/Zhipu, Qwen, MiniMax, OpenAI, OpenRouter, Gemini, xAI, Groq, Mistral, Ollama/local); `adapters/openai_compatible.rs` covers OpenAI-compatible transports including user-defined profiles from `~/.forge/config.json` (data-only profiles — no executable plugin code). `provider_conformance.rs` and `provider_probe.rs` power the manual compatibility probe and model-catalog refresh.

API keys are **reference-only**: `settings.rs` stores a `CredentialRef` and resolves secrets from the system credential store (macOS Keychain in production builds; unsupported platforms fail closed). Plaintext `config.json`/`profiles.json` keys are migrated at startup with byte-preserving rollback. Log sinks redact registered credentials before persisting.

### Tool execution & permission gate (`executor/` + `harness/`)

`ToolExecutor` (`executor/mod.rs`, split into `FileExecutor` and `ShellExecutor`) handles file, shell, search, and web tools. For dangerous writes/shell commands it emits `ConfirmAsk` and **blocks on a oneshot channel** until the user responds via the `confirm_response` IPC command. Permission modes (manual confirm / trust project / full access) live in `harness/permissions.rs`; decisions are recorded as replayable `PermissionLedgerEvent`s (`harness/permission_ledger.rs`); shell validation is in `harness/shell_policy.rs`; workspace write boundaries in `harness/write_boundary.rs`.

### Capabilities (`harness/`)

MCP servers (`harness/mcp.rs`), hooks (`harness/hooks.rs`), and skills (`harness/skills.rs`) are managed under `harness/` with capability descriptors in `harness/capability.rs` + `harness/capabilities/`. These feed context into turns but are not user-facing product concepts.

### Long-running work (`loop_runtime/`, `gateway/`, `scheduler/`, `service/`)

`loop_runtime/` is the Level-3 runtime: append-only loop event journal, rebuildable projection, durable human gates, policy/budget preflight, typed completion evidence, and crash/replay recovery. `gateway/` hosts background trigger runs with lease/retry/dead-letter evidence; `service/` is the local service facade; `diagnostics/` aggregates runtime health. Boundary rule: gateway autonomous resume stays human-gated; local desktop ownership is the default.

### Memory and continuity (`memory/`, `continuity/`, `forge_wiki/`)

Unified memory records (`memory/`) back saved background, user facts, and project archive entries with archive/forget/recall-policy metadata. `continuity/` distills cross-session lessons. `forge_wiki/` stores per-project pages (index, decisions, tasks, log) and writeback proposals. Recall decisions are surfaced body-free in the `turn_prepared` audit.

### Frontend state (`src/store/`)

Zustand store, sliced by responsibility: `blocks.ts` (event → BlockState), `event-dispatch.ts` (central stream handler), `persistence.ts` + `hydration.ts` (IndexedDB), `usage-ledger.ts` (provider usage/cost facts), `health-alerts.ts`, `recovery-notices.ts`, `runtime-projections.ts`, and action modules (`session-actions.ts`, `workspace-actions.ts`, `context-actions.ts`, `preferences-actions.ts`). Server state for ecosystem surfaces is migrating to React Query (`src/hooks/queries/`, `src/lib/query-client.ts`).

### Frontend IPC (`src/lib/tauri.ts` + `src/lib/ipc/`)

All Tauri `invoke()` calls are wrapped here. Rust handlers live in `src-tauri/src/ipc/` (one module per domain: `session_lifecycle.rs`, `confirmations.rs`, `send_input_context.rs`, `unified_memory.rs`, `settings_handlers.rs`, …) and are registered in `lib.rs` via `generate_handler!`.

### Component tree

`App` → `AppShell` → sidebar/titlebar/session surfaces + status bar. `SessionView` contains the conversation lane (`components/chat/`) and composer (`components/session/`); per-event block renderers live in `components/messages/`. Presentation logic is extracted into pure, tested modules beside components (e.g. `processToolPresentation.ts`, `writePreviewPresentation.ts`) — keep components thin and put new derivation logic in those modules.

## Key patterns

- When adding a new stream event type: add it to BOTH `protocol/events.rs` (Rust) and `lib/protocol.ts` (TS), keep field names and optionality aligned (the sync check is field-level), handle it in `dispatchOutputEvent` and/or `eventToBlock`, and add a renderer in `components/messages/`.
- Styles: design tokens in `src/styles/tokens.css`, domain rules in per-surface files (`composer.css`, `messages.css`, `process.css`, …) coordinated by `globals.css`. `npm run check:conversation-style` is the static contract gate that runs before every build. The design language contract is `docs/product/forge-design-language.md`.
- API key status is fetched via `getApiKeyStatus()`; it returns configured/source/status/error only, never secrets.
- The `@/` path alias maps to `src/` (configured in `vite.config.ts` and `tsconfig.json`).
- shadcn/ui components live in `src/components/ui/` (config `components.json`, style "base-nova", lucide icons); app-specific primitives live in `src/components/primitives/`.
- Checkpoints use V2 Git snapshots (`ipc/checkpoint_snapshot.rs` / `agent/snapshot.rs`): staged/unstaged patches, untracked bytes, HEAD verification, and full pre-state restore on failure.

# GitNexus — Code Intelligence

This project is indexed by GitNexus as **forge-v1** (8129 symbols, 18116 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

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
