# Forge Eval Runner Team Service Design

## Purpose

Turn `forge-eval-runner` from a local CLI/API demo into a small deployable backend service for team Agent backtesting.

The service should let a teammate start an eval run, let a background worker execute Forge headless tasks, persist run state, and return trace/report artifacts after completion. This is an eval control plane, not a replacement for Forge itself.

## Non-Goals

- Do not build a full SaaS platform.
- Do not add multi-tenant billing, organization management, or a complex UI.
- Do not require Celery, Redis, Kubernetes, or S3 in the first deployable version.
- Do not store large trace JSON directly in database rows.
- Do not make API requests wait until a full backtest finishes.

## Recommended Approach

Build V1 as a lightweight FastAPI service with SQLite persistence, a local worker loop, and filesystem artifacts.

This gives the project a standard backend shape while keeping it simple enough to understand:

- FastAPI receives requests and returns run state.
- SQLite stores durable metadata and status.
- Worker executes pending runs outside the request path.
- `artifacts/` stores full trace/report JSON.
- Docker Compose starts the API and worker against the same database/artifact volume.

Postgres can be added later without changing the API contract.

## Architecture

```text
Client / Forge UI
        |
        v
FastAPI API
  POST /runs
  GET /runs/{id}
  GET /runs/{id}/report
  GET /runs/{id}/trace
  POST /runs/{id}/cancel
        |
        v
SQLite database
        |
        v
Worker loop
        |
        v
ForgeAgentRunner -> Forge headless command -> AgentTrace
        |
        v
artifacts/{run_id}/report.json
artifacts/{run_id}/trace.json
```

## Service Boundaries

### API Layer

The API should be fast and predictable. `POST /runs` creates a run and returns immediately with `status=pending`.

Responsibilities:

- Validate request payloads with Pydantic.
- Create run records.
- Return run status, metrics, report, and trace metadata.
- Mark a run as cancel requested.
- Avoid long-running Forge execution inside HTTP handlers.

### Worker Layer

The worker owns real execution.

Responsibilities:

- Poll for `pending` runs.
- Atomically claim one run and mark it `running`.
- Load eval cases.
- Execute each task through the existing runner boundary.
- Persist per-task results.
- Write trace/report artifacts.
- Mark run `completed`, `failed`, `cancelled`, or `timeout`.

The first version can be a simple Python process:

```bash
uv run python -m app.worker
```

### Storage Layer

The database stores durable metadata. Large JSON payloads stay in artifact files.

SQLite is enough for V1 because the service is mostly IO-bound. The bottleneck is LLM calls, shell commands, package installs, and verification tests, not Python request throughput.

## Data Model

### eval_runs

One row per backtest run.

Fields:

- `id`
- `status`: `pending`, `running`, `completed`, `failed`, `cancelled`, `timeout`
- `provider`: `mock` or `forge`
- `model`
- `case_source`
- `success_rate`
- `verification_pass_rate`
- `scope_violation_rate`
- `failure_categories_json`
- `created_at`
- `started_at`
- `finished_at`
- `cancel_requested_at`
- `error`
- `failure_reason`

### eval_run_tasks

One row per case inside a run.

Fields:

- `id`
- `run_id`
- `task_id`
- `status`
- `passed`
- `verification_passed`
- `duration_ms`
- `model_rounds`
- `confirm_requests`
- `changed_files_json`
- `scope_violations_json`
- `failure_category`
- `failure_reason`

### eval_artifacts

One row per generated artifact.

Fields:

- `id`
- `run_id`
- `kind`: `report`, `trace`, `stdout`, `stderr`
- `path`
- `size_bytes`
- `created_at`

## Run Lifecycle

```text
pending
  -> running
  -> completed
  -> failed
  -> cancelled
  -> timeout
```

Rules:

- `POST /runs` only creates `pending`.
- Worker is the only component that can move `pending` to `running`.
- Worker writes task rows incrementally so partial failures are inspectable.
- Final report is built from persisted traces/results.
- Cancellation is cooperative: API sets `cancel_requested_at`; worker checks between tasks and before starting a new Forge process.

## Artifact Strategy

Keep artifacts in the filesystem for V1:

```text
artifacts/
  {run_id}/
    report.json
    trace.json
```

The DB stores artifact metadata and paths. This keeps SQLite small and makes artifacts easy to inspect locally.

Later, the same abstraction can point to S3 or another object store.

## Deployment

V1 Docker Compose should run:

- `api`: `uvicorn app.main:app --host 0.0.0.0 --port 8000`
- `worker`: `python -m app.worker`
- shared volume for `forge_eval.db`
- shared volume for `artifacts/`

SQLite is acceptable for a small internal deployment when API and worker share a volume and write concurrency is low.

For a larger team or multiple workers, migrate to Postgres.

## Authentication

V1 can use a simple static API token:

- `Authorization: Bearer <token>`
- token loaded from environment variable
- `/health` can remain public

This is enough for an internal service and avoids premature user management.

## Error Handling

Expected failure categories:

- `verification_failed`
- `scope_violation`
- `tool_error`
- `timeout`
- `runner_error`
- `cancelled`

Every failed run or task should have:

- machine-readable `failure_category`
- human-readable `failure_reason`
- enough artifact evidence to debug

## Testing

Minimum tests:

- Storage creates and updates runs.
- Worker claims only pending runs.
- Worker persists task results and artifacts.
- API returns pending immediately after `POST /runs`.
- API returns completed report after worker finishes.
- Cancellation marks the run and stops before next task.
- Existing CLI/report tests still pass.

## Migration Path

### V0.2 Persistence

- Add SQLite storage.
- Add run/task/artifact tables.
- Keep current synchronous API behavior where needed for compatibility.

### V0.3 Worker

- Add `app.worker`.
- Make `POST /runs` asynchronous.
- Persist report and trace artifacts.

### V0.4 Team Deploy

- Add Docker Compose API + worker split.
- Add static token auth.
- Add run cancellation.

### V0.5 Postgres Ready

- Add DB URL config.
- Keep SQLite default for local dev.
- Support Postgres for team deployment.

## Success Criteria

The design is successful when a teammate can:

1. Start the service with Docker Compose.
2. Submit a Forge eval run through HTTP.
3. See `pending` immediately.
4. Watch it move to `running`.
5. Fetch a final report after completion.
6. Open the stored trace artifact.
7. Re-run the same cases and compare metrics.

## Recommended Next Implementation Plan

Implement V0.2 first:

- Define SQLModel or SQLAlchemy models.
- Add SQLite-backed storage.
- Preserve the current in-memory storage tests by adding a storage contract test suite.
- Add persistence tests before changing API behavior.

Do not implement worker and Docker split until persistence is stable.
