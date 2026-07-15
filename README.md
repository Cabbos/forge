# Forge

Eval CLI release runs now return success only for trusted, threshold-passing evidence; use `--report-only` to inspect incomplete evidence without turning trust blockers into the process exit gate.
Eval report artifacts expose score means separately from evidence coverage, so missing feature-specific evidence cannot masquerade as a complete release signal.
The Eval runner independently observes workspace effects instead of trusting agent-reported paths, with bounded process groups, sandbox scrub evidence, and fresh-fixture patch replay in the R2/R3 gate.
The desktop runtime now enforces an explicit production CSP, a localhost-only development CSP, frozen JavaScript prototypes, and a minimal main-window capability covering only folder selection and session event subscription.

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

The public-beta eval release slice has four stable acceptance labels:

```text
eval execution identity baseline
eval independent workspace evidence baseline
eval trusted execution baseline
eval authenticated fenced worker baseline
```

Run any one with `scripts/acceptance.sh --only "<label>"`. The fixed eval quality
commands used by the matrix are:

```bash
cd apps/eval-runner
uv sync --frozen --dev
uv run pytest -q
uv run ruff check .
uv run ruff format --check .
uv run mypy app
```

For the current desktop acceptance sweep, run:

```bash
scripts/acceptance.sh
```

Use `scripts/acceptance.sh --dry-run` to print the gate plan without running it, `scripts/acceptance.sh --list-json` for machine-readable gate metadata, `scripts/acceptance.sh --only "<label>"` to run one exact gate, `scripts/acceptance.sh --grep "<text>"` to filter gates by case-insensitive label substring, or `scripts/acceptance.sh --results-json gate-results.json` to write executed gate statuses plus domain/tier metadata for release confidence reports.
Full acceptance starts with `node --test scripts/acceptance.test.mjs` so the executable gate matrix, generated help text, JSON output, domain/tier/runtime-cost/manual-evidence metadata, exact-label runner, and gate-result JSON output stay in sync. Backend gates are tagged by authority domain: runtime, permission, usage/context, memory, gateway, eval, and UI evidence; release tiers are `fast-contract`, `runtime-core`, `desktop-ui`, `manual-evidence`, and `full-release`, with `fast-contract` plus `runtime-core` marked as CI-default tiers. Use `scripts/acceptance.sh --ci-default` to run or dry-run that same CI-default subset. `node scripts/release-confidence-summary.mjs --markdown --gate-results gate-results.json` can turn the matrix plus optional gate-result, eval-report, and boundary evidence into a PR-ready release confidence summary, including task-level failing eval scores when a report does not provide `score_summary`, gate-results execution completeness and execution reason evidence, dashboard artifact output with `--out-dir`, CI-default gate totals/pass/fail/unknown counts beside the full matrix, acceptance domain/tier breakdowns, gate detail metadata for failed/manual/unknown gates, verified capability evidence when referenced gates and scores pass, and capability evidence gaps when a declared capability points at a missing acceptance gate, missing acceptance result, failing acceptance gate, missing eval score, or failing eval score; add `--ci-default-only` when the report should ignore non-CI gates without supplied results, add `--no-acceptance-matrix` when a self-describing gate-results file should be the only acceptance source, and add `--fail-on-attention` when the report should behave like a gate and exit nonzero for `attention_required` or `failed`.

For local GitNexus CLI or index refresh commands, use `node scripts/gitnexus-safe.mjs -- <command>`. It applies a 60 second timeout and prints the fallback impact-report template; `node scripts/gitnexus-safe.mjs --print-template` prints the template without running a command.

## Desktop Runtime Surfaces

The desktop app now includes the Hermes-parity runtime scaffolding that is being hardened in Phase 7:

- Session History: search, provider filtering, resume, delete, rename, JSON export, and conservative pruning.
- Settings: models with config-defined provider profiles surfaced in the desktop catalog, compact provider metadata rendering, current Kimi/GLM coding defaults, custom provider profile templates and add/edit/delete, no-auth local provider support, provider-aware start readiness with dated cached evidence checks and targeted Settings recovery, manual provider compatibility probes with persisted redacted evidence and summary, live/static model catalog refresh including native Anthropic and Anthropic-compatible `/v1/models`, native Gemini `/v1beta/models`, and Ollama `/api/tags` with dated persisted source labeling and model selection/default saving, workspace, tools, memory, data, diagnostics, scheduler, and general service/autostart surfaces.
- Diagnostics: doctor checks, gateway runtime status, recent trigger-run executor/failure/lease evidence, current session health alerts that ignore idle/completed turns, repair actions, trigger/session queues, and runtime loop visibility.
- Permissions: per-tool allow, deny, reset, default policy states, visible Composer permission modes for Manual Confirm, Trust Project, and Full Access, runtime workspace-scoped inheritance in the same project, pending same-workspace confirmation takeover when enabling broader access, confirmation cards that show the affected workspace/action for write, edit, shell, and MCP operations, replayable confirmation response events for approved/declined history state, and ask-user cards that state the current continue/cancel response limit. Full Access skips routine confirmation prompts but keeps explicit deny rules, external-write blocks, and catastrophic shell blocks.
- Review and background work: Agent Workbench review queue/history, severity-calibrated `/code-review` guidance, derived A2A parent/child lineage badges, completion/review-to-commit eligibility facts, plus a global background status bar and task list.
- Acceptance: browser smoke coverage for resume, diagnostics, provider probes/model catalog refresh, static fallback catalogs, selection/default saving, compact provider metadata rendering, provider evidence start readiness, custom provider profile templates/add/edit/delete, permissions including confirmation replay, Trust Project, and Composer Full Access evidence, scheduler, A2A review, background task UI, current health alerts, stale-banner scoping, and preview ownership details, plus an acceptance matrix contract gate, runtime ownership gates for mocked restart evidence, provider usage, transcript usage hydration, state consistency map status, post-shell file effects, persisted A2A lineage, A2A child runtime events/capsules, A2A review gate V2/recovery suggestions, review-to-commit eligibility, runtime journal authority/recovery, gated headless policy/approval checks, and the real Rust `run_worktree_worker` harness.
- Phase 8 desktop UI evidence helpers now surface normalized recovery commands and `permissionScope` from preflight through the disposable loop status/runbook summaries, including nested `not_checked` summaries when UI preflight is skipped, make clear that Forge Trust/Full Access does not grant macOS Screen Recording or Accessibility, have preflight/status/runbook/doctor recovery paths point back to the strict preflight and `--require-live-ready` hard gate after local permission recovery or skipped UI checks, add a doctor `--run-checks` command that reruns both gates after permission recovery, expose shared `liveReadyGate.pass/reason` diagnostics for the hard gate in status/runbook output, the acceptance matrix runs `--require-live-ready` as a hard gate that requires a checked UI preflight, and archived rows require validation/evidence/markdown sidecars before they are treated as complete.

## Level 3 Runtime Evidence

Forge Level 3 Runtime backs long-running agent work with an append-only loop event journal, rebuildable projections, durable human gates, policy and budget preflight, typed completion evidence, crash/replay regression coverage, gateway runner leases, and gateway session-host run evidence.

The current acceptance suite advertises and runs those runtime gates before the desktop smoke tests, so product claims about durable loop state are backed by replay, policy, gate, completion, review-to-commit eligibility, gateway status, gateway session-host run evidence, backend restart-smoke dry-run coverage, subagent projection, mocked desktop restart evidence as partial macOS proof, an explicit desktop restart harness preflight plus contract and blocker-documentation gates that report the current macOS official WKWebView WebDriver support gap and require a desktop restart harness launch command before non-macOS can claim official readiness, provider usage known/unknown telemetry, transcript usage hydration, state consistency map status, bounded post-shell file-effect evidence, persisted A2A lineage, gated headless ownership policy/approval checks, A2A child runtime file-IO facts, direct ToolExecutor file-IO stream smoke coverage, and a real Rust `run_worktree_worker` harness using the mock adapter/harness.

Eval-runner now also includes prepared-turn evidence scoring for prompt/context-source quality, file effects evidence scoring for changed-file duplicates, trace/evidence alignment, and file diff completeness when ForgeRunEvidence file effects are present, tool/shell evidence scoring for replay identity, command/tool facts, exit-code consistency, trace alignment, and secret-like output leakage, usage unknown conflict scoring for explicit unknown reasons and non-invented token/cost values, provider usage value validation for malformed token/cost facts, failure evidence scoring for category/reason alignment, continuity lessons scoring for formed lesson metadata, plus memory recall audit scoring for candidate reason and token-budget evidence.

`run_gateway_read_only_owner_diagnostics` is the first gated gateway read-only diagnostics owner slice. It requires explicit approval or a dev-only flag, records replayable requested/lease/completed owner evidence, returns `gateway_can_resume=true` only for that read-only diagnostics result, and keeps provider/tool/shell/file/confirmation/commit side effects false. Operators can invoke the same slice with `forge_trigger read-only-owner-diagnostics --task-id <id> --approved-by <name>` or the dev-only local flag.

`forge_trigger ownership-eligibility --mode gateway_patch_proposal_owner` now exercises the gateway patch proposal owner gate. It reports proposal-only patch generation intent plus required review/diff evidence, while `would_apply_patch=false`, `would_write_files=false`, and direct-write gateway ownership remains blocked by default.

Forge separates model usage from context-window status. Provider usage rows show the model call's reported tokens/cost, while the Composer context indicator shows estimated context used and true remaining context; `turn_prepared` now gives that indicator a backend pre-dispatch estimate plus recall audit metadata and context budget buckets for visible input, hidden system context, memory, files, project records, compacted transcript, connector context, and reserved output without hidden memory bodies before provider usage later reconciles the count, and the auto-compact threshold distance stays in the tooltip so it is not confused with provider context remaining. The local session snapshot persists cumulative cost alongside the usage ledger so reloads do not reset visible cost when older provider usage blocks are unavailable; Tauri transcript replay also recovers legacy `usage` events that predate `provider_usage`, replayed provider usage refreshes the Composer context label over stale persisted metadata, and restored compacted-context blocks keep the Composer label on the post-compact local estimate even when older session metadata still contains the pre-compaction count.

The 4C.4 fake headless owner executor fixture is currently backed by focused runner, journal, projection, and replay tests. It proves runner-only orchestration state chains for completed, pending-confirmation, pending-tool-call, interrupted, cancelled, expired, and stale pending-view idempotency paths through the same journal/projection/envelope path; it is not a new acceptance-matrix or e2e autonomous resume gate.

The live file-IO evidence covers successful direct executor file-ish tools (`read`, `write`, `edit`, `diff`, `list`, and `search`), A2A child file-ish tool facts, and bounded post-shell worktree deltas with `source: "post_shell_delta"`. Boundary language stays explicit: commit remains human-gated; shell-internal tracing is not claimed; unknown provider token/cost remains unknown when adapters omit usage; gateway autonomous resume requires explicit policy and human approval. Gateway trigger runs now keep explicit executor, retry/dead-letter, failure-category, and restart-smoke evidence. This proves backend-visible ownership and persistence, but it still does not claim unattended autonomous continuation, auto commit/merge/push, or official Tauri/WebDriver force-quit recovery. The current headless gate records and replays approval intent, safe coordinator dry-run facts, and a test-only fake executor fixture, and Forge still does not claim Tauri/WebDriver force-quit coverage, syscall/file-descriptor tracing, full non-git workspace enumeration, billing-grade usage accounting, exact cost when usage/pricing is unknown, automatic creation of parent-session context, fuzzy parent/root-task selection, real headless `AgentSession` execution, model/tool/file side effects, `gateway_can_resume=true`, pending confirmation/tool auto-acceptance, or automatic commit/merge/push behavior.

The desktop loop task panel also displays derived headless readiness and lease-pending status for waiting tasks; `gateway_can_resume` remains false, and commit/merge/push stays human-gated.

## Product Tracking

Roadmap items live in the private GitHub Project:

https://github.com/users/Cabbos/projects/3
