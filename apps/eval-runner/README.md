# Forge Eval Runner

Forge Eval Runner is a Python MVP for evaluating coding agents. It focuses on the parts hiring teams usually care about for AI Product Engineer, LLM Application Engineer, and Agent Engineer roles: trace capture, task-level metrics, failure analysis, typed API contracts, tests, and containerized delivery.

The first version uses a deterministic mock runner instead of calling a real model. That keeps the project reproducible while preserving the same trace shape a real agent platform would need: prompts, context files, tool calls, shell output, file diffs, final answer, verification result, timing, and failure reason.

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
DeterministicMockRunner -> AgentTrace[]
        |                  |
        v                  v
VerificationResult    calculate_metrics()
```

## Project Structure

```text
forge-eval-runner/
  app/
    main.py       FastAPI app and route handlers
    models.py     Pydantic task, trace, run, and metrics schemas
    runner.py     Deterministic mock coding-agent runner
    trace.py      Trace timestamp and mock diff helpers
    metrics.py    Success, coverage, duration, and failure aggregation
    storage.py    In-memory task/run storage boundary
    config.py     Environment-driven settings
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
- `GET /runs/{run_id}`
- `GET /runs/{run_id}/trace`
- `GET /runs/{run_id}/metrics`

## Run Locally

```bash
cd forge-eval-runner
uv sync
uv run uvicorn app.main:app --reload --port 8000
```

Open:

- API docs: `http://localhost:8000/docs`
- Health check: `http://localhost:8000/health`

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

Fetch metrics:

```bash
curl http://localhost:8000/runs/<run_id>/metrics
```

## Run With Docker

```bash
cd forge-eval-runner
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
- Run real agent providers behind a common adapter interface.
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
