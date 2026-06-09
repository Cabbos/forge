# Forge Eval Runner — 运维手册

## 快速启动

### 1. 本地启动 FastAPI 服务（SQLite + queued 模式）

```bash
cd apps/eval-runner

FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
FORGE_EVAL_RUN_EXECUTION_MODE=queued \
uv run uvicorn app.main:app --port 8000
```

环境变量说明：

| 变量 | 默认值 | 说明 |
|---|---|---|
| `FORGE_EVAL_STORAGE_BACKEND` | `memory` | `memory` 或 `sqlite` |
| `FORGE_EVAL_DB_PATH` | `./forge_eval.db` | SQLite 数据库路径 |
| `FORGE_EVAL_ARTIFACTS_PATH` | `./artifacts` | trace/report 文件落盘路径 |
| `FORGE_EVAL_TASKS_PATH` | `./tasks/sample_tasks.json` | 评测任务定义路径 |
| `FORGE_EVAL_RUN_EXECUTION_MODE` | `sync` | `sync` 或 `queued` |
| `FORGE_EVAL_WORKER_ID` | `local-worker` | Worker 身份标识 |
| `FORGE_EVAL_HEARTBEAT_INTERVAL_SECONDS` | `30` | 心跳间隔 |
| `FORGE_EVAL_POLL_INTERVAL_SECONDS` | `5` | Worker 轮询间隔 |

### 2. 启动 Worker（持续消费 queued runs）

```bash
cd apps/eval-runner

FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker
```

只执行一次：

```bash
uv run python -m app.worker --once
```

Worker 支持 `SIGTERM`/`SIGINT` 优雅停止。

### 3. 跑真实 Forge Smoke 评测

```bash
# 从 monorepo 根目录
npm run eval:forge:smoke:dry-run   # 干跑，确认配置
npm run eval:forge:smoke           # 真实运行（需要 API key）
```

需要 `~/.forge/config.json` 中配置对应 provider 的 API key。

### 4. 清理

```bash
cd apps/eval-runner

# 清理 SQLite 和 artifacts
rm -f forge_eval.db
rm -rf artifacts/

# 清理测试生成的临时文件
rm -rf .pytest_cache/
```

## 常用 API

```bash
# 健康检查
curl http://localhost:8000/health

# 列出任务
curl http://localhost:8000/tasks

# 创建 queued run
curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{"task_ids": ["task-pass"], "provider": "mock"}'

# 列出 runs（支持状态过滤）
curl "http://localhost:8000/runs?status=completed"
curl "http://localhost:8000/runs?status=pending"
curl "http://localhost:8000/runs?status=failed"

# 查看 run 详情（含 failure_reason、worker_id、retry_count）
curl http://localhost:8000/runs/{run_id}

# 查看 trace
curl http://localhost:8000/runs/{run_id}/trace

# 查看 metrics
curl http://localhost:8000/runs/{run_id}/metrics

# 查看 report
curl http://localhost:8000/runs/{run_id}/report

# 查看 artifacts 列表
curl http://localhost:8000/runs/{run_id}/artifacts

# 取消 run
curl -X POST http://localhost:8000/runs/{run_id}/cancel
```

## 测试

```bash
# 全部测试（含 smoke）
uv run pytest

# 排除慢测试
uv run pytest -m "not slow"

# 仅 smoke 测试
uv run pytest -m slow

# Lint
uv run ruff check .
uv run ruff format --check .
```

## 目录结构

```
forge_eval.db          # SQLite 元数据
artifacts/{run_id}/    # trace.json, report.json
```
