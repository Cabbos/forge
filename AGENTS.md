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
scripts/acceptance.sh --dry-run
```

When editing an imported app, also read that app's local `AGENTS.md` if present.

## Current Desktop Product Surfaces

The active desktop hardening work is tracked in `docs/superpowers/plans/2026-06-12-forge-hermes-runtime-gap-roadmap.md`.
Recent Phase 7 surfaces include History session management, Settings diagnostics, permission rules, scheduler UI, A2A review summaries, background task status/list UI, and the Phase 7 acceptance script.

When touching these surfaces, keep docs and acceptance coverage in sync:

- Update `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` when a user-visible runtime surface changes.
- Prefer adding or extending `apps/desktop/e2e/acceptance.spec.ts` for product-level smoke coverage.
- Keep `scripts/acceptance.sh --dry-run` aligned with the specs it advertises.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **forge** (12778 symbols, 33301 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `pnpm --allow-build=@ladybugdb/core --allow-build=gitnexus --allow-build=tree-sitter --allow-build=tree-sitter-kotlin dlx gitnexus@latest analyze --index-only` from the project root. The generated `.gitnexus/run.cjs` can fall back to an npx cache missing optional grammars (`tree-sitter-swift` / Kotlin native build), so prefer the explicit pnpm command until the upstream runner is fixed.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

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
