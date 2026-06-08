# AGENTS.md

This is the Forge product monorepo.

## Structure

- `apps/desktop`: Forge Tauri desktop app.
- `apps/eval-runner`: Forge eval runner and backtest service.
- `apps/website`: Forge website prototype.

Keep the three apps independently runnable in the first migration. Do not extract shared packages until code is actually shared by at least two apps.

## Commands

```bash
npm run build:desktop
npm run build:website
npm run test:eval
```

When editing an imported app, also read that app's local `AGENTS.md` if present.
