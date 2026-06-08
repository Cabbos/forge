# Forge Eval Runner

Forge Eval Runner is a Python MVP for evaluating coding agents. It focuses on the parts hiring teams usually care about for AI Product Engineer, LLM Application Engineer, and Agent Engineer roles: trace capture, task-level metrics, failure analysis, typed API contracts, tests, and containerized delivery.

The default version still uses a deterministic mock runner for reproducible tests. It now also has a `provider=forge` runner seam for connecting a real Forge headless command without changing the API shape. Both paths produce the same trace contract: prompts, context files, raw events, tool calls, shell output, file diffs, changed files, scope violations, final answer, verification result, timing, and failure reason.

## Why This Project Exists

Agent products are hard to debug when they only expose a final answer. A useful eval platform needs to answer:

- What did the agent see?
- Which tools did it call?
- What shell commands ran, and what did they return?
- Which files changed?
- Did verification run?
- Why did a task fail?
- How does one run compare with another at the metrics level?

Forge Eval Runner is a portfolio-oriented implementation of that workflow. It demonstrates how to turn an agent execution into structured data that can power dashboards, trace replay, regression analysis, and model/provider comparison.

## Architecture

```text
tasks/sample_tasks.json
        |
        v
InMemoryStorage ----> FastAPI routes
        |                  |
        v                  v
RunnerFactory -> DeterministicMockRunner / ForgeAgentRunner -> AgentTrace[]
        |                  |
        v                  v
VerificationResult    calculate_metrics()
```

## Project Structure

```text
forge-eval-runner/
  app/
    cases.py      JSON eval case loader for files or directories
    cli.py        Minimal backtest CLI for local/offline runs
    main.py       FastAPI app and route handlers
    models.py     Pydantic task, trace, run, and metrics schemas
    runner.py     Deterministic mock coding-agent runner
    trace.py      Trace timestamp and mock diff helpers
    metrics.py    Success, coverage, duration, and failure aggregation
    reporting.py  Backtest report aggregation and per-task summaries
    storage.py    Memory and SQLite task/run/artifact storage boundary
    config.py     Environment-driven settings
  eval_cases/
    */case.json   Portable eval case definitions plus disposable fixtures
  tasks/
    sample_tasks.json
  tests/
    test_api.py
    test_metrics.py
    test_runner.py
```

## API

- `GET /health`
- `GET /tasks`
- `POST /runs`
- `GET /runs`
- `GET /runs/{run_id}`
- `GET /runs/{run_id}/trace`
- `GET /runs/{run_id}/metrics`
- `GET /runs/{run_id}/report`
- `GET /runs/{run_id}/artifacts`

## Run Locally

```bash
cd apps/eval-runner
uv sync
uv run uvicorn app.main:app --reload --port 8000
```

Open:

- API docs: `http://localhost:8000/docs`
- Health check: `http://localhost:8000/health`

## SQLite Persistence

The API still defaults to in-memory storage for local smoke tests. V0.2 adds a SQLite-backed storage option for durable run, task, and artifact metadata:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

SQLite creates three metadata tables:

- `eval_runs`: run status, requested task ids, metrics summary, report rates, failure category counts.
- `eval_run_tasks`: per-task pass/fail summary, duration, model rounds, confirm requests, changed files, scope violations.
- `eval_artifacts`: artifact kind, path, size, and created timestamp.

Large trace/report payloads are written to filesystem artifacts instead of database rows:

```text
artifacts/
  {run_id}/
    trace.json
    report.json
```

This preserves the current synchronous API contract while making completed runs, trace/report artifacts, and run lists queryable after the storage object or process restarts.

## Worker Mode

V0.3 adds a small local worker while keeping synchronous execution as the default. To make `POST /runs` return immediately with a `pending` run, start the API with queued execution:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_RUN_EXECUTION_MODE=queued \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

Run one pending job:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker --once
```

Or poll continuously:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker
```

The worker claims the oldest `pending` run, marks it `running`, executes each task through the existing runner boundary, writes per-task summaries, stores trace/report artifacts, and marks the run `completed`.

## Run Eval Cases With CLI

The repository includes five portable cases under `eval_cases/`. The default mock provider runs fully offline and is intended to validate case loading, trace shaping, scope checks, and report aggregation:

```bash
uv run python -m app.cli --cases eval_cases --provider mock
```

The CLI prints a JSON backtest report:

```json
{
  "total_tasks": 5,
  "success_rate": 0.4,
  "verification_pass_rate": 0.6,
  "scope_violation_rate": 0.2,
  "avg_duration_ms": 564.0,
  "avg_model_rounds": 2.0,
  "avg_confirm_requests": 0.6,
  "failure_categories": {
    "scope_violation": 1,
    "timeout": 1,
    "verification_failed": 1
  },
  "tasks": []
}
```

Use the same command shape for the Forge contract once `FORGE_EVAL_FORGE_AGENT_COMMAND` points to the Forge headless binary:

```bash
FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path ../desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet" \
  uv run python -m app.cli --cases eval_cases/small-edit-success --provider forge --model local-forge
```

The Forge command reads the eval payload from stdin and writes one trace JSON object to stdout. The current headless default is DeepSeek `deepseek-v4-flash`; override it with `FORGE_HEADLESS_PROVIDER` or `FORGE_HEADLESS_MODEL` when needed.

When debugging a real agent run, add `--output` to keep the full trace artifact
while stdout remains the compact report:

```bash
FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path ../desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet" \
  uv run python -m app.cli \
    --cases eval_cases/small-edit-success \
    --provider forge \
    --model local-forge \
    --output output/small-edit-success.trace.json
```

The output file contains:

```json
{
  "report": {},
  "traces": []
}
```

## Three Ways to Run

The eval runner supports three execution modes, from fastest/most deterministic to slowest/most realistic:

### a. Mock Offline Backtest

Fully deterministic, no API keys, no network. Validates case loading, trace shaping, scope checks, and report aggregation:

```bash
uv run python -m app.cli --cases eval_cases --provider mock
```

### b. Forge Headless Local Real Backtest

Runs a real Forge agent through the headless `forge_eval_agent` binary. Requires a valid API key and Rust toolchain:

```bash
# From the monorepo root
npm run eval:forge:smoke:dry-run

# Or run the real backtest
npm run eval:forge:smoke
```

The `run-forge-backtest.mjs` script automatically resolves the eval-runner sibling directory and generates the correct `FORGE_EVAL_FORGE_AGENT_COMMAND` pointing to `apps/desktop/src-tauri/Cargo.toml`.

### c. Queued Worker + SQLite Service Mode

Start the FastAPI service with SQLite persistence and queued execution, then poll with the worker:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_RUN_EXECUTION_MODE=queued \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

Run one pending job:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker --once
```

## Required Environment Variables

| Variable | Required For | Description |
|---|---|---|
| `FORGE_EVAL_FORGE_AGENT_COMMAND` | `provider=forge` | Command to launch the Forge headless agent. Defaults to `cargo run --manifest-path ../desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet` when run from `apps/desktop/`. |
| `FORGE_HEADLESS_PROVIDER` | `provider=forge` | LLM provider for the headless agent (e.g. `anthropic`, `openai`, `deepseek`). Defaults to `deepseek`. |
| `FORGE_HEADLESS_MODEL` | `provider=forge` | Model ID for the headless agent. Defaults to `deepseek-v4-flash`. |
| `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DEEPSEEK_API_KEY` | `provider=forge` | API key for the chosen provider. Stored in `~/.forge/config.json` via Forge settings. |
| `FORGE_EVAL_RUNNER_PATH` | Optional | Override the default eval-runner directory. Defaults to the sibling `eval-runner` of `apps/desktop/`. |

## Example Requests

List tasks:

```bash
curl http://localhost:8000/tasks
```

Create an eval run:

```bash
curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run", "parser-regression-failure"],
    "provider": "mock",
    "model": "deterministic-agent-v1"
  }'
```

Fetch trace:

```bash
curl http://localhost:8000/runs/<run_id>/trace
```

List runs:

```bash
curl http://localhost:8000/runs
```

Fetch metrics:

```bash
curl http://localhost:8000/runs/<run_id>/metrics
```

Fetch the backtest report:

```bash
curl http://localhost:8000/runs/<run_id>/report
```

Fetch artifact metadata:

```bash
curl http://localhost:8000/runs/<run_id>/artifacts
```

## Eval Case Format

Cases are dependency-free JSON files. A directory case uses `eval_cases/<case-id>/case.json` with fixture paths resolved relative to that file:

```json
{
  "schema_version": 1,
  "task": {
    "id": "small-edit-success",
    "title": "Small focused edit succeeds",
    "prompt": "Update src/calculator.py and run validation.",
    "fixture_path": "fixture",
    "context_files": ["src/calculator.py", "tests/test_calculator.py"],
    "validation_commands": ["python -m pytest tests/test_calculator.py"],
    "verification_command": "python -m pytest tests/test_calculator.py",
    "expected_files_changed": ["src/calculator.py"],
    "forbidden_files_changed": [".env"],
    "metadata": {
      "mock": {
        "changed_files": ["src/calculator.py"],
        "model_rounds": 2,
        "confirm_requests": 0
      }
    }
  }
}
```

`metadata.mock` is only used by the deterministic mock provider. It lets offline cases simulate changed files, raw events, tool commands, model rounds, confirmation requests, token counts, and failure categories without depending on a live Forge app.

## Running Against Forge

Set `FORGE_EVAL_FORGE_AGENT_COMMAND` to the Forge headless binary, then create a run with `provider: "forge"`:

```bash
export FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path ../desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet"

curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run"],
    "provider": "forge",
    "model": "local-forge"
  }'
```

The command receives JSON on stdin:

```json
{
  "task": {
    "id": "python-cli-dry-run",
    "prompt": "Add a --dry-run flag...",
    "context_files": ["src/cli.py"],
    "expected_files_changed": ["src/cli.py"],
    "forbidden_files_changed": [".env"]
  },
  "prompt": "Add a --dry-run flag...",
  "provider": "forge",
  "model": "local-forge",
  "workspace_path": "/tmp/forge-eval-.../workspace"
}
```

It should write a JSON object to stdout. The runner maps it into `AgentTrace`:

```json
{
  "raw_events": [{ "event_type": "tool_call_start", "tool_name": "read_file" }],
  "tool_calls": [{ "command": "read_file src/cli.py", "stdout": "loaded", "exit_code": 0 }],
  "shell_outputs": [{ "command": "pytest", "stdout": "1 passed", "exit_code": 0 }],
  "file_diffs": [{ "path": "src/cli.py", "change_type": "modified", "diff": "diff --git ..." }],
  "changed_files": ["src/cli.py"],
  "verification_result": {
    "command": "pytest",
    "passed": true,
    "exit_code": 0
  },
  "final_answer": "Completed.",
  "model_rounds": 2,
  "confirm_requests": 1,
  "input_tokens": 120,
  "output_tokens": 40
}
```

Scope checks are automatic: files in `forbidden_files_changed`, or files outside `expected_files_changed` when that list is provided, mark the trace as `scope_violation` even if verification passed.

For stronger backtests, put independent judge commands in `validation_commands`. These run inside the disposable workspace after Forge finishes and override any `verification_result` returned by the external command.

## Run With Docker

```bash
cd apps/eval-runner
docker compose up --build
```

The service listens on `http://localhost:8000`.

## Test And Lint

```bash
uv run pytest
uv run ruff check .
uv run ruff format --check .
```

## Portfolio Value

This project maps directly to real agent-platform work:

- Agent eval platform: task ingestion, run creation, per-task pass/fail, aggregate metrics.
- Trace viewer backend: structured trace schema for tool calls, shell outputs, diffs, final answer, and verification.
- Debug analysis: failure categories and failure reasons are first-class fields, not log text.
- Engineering delivery: FastAPI service, Pydantic contracts, pytest coverage, ruff linting, Dockerfile, and docker-compose.
- Extensibility: the mock runner can later be replaced by a real Codex/OpenAI/DeepSeek-compatible adapter without changing the API shape.

## Next Iterations

- Persist runs to SQLite or Postgres.
- Add a frontend trace viewer with timeline and diff panels.
- Wire `ForgeAgentRunner` to Forge's real headless/Tauri backend entry point.
- Execute verification commands in an isolated container.
- Compare multiple models/providers across the same task set.
- Export run reports as JSON, Markdown, or HTML.

## Portfolio Demo Assets

项目包含可截图、可展示的作品集素材：

| 文件 | 说明 |
|---|---|
| `docs/api-examples.md` | API 用法和 curl 示例，适合截图 |
| `docs/portfolio-demo-guide.md` | 2 分钟演示脚本和面试讲解路径 |
| `docs/architecture.md` | Mermaid 架构图（系统、trace、metrics、Forge 关系） |
| `examples/sample-run-request.json` | 创建 run 的请求体示例 |
| `examples/sample-run-response.json` | 完整 run 响应（含 traces + metrics） |
| `examples/sample-trace-response.json` | trace 响应示例 |
| `examples/sample-metrics-response.json` | metrics 响应示例 |
| `scripts/capture_demo_assets.sh` | 一键刷新所有示例 JSON |

### How to Refresh Sample Assets

```bash
# 1. 启动服务
uv run uvicorn app.main:app --port 8000

# 2. 运行采集脚本
bash scripts/capture_demo_assets.sh
```

### 截图顺序建议

1. 浏览器访问 `http://localhost:8000/docs` — OpenAPI 文档页
2. 终端 `curl http://localhost:8000/tasks` — 任务列表
3. 终端 `curl -X POST http://localhost:8000/runs ...` — 创建 run
4. 终端 `curl http://localhost:8000/runs/{id}/metrics` — metrics 汇总
5. 终端 `curl http://localhost:8000/runs/{id}/trace | python3 -m json.tool` — trace 详情
