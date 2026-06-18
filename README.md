# Forge

Forge is a local-first AI Agent Workbench product.

This monorepo keeps the three main product lines together:

- `apps/desktop` - the Forge Tauri desktop app.
- `apps/eval-runner` - backtest, continuity, and agent trace evaluation tooling.
- `apps/website` - the Forge product website prototype.

The first migration keeps each existing project mostly intact. Shared packages and crates should only be extracted once the dependency boundary is real.

## Development

```bash
npm run build:desktop
npm run build:website
npm run test:eval
```

For the current desktop acceptance sweep, run:

```bash
scripts/acceptance.sh
```

Use `scripts/acceptance.sh --dry-run` to print the gate plan without running it.

## Desktop Runtime Surfaces

The desktop app now includes the Hermes-parity runtime scaffolding that is being hardened in Phase 7:

- Session History: search, provider filtering, resume, delete, rename, JSON export, and conservative pruning.
- Settings: models, workspace, tools, memory, data, diagnostics, scheduler, and general service/autostart surfaces.
- Diagnostics: doctor checks, gateway runtime status, repair actions, trigger/session queues, and runtime loop visibility.
- Permissions: per-tool allow, deny, reset, and default policy states for write, edit, shell, and MCP operations.
- Review and background work: Agent Workbench review queue/history, derived A2A parent/child lineage badges, plus a global background status bar and task list.
- Acceptance: browser smoke coverage for resume, diagnostics, permissions, scheduler, A2A review, and background task UI, plus a real Rust `run_worktree_worker` harness gate.

## Level 3 Runtime Evidence

Forge Level 3 Runtime backs long-running agent work with an append-only loop event journal, rebuildable projections, durable human gates, policy and budget preflight, typed completion evidence, crash/replay regression coverage, and gateway runner leases.

The current acceptance suite advertises and runs those runtime gates before the desktop smoke tests, so product claims about durable loop state are backed by replay, policy, gate, completion, gateway status, subagent projection, provider usage known/unknown telemetry, mocked desktop acceptance checks, A2A child runtime file-IO facts, direct ToolExecutor file-IO stream smoke coverage, bounded post-shell file-effect evidence smoke coverage, and a real Rust `run_worktree_worker` harness using the mock adapter/harness.

The live file-IO evidence covers successful direct executor file-ish tools (`read`, `write`, `edit`, `diff`, `list`, and `search`), A2A child file-ish tool facts, and bounded post-shell worktree deltas with `source: "post_shell_delta"`. Provider usage evidence covers active Anthropic and OpenAI-compatible model calls: reported tokens are known, omitted usage remains unknown, and unknown pricing leaves cost unknown. Forge still does not claim Tauri/WebDriver force-quit coverage, shell-internal file effect tracing from `run_shell`, syscall/file-descriptor tracing, full non-git workspace enumeration, billing-grade usage accounting, exact cost when usage/pricing is unknown, gateway autonomous resume, automatic parent selection, parent-session structs, or automatic commit/merge/push behavior.

## Product Tracking

Roadmap items live in the private GitHub Project:

https://github.com/users/Cabbos/projects/3
