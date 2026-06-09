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

### 3. 三种 Smoke 模式

| 命令 | 说明 | 需要 API Key | 执行 `forge_eval_agent` |
|---|---|---|---|
| `npm run eval:forge:smoke:dry-run` | 干跑：验证命令规划、case 选择、fixture 路径 | 否 | 否 |
| `npm run eval:forge:mock` | Mock provider：用确定性 mock runner 跑 case，不调用模型 | 否 | 否 |
| `npm run eval:forge:smoke:real` | 真实 Forge provider：实际启动 `forge_eval_agent`，调用真实模型，并用小预算保护 smoke 成本 | **是** | **是** |

#### Dry-run（命令规划验证）

```bash
cd apps/desktop
npm run eval:forge:smoke:dry-run
```

只输出计划（JSON），不执行任何命令。用于验证：
- runnerRoot 指向正确
- case 文件被正确选择
- `FORGE_EVAL_FORGE_AGENT_COMMAND` 生成正确
- fixture 路径解析为绝对路径

#### Mock smoke（无模型成本）

```bash
cd apps/desktop
npm run eval:forge:mock
```

使用 `provider=mock`，由 Python `DeterministicMockRunner` 生成确定性 trace。
- 适合 CI 中验证 eval-runner pipeline 完整性
- 不依赖外部 API，秒级完成
- 验证 report 格式、scope 检查、verification 逻辑

#### Real smoke（真实 Forge provider）

```bash
cd apps/desktop
npm run eval:forge:smoke:real
```

默认 smoke 只跑 `forge-session-capitalize`，并注入 `--timeout 120 --max-model-rounds 20`。如果超过预算，说明真实 agent loop 在这条最小 case 上已经不稳定，需要看 artifact 里的 `failure_category`、`model_rounds`、`scope_violations` 和 raw trace。

实际执行以下完整链路：
1. Bridge script → Python CLI (`app.cli`)
2. `ForgeAgentRunner` 准备 workspace、复制 fixture
3. `ForgeAgentRunner` 调用 `FORGE_EVAL_FORGE_AGENT_COMMAND`
4. `forge_eval_agent`（Rust 二进制）读取 stdin JSON
5. `eval_headless::run_stdin_json` 解析请求、检测 API key
6. 构建 AgentSession，调用真实 LLM API
7. 执行 tool calls、验证命令、修复循环
8. 输出 trace JSON → Python runner 生成 report

需要 `~/.forge/config.json` 中配置对应 provider 的 API key，或设置环境变量：

```json
{
  "api_keys": {
    "deepseek": "sk-...",
    "anthropic": "sk-ant-...",
    "openai": "sk-...",
    "openrouter": "sk-..."
  }
}
```

支持的 env var：`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`、`DEEPSEEK_API_KEY`、`OPENAI_API_KEY`、`OPENROUTER_API_KEY`。

**无 API key 时的行为**：

```bash
npm run eval:forge:smoke:real
# → [forge-backtest] ERROR: No API key found for Forge eval.
# →  Real Forge provider smoke requires a configured API key.
# →  Options:
# →    1. Add a key to ~/.forge/config.json
# →    2. Set an environment variable
# →  To run without a real model, use: npm run eval:forge:mock
# →  To preview the command plan, use: npm run eval:forge:smoke:dry-run
```

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
