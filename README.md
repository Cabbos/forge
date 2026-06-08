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

## Product Tracking

Roadmap items live in the private GitHub Project:

https://github.com/users/Cabbos/projects/3
