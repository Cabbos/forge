# Forge Bun CLI Design Spec

**Date**: 2026-06-05
**Status**: Ready for review
**Scope**: v0 thin CLI for scriptable Forge runs, evals, and trace export

---

## Overview

Forge should have a CLI version, but the first version should be a thin TypeScript/Bun entrypoint rather than a second agent product. Forge Desktop remains the visual Agent Workbench. Forge CLI becomes the scriptable engineering interface for running tasks, backtests, diagnostics, and trace export.

The CLI should reuse the existing Rust headless boundary:

```text
Bun CLI -> Forge headless binary -> eval_headless::run_stdin_json
        -> eval_headless::run_request -> AgentSession -> ToolExecutor
        -> JSON / NDJSON output
```

This keeps the strongest TS/Bun work visible while preserving the existing Forge engine, provider adapters, permission behavior, memory path, and eval contract.

## Goals

- Create a Bun + TypeScript CLI that is easy to run locally and can later compile into a standalone binary.
- Reuse existing Forge headless execution instead of reimplementing the agent loop.
- Make `run`, `eval`, `trace`, and `doctor` the first command family.
- Preserve Forge Desktop as the primary visual product surface.
- Strengthen the portfolio story: TypeScript CLI orchestration plus Forge Agent Workbench plus Agent Eval/Trace/Metrics.

## Non-Goals

- Do not replace the Tauri desktop app.
- Do not rewrite `AgentSession`, provider adapters, `ToolExecutor`, memory, continuity, or permission gates in TypeScript.
- Do not introduce a second durable session model in the CLI.
- Do not make the first version depend on a packaged desktop app being installed.
- Do not broaden Forge product UI scope as part of this CLI work.

## Approach Options

### Option A: Bun CLI Calling Existing Headless Binary

The CLI parses arguments, resolves config, spawns the Forge headless binary, passes JSON over stdin, and renders JSON or NDJSON results.

Pros:
- Lowest implementation risk.
- Reuses existing Rust engine and eval path.
- Clear TypeScript ownership boundary.
- Good first portfolio artifact.

Cons:
- Requires Rust binary availability during early development.
- Output is limited by current headless response shape.

### Option B: Bun CLI Reimplementing Agent Execution

The CLI directly implements provider adapters, tool execution, memory, permission prompts, and result rendering in TypeScript.

Pros:
- Pure TypeScript runtime.
- More control over CLI UX.

Cons:
- Duplicates Forge internals.
- Creates drift between Desktop and CLI.
- High risk and weakens the existing architecture story.

### Option C: Rust Native CLI First

Add a Rust CLI binary that directly calls `eval_headless::run_request`, then optionally wrap it with TS later.

Pros:
- Closest to the engine.
- Fewer process-boundary details.

Cons:
- Less aligned with the user's TypeScript strength.
- Lower value as a TS/Bun engineering artifact.
- Easy to become engine refactoring work.

## Recommendation

Use Option A for v0. The CLI should be written in Bun + TypeScript and call the existing Rust headless binary. This gives Forge a scriptable interface without fracturing the product. It also makes the user's TypeScript strength visible while keeping Rust as the stable engine layer.

## Command Surface

### `forge run`

Run one prompt against a workspace.

```bash
forge run "Fix the failing test"
forge run --cwd /path/to/project --provider forge --model local-forge "Summarize the current diff"
forge run --json "Check what changed"
```

Responsibilities:
- Accept prompt text from argv or stdin.
- Resolve workspace path.
- Build the headless request JSON.
- Spawn the Forge headless binary.
- Render final answer, changed files, validation result, and failure reason.

### `forge eval`

Run Forge eval suites through the existing eval-runner path.

```bash
forge eval --suite smoke
forge eval --suite continuity
forge eval --case continuity-pipeline-normalize-input
forge eval --dry-run
```

Responsibilities:
- Replace the current `scripts/run-forge-backtest.mjs` surface with a typed CLI command.
- Preserve suite selection behavior.
- Keep the default Forge agent command compatible with the current Rust headless binary.
- Write output artifacts to the same eval output locations unless explicitly overridden.

### `forge trace`

Inspect or export recent headless run results.

```bash
forge trace --last
forge trace --last --json
forge trace --output trace.md
```

Responsibilities:
- Print a compact task summary.
- Export JSON for tooling.
- Export Markdown for demos and interview storytelling.
- Include prompt, provider, model, changed files, validation status, and final answer when available.

### `forge doctor`

Check local readiness.

```bash
forge doctor
forge doctor --json
```

Responsibilities:
- Check Bun availability.
- Check Forge repo root.
- Check Rust/Cargo headless binary path.
- Check API key presence through the same config assumptions used by headless.
- Check sibling `forge-eval-runner` availability for `forge eval`.

## Package Layout

Recommended first layout:

```text
cli/
  package.json
  bun.lock
  tsconfig.json
  src/
    index.ts
    commands/
      run.ts
      eval.ts
      trace.ts
      doctor.ts
    lib/
      headless.ts
      config.ts
      paths.ts
      output.ts
      spawn.ts
  test/
    run.test.ts
    eval.test.ts
    doctor.test.ts
```

This keeps CLI code separate from the Vite/Tauri frontend package. If the CLI later becomes a publishable package, it can move to `packages/forge-cli` without changing its public command model.

## Data Flow

### `forge run`

```text
argv/stdin prompt
  -> parse command options
  -> resolve workspace
  -> build EvalHeadlessRequest
  -> spawn Forge headless binary
  -> write request JSON to stdin
  -> parse result JSON
  -> render text or JSON output
```

### `forge eval`

```text
eval options
  -> select suite/case files
  -> create temporary suite case file
  -> build eval-runner command
  -> inject FORGE_EVAL_FORGE_AGENT_COMMAND if unset
  -> spawn uv/python eval-runner
  -> stream progress
  -> preserve output artifact
```

## Headless Request Shape

The CLI should initially use the existing headless request contract:

```json
{
  "prompt": "Fix the failing test",
  "provider": "forge",
  "model": "local-forge",
  "workspace_path": "/absolute/path/to/project"
}
```

For eval cases, it can pass the existing `task` payload as the headless layer already supports task-based validation and repair.

## Output Modes

Default human output should be compact:

```text
Forge run completed

Provider: forge
Model: local-forge
Changed files: 2
Validation: passed

Final answer:
...
```

`--json` should print the raw result payload with stable formatting.

Future `--ndjson` can stream event-style output, but v0 can stay with final JSON if the headless binary does not yet expose stable streaming.

## Error Handling

The CLI should map common setup failures into direct messages:

| Failure | User-facing behavior |
|---|---|
| Missing API key | Print provider/model and config path hint |
| Missing Cargo/headless binary | Print exact command the CLI attempted |
| Missing eval-runner | Print expected sibling path and override flag |
| Invalid JSON from headless | Save raw output to a temp file and show the path |
| Validation failure | Show command, exit code, concise stdout/stderr preview |

The CLI should exit non-zero for setup failures, validation failures, and internal parser errors.

## Testing Strategy

Use Bun tests for CLI behavior:

- Argument parsing for each command.
- Headless request construction.
- Spawn plan construction without executing Rust.
- JSON output rendering.
- `doctor` checks with mocked filesystem/process probes.

Use one integration test path when stable:

- Build or run `forge_eval_agent`.
- Send a small fixture prompt/request.
- Assert valid JSON and expected top-level fields.

Existing Forge backend checks remain separate:

```bash
npm run check:backend
npm run eval:forge:dry-run
npm run eval:forge:mock
```

## Implementation Phases

### Phase 1: CLI Skeleton

- Add `cli/` Bun package.
- Add `forge doctor`.
- Add typed command parser.
- Add spawn-plan tests.

### Phase 2: `forge run`

- Add headless request builder.
- Spawn the existing Rust headless command.
- Support text and JSON output.
- Add mocked spawn tests.

### Phase 3: `forge eval`

- Port the behavior from `scripts/run-forge-backtest.mjs`.
- Keep existing npm eval scripts working during transition.
- Add tests equivalent to `scripts/run-forge-backtest.test.mjs`.

### Phase 4: `forge trace`

- Add trace discovery/export from recent run artifacts.
- Support JSON and Markdown export.
- Keep the shape demo-friendly and small.

### Phase 5: Standalone Binary

- Use Bun's compile mode once commands are stable.
- Add a release artifact path for macOS first.
- Decide later whether Desktop should bundle the CLI as a sidecar.

## Success Criteria

v0 is successful when:

- `forge doctor` explains whether the local environment can run Forge headless.
- `forge run "..." --json` can call the existing Rust headless boundary and produce valid JSON.
- `forge eval --suite smoke --dry-run` preserves the current eval planning behavior.
- `forge eval --suite smoke` can run the same path as the existing backtest wrapper.
- Tests cover parser, request building, spawn planning, and output formatting.
- No Forge Desktop UI or agent engine behavior is changed for the CLI skeleton.

## Design Guardrails

- The CLI is an orchestration layer, not a new engine.
- The Rust headless layer remains the source of truth for agent execution.
- TypeScript owns command UX, output formatting, path resolution, and eval orchestration.
- Every new command should be useful both for local work and for explaining Forge in interviews.
- Keep the first release small enough to validate with real tasks before broadening command scope.
