# Forge

[中文说明](./README.md)

Forge is a local-first AI agent workbench for creating, maintaining, fixing, and continuing real software work in real projects.

It puts CLI-class coding agent capability into an auditable, resumable, sustainable desktop workbench: pick a local project, describe the goal, and Forge brings in project context, executes file and shell work inside workspace boundaries, shows process evidence, and preserves project background so the next round can continue honestly.

> Status: Forge is still in early product and internal-beta hardening. It is not a stable public release, but the core direction is clear: make local agent work safer, more visible, and easier to continue.

## Forge in 60 Seconds

- **What it is**: a local-first AI agent workbench (Tauri 2 desktop app, Rust backend + React frontend). Select a local project, describe a goal, and Forge assembles project context, executes file/shell operations inside workspace boundaries, and streams process evidence live.
- **What it solves**: not "models can't write code" — but that agent workflows are hard to trust over time: scattered context, misread projects, dangerous operations hidden inside auto-execution, no process evidence, and no honest way to resume after interruption.
- **Three product layers**: Current Task → Project Archive → Delivery. Internal machinery (Workflow Router, Context Activation, Memory, Auto Compact, MCP, Skills) never becomes a concept users must learn.
- **Five engineering commitments**: a single `StreamEvent` protocol contract across frontend/backend (field-level sync gate); workspace boundaries + confirmation gates for risky actions (confirm cards carry permission evidence; approve/cancel decisions are replayable); Checkpoint V2 Git snapshots with full round-trip restore; API keys stored only in the system credential store with log redaction; evidence gaps explicitly marked `unknown` instead of guessed.
- **Current status**: early internal beta. See "Current Boundaries" below for the explicit boundary list and "Development Commands" for the release gate matrix.

## Why Forge Exists

Coding agents are strong, but the common failure in real use is not model quality — it is that workflows are hard to trust long-term:

- Task context is scattered across chats, files, terminals, and notes.
- Agents misread the current project, or act in the wrong workspace.
- Shell, file-write, and connector calls lack clear risk confirmation.
- Process evidence is dispersed, so users cannot tell whether work is actually done.
- After an interruption, the next round cannot honestly pick up where things left off.

Forge's product hypothesis: a local agent should be neither a chat window nor a terminal wrapper. It should be a workbench organized around projects, evidence, permissions, and delivery state.

## Core Commitments

| Commitment | How Forge Delivers |
| --- | --- |
| Work only inside the current project | Every session binds to a local workspace; `@file` search and file reads stay inside the project boundary. |
| Visible process | Thinking summaries, tool calls, shell output, diffs, checkpoints, verification results, and delivery state all render as a structured event stream. |
| Confirmable risky actions | High-risk shell commands, file writes, and connector calls trigger confirmation — dangerous operations are never hidden inside auto-execution. |
| Continuable context | Forge composes project instructions, saved background, project archive records, user-selected files, connector context, and compacted history. |
| Judgeable results | Each turn is organized around the current task, project archive, and delivery state; preview URLs carry project attribution so users can decide to continue, verify, fix, or stop. |

## Product Layers

Forge asks users to understand only three product layers:

| Layer | Meaning |
| --- | --- |
| Current Task | What Forge believes the user is driving right now. |
| Project Archive | Durable project notes, decisions, background, task logs, and reusable materials. |
| Delivery | Preview state, checkpoints, verification results, risk hints, and next actions. |

Internal capabilities — Workflow Router, Context Activation, Memory, Auto Compact, Wiki Storage, MCP, Hooks, and Skills — should never become a burden on the user. They are the capability layer behind Forge, not new concepts to learn.

## What Forge Does

- Open a local project and run multi-turn agent tasks against it.
- Configure and select models across DeepSeek, Anthropic, Kimi/Moonshot, GLM/Zhipu, Alibaba/Qwen, MiniMax, OpenAI, OpenRouter, Gemini, xAI, Groq, Mistral, Ollama/local, and custom compatible providers.
- Stream agent output, tool activity, shell results, diffs, confirmation requests, and delivery summaries into the desktop UI.
- Reference project files with `@file`, injecting selected files as hidden turn context.
- Read project-level instruction files such as `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`.
- Compose per-turn context from saved background, project archive records, connector context, and recent conversation history; the project archive unifies saved background, user facts, and continuity lessons with archive/forget actions.
- Support MCP Resources, Prompts, Tools, plus local Hooks, Skills, and capability management.
- Intercept dangerous commands, file writes, and external connector actions for confirmation; confirm cards show the project path, affected files, single-use scope, and backend permission evidence, and approve/cancel responses are written to history as replayable events. The composer offers `manual confirm`, `trust project`, and `full access` permission modes; out-of-workspace writes, disaster commands, and explicit deny rules stay blocked in every mode.
- Record task state, context sources, tool evidence, preview attribution, checkpoints, verification results, and resume state.
- Track provider usage, context usage, and cumulative cost; unknown provider usage stays explicitly `unknown` rather than fabricated.
- Search, filter, resume, rename, export, and prune local session snapshots in History.
- Inspect diagnostics, Gateway runtime, scheduled tasks, permission rules, memory, and local service status in Settings.

## Current Boundaries

Forge deliberately keeps its boundaries clear:

- No cloud collaboration, org management, hosted execution, enterprise gateway, or billing.
- No promise of fully unattended long-running autonomous execution; commit/merge/push remain human-gated.
- Not a replacement for Git, IDEs, terminals, or code review — agent work lives inside an inspectable local workbench.
- Internal context-engineering jargon stays out of the UI; the product speaks in task, project, and delivery language.

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

Set provider keys from Settings or write them to `~/.forge/config.json`:

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

Custom providers are data-only profiles: they can add or override a provider's label, transport, base URL, key/model environment variables, default model, and capability flags, and they appear in Settings and the composer model menu — but they never load executable plugin code.

**Credential storage is reference-only.** Production builds on macOS resolve secrets from the system Keychain; platforms without system credential support fail closed. Plaintext keys in legacy `config.json`/`profiles.json` are migrated at startup with deterministic references and byte-preserving rollback. Log sinks redact registered credentials, auth headers, sensitive JSON fields, and URL query/fragment values before persisting; if redaction fails, the log line is suppressed instead of written.

**Checkpoints use V2 Git snapshots**: full HEAD, porcelain-v2 status, separate staged/unstaged full-index binary patches, untracked file bytes, and executable bits — round-tripping staged-only, unstaged-only, same-file double edits, renames/deletes, binaries, and unborn repositories. Restore validates schema, paths, sizes, and HEAD before touching the workspace, and any mid-apply failure restores the pre-call state completely.

## Development Commands

```bash
npm run dev            # Vite dev server only
npm run tauri dev      # Full Tauri desktop app
npm run build          # TypeScript check + Vite production build
npm run tauri:build    # Desktop bundle
npm run test:e2e       # Playwright e2e tests
npm run check:backend  # Rust fmt + clippy + test
```

The repo root provides the Level 3 runtime acceptance harness:

```bash
scripts/acceptance.sh          # contract matrix + build + eval + runtime + desktop smoke
scripts/acceptance.sh --dry-run
scripts/acceptance.sh --list-json
scripts/acceptance.sh --ci-default
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
  - AgentSession: agent loop, tool orchestration, context assembly, compaction, verification
  - ContextBuilder: system prompt, summaries, selected files, project records, saved background, connectors, history
  - ToolExecutor / Harness: file, shell, MCP, hooks, skills, permission control
  - Snapshot storage: session, turn, current task, delivery, checkpoint, resume state
  - Project Archive: local project records and writeback proposals
```

The streaming protocol is the backbone of the app. The Rust backend emits structured `StreamEvent`s to the frontend, where the store accumulates them into renderable `BlockState[]`. When adding a backend-to-frontend event, update both:

- `src-tauri/src/protocol/events.rs`
- `src/lib/protocol.ts`

`npm run check:protocol` verifies the two stay in sync at field level. Then update the Zustand store and add or adjust a renderer under `src/components/messages/`.

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

## Reliability Direction

Forge's trustworthiness comes from engineering constraints:

- A shared `StreamEvent` protocol contract across frontend and backend.
- Workspace boundary checks and session binding.
- Confirmation gates on file writes, dangerous shell commands, and connector calls.
- Explicit presentation of checkpoints, verification results, and delivery state.
- A Rust + Tokio backend carrying the agent loop, IPC, PTY, MCP, and local storage.
- Playwright e2e and Rust backend checks covering critical paths.
- Gateway runtime, scheduler, diagnostics, and session-store state observable from Settings/CLI.
- Boundary language stays explicit: `commit remains human-gated`; `unknown provider token/cost remains unknown when adapters omit usage`; `gateway autonomous resume requires explicit policy and human approval`.

## Product Direction

V1 is not about more panels — it is about making the local agent core loop trustworthy:

- Pick a project, with an explicit current workspace.
- Describe a task; Forge brings the necessary context automatically.
- Process evidence is visible; risky actions are confirmable.
- Results are verifiable; failures are recoverable.
- The next round can continue from reliable records.

V2 moves toward deeper project-native intelligence: the more Forge knows a project, the better it picks context, follows project conventions, flags risky files, and helps users keep going without exposing internal machinery.

See the Chinese README for the full evidence-level capability and boundary language; product detail docs live in [`docs/product/`](./docs/product/).

## Repository Hygiene

Local tool state and development workflow notes should not be committed:

- `.forge/`
- `.agents/`
- `.claude/`
- `.superpowers/`
- `docs/superpowers/`
- `test-results/`

Use Obsidian or another external knowledge base for long-running product planning notes.
