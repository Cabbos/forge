# Forge Eval Backtest Runbook

Forge uses the sibling `forge-eval-runner` project as the eval harness. Forge itself only provides the headless agent command; the runner owns fixtures, validation commands, scope checks, trace artifacts, metrics, and reports.

## Prerequisites

- `forge-eval-runner` lives next to this repo at `../forge-eval-runner`, or set `FORGE_EVAL_RUNNER_PATH`.
- Run commands from this Forge repo.
- Real Forge runs require a configured provider API key in Forge settings or environment.

## Commands

Preview the resolved runner path, selected cases, output path, and headless command:

```bash
npm run eval:forge:dry-run
```

Run the five real-session cases through the deterministic mock provider. This proves the case loader, aggregation, reporting, and artifact output without spending model calls:

```bash
npm run eval:forge:mock
```

Run the same five `forge-session-*` cases through the real Forge headless binary:

```bash
npm run eval:forge
```

Run one smoke case before spending time on the full suite:

```bash
npm run eval:forge -- --suite smoke
```

Run a specific real-session case:

```bash
npm run eval:forge -- --case forge-session-normalize-input
```

## Continuity Pipeline Eval

Continuity evals reuse the same sibling runner, but they use `continuity-pipeline-*` cases. These cases keep Forge headless focused on doing the task, then run post-validation commands in the disposable workspace:

- `npm test`
- `npx tsc --noEmit`
- `python3 scripts/assert-continuity.py ...`

Preview the Continuity suite:

```bash
npm run eval:continuity:dry-run
```

Run one real smoke case:

```bash
npm run eval:continuity:smoke
```

Run all Continuity pipeline cases:

```bash
npm run eval:continuity
```

The SQLite assertion checks that `.forge/continuity.db` exists, required event types were recorded, reflections were marked formed, FTS rows match experiences, prompt-echo dirty candidates are absent, successful shell output was not stored as an error, and Evidence file paths are deduplicated.

## Defaults

`npm run eval:forge` defaults to:

```text
runner: ../forge-eval-runner
suite: forge-session
provider: forge
model: local-forge
output: artifacts/eval-runs/{timestamp}-forge-session-forge.json
```

The default headless command is:

```bash
cargo run --manifest-path ./src-tauri/Cargo.toml --bin forge_eval_agent --quiet
```

Override it with `FORGE_EVAL_FORGE_AGENT_COMMAND` when you need a prebuilt binary or custom model/provider environment.

## What Counts As Effective

The report should be read at two levels:

- `success_rate`, `verification_pass_rate`, and `scope_violation_rate` say whether Forge completed the task and stayed inside the declared file boundary.
- Per-task trace summaries show changed files, model rounds, confirmation requests, failure categories, and failure reasons.

For a healthy local regression pass, the mock suite should be `success_rate=1.0`. Real Forge runs can fail because of model behavior, provider errors, or verification-command selection; those should still produce a valid trace artifact instead of a runner crash.

Forge headless also runs the eval case validation command inside the temporary workspace. If validation fails, Forge sends the validation output back into the same `AgentSession` once so the agent can make a minimal repair before returning its trace. The external eval runner still performs the final validation and remains the source of truth for pass/fail scoring.

## Artifacts

Backtest artifacts are written under:

```text
artifacts/eval-runs/
```

This directory is ignored by git and blocked by the pre-commit hook. Keep useful JSON reports locally as evidence, but do not commit them.
