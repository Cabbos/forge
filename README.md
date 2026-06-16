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
- Review and background work: Agent Workbench review queue/history plus a global background status bar and task list.
- Acceptance: browser smoke coverage for resume, diagnostics, permissions, scheduler, A2A review, and background task UI.

## Product Tracking

Roadmap items live in the private GitHub Project:

https://github.com/users/Cabbos/projects/3
