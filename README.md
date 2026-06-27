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
- Settings: models with config-defined provider profiles surfaced in the desktop catalog, compact provider metadata rendering, current Kimi/GLM coding defaults, custom provider profile templates and add/edit/delete, no-auth local provider support, provider-aware start readiness with dated cached evidence checks and targeted Settings recovery, manual provider compatibility probes with persisted redacted evidence and summary, live/static model catalog refresh including native Anthropic and Anthropic-compatible `/v1/models`, native Gemini `/v1beta/models`, and Ollama `/api/tags` with dated persisted source labeling and model selection/default saving, workspace, tools, memory, data, diagnostics, scheduler, and general service/autostart surfaces.
- Diagnostics: doctor checks, gateway runtime status, current session health alerts that ignore idle/completed turns, repair actions, trigger/session queues, and runtime loop visibility.
- Permissions: per-tool allow, deny, reset, default policy states, visible Composer permission modes for Manual Confirm, Trust Project, and Full Access, runtime workspace-scoped inheritance in the same project, pending same-workspace confirmation takeover when enabling broader access, confirmation cards that show the affected workspace/action for write, edit, shell, and MCP operations, replayable confirmation response events for approved/declined history state, and ask-user cards that state the current continue/cancel response limit. Full Access skips routine confirmation prompts but keeps explicit deny rules, external-write blocks, and catastrophic shell blocks.
- Review and background work: Agent Workbench review queue/history, severity-calibrated `/code-review` guidance, derived A2A parent/child lineage badges, completion/review-to-commit eligibility facts, plus a global background status bar and task list.
- Acceptance: browser smoke coverage for resume, diagnostics, provider probes/model catalog refresh, static fallback catalogs, selection/default saving, compact provider metadata rendering, provider evidence start readiness, custom provider profile templates/add/edit/delete, permissions including the Composer Full Access control, scheduler, A2A review, background task UI, current health alerts, stale-banner scoping, and preview ownership details, plus runtime ownership gates for mocked restart evidence, provider usage, post-shell file effects, persisted A2A lineage, review-to-commit eligibility, gated headless policy/approval checks, and the real Rust `run_worktree_worker` harness.
- Phase 8 desktop UI evidence helpers now surface recovery commands from preflight through the disposable loop status/runbook summaries when local screenshot or window observation is blocked, and the status helper can be run with `--require-live-ready` as a hard automation gate.

## Level 3 Runtime Evidence

Forge Level 3 Runtime backs long-running agent work with an append-only loop event journal, rebuildable projections, durable human gates, policy and budget preflight, typed completion evidence, crash/replay regression coverage, and gateway runner leases.

The current acceptance suite advertises and runs those runtime gates before the desktop smoke tests, so product claims about durable loop state are backed by replay, policy, gate, completion, review-to-commit eligibility, gateway status, subagent projection, mocked desktop restart evidence as partial macOS proof, provider usage known/unknown telemetry, bounded post-shell file-effect evidence, persisted A2A lineage, gated headless ownership policy/approval checks, A2A child runtime file-IO facts, direct ToolExecutor file-IO stream smoke coverage, and a real Rust `run_worktree_worker` harness using the mock adapter/harness.

Forge separates model usage from context-window status. Provider usage rows show the model call's reported tokens/cost, while the Composer context indicator shows estimated context used and true remaining context; the auto-compact threshold distance stays in the tooltip so it is not confused with provider context remaining.

The 4C.4 fake headless owner executor fixture is currently backed by focused runner, journal, projection, and replay tests. It proves runner-only orchestration state chains for completed, pending-confirmation, pending-tool-call, interrupted, cancelled, expired, and stale pending-view idempotency paths through the same journal/projection/envelope path; it is not a new acceptance-matrix or e2e autonomous resume gate.

The live file-IO evidence covers successful direct executor file-ish tools (`read`, `write`, `edit`, `diff`, `list`, and `search`), A2A child file-ish tool facts, and bounded post-shell worktree deltas with `source: "post_shell_delta"`. Boundary language stays explicit: commit remains human-gated; shell-internal tracing is not claimed; unknown provider token/cost remains unknown when adapters omit usage; gateway autonomous resume requires explicit policy and human approval. The current headless gate records and replays approval intent, safe coordinator dry-run facts, and a test-only fake executor fixture, and Forge still does not claim Tauri/WebDriver force-quit coverage, syscall/file-descriptor tracing, full non-git workspace enumeration, billing-grade usage accounting, exact cost when usage/pricing is unknown, automatic creation of parent-session context, fuzzy parent/root-task selection, real headless `AgentSession` execution, model/tool/file side effects, `gateway_can_resume=true`, pending confirmation/tool auto-acceptance, or automatic commit/merge/push behavior.

The desktop loop task panel also displays derived headless readiness and lease-pending status for waiting tasks; `gateway_can_resume` remains false, and commit/merge/push stays human-gated.

## Product Tracking

Roadmap items live in the private GitHub Project:

https://github.com/users/Cabbos/projects/3
