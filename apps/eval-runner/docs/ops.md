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

### 队列状态

服务启动后可用 `/queue/status` 查看当前队列概览：

```bash
curl http://localhost:8000/queue/status
```

响应包含各 run status 计数，以及最早的 pending/running run ID，便于判断
worker 是否积压或卡住：

```json
{
  "counts": {"pending": 2, "running": 1, "completed": 10},
  "oldest_pending_run_id": "run-a",
  "oldest_running_run_id": "run-b"
}
```

如果 `oldest_running_run_id` 长时间不变，优先查看对应 run 的
`failure_reason`、worker 心跳、artifact 目录和 worker stderr 摘要；如果用户
取消 run，worker 会保留取消前已经写出的 trace/report/trajectory artifact，方便
继续排障。当前实现使用 claim/heartbeat 元数据避免多个 worker 重复消费同一个
run；stale lease 的第一信号也是 `/queue/status` 中 running 计数和 oldest running
run 持续不变。

### 可信回测操作路径

生产或 release 前不要只看任务是否跑完，要同时看两类状态：

- `execution_status`：任务是否 completed、failed、cancelled、timeout。
- `trust_status`：harness、dataset fingerprint、scorer calibration、red-team gate
  是否足以让分数进入发布决策。

推荐顺序：

1. 先跑 golden harness self-check。若 mock golden case 不能稳定通过，trust gate
   必须以 `harness_untrusted` fail closed。
2. 加载 case 后检查 case quality diagnostics。可执行 case 要有
   verification/validation 命令、expected file 断言和有效 fixture；prompt-only
   或 adapter-contract case 要设置 `metadata.contract_only: true`。
3. 正常 lane 使用 dataset fingerprint 和 immutable experiment snapshot 固化输入：

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --trials 3 \
     --experiment-name local-regression \
     --output output/local-regression.json \
     --min-success-rate 0.1 \
     --max-scope-violation-rate 0.2 \
     --max-total-cost-usd 1.00
   ```

4. 对真实用户表达方式单独跑 prompt mutation：

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --prompt-mutation terse-bug-report \
     --min-success-rate 0.1
   ```

5. red-team lane 单独跑，不混入正常 success rate：

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --red-team-only \
     --max-red-team-failure-rate 0
   ```

6. 查看 `score_summary` 的 layered scorer：functional correctness、scope、
   prompt injection、secret leak、unsafe tool use、future-state leakage、
   `regression_ok`、`bugfix_ok`。LLM-as-judge 或 semantic scorer 未经过
   golden-label calibration 前只能 report-only，不能 gate CI。
7. 对真实 Forge 或外部 agent，检查 adapter metadata、`trajectory_path` 和
   `total_cost_usd`。超预算时用 `--max-total-cost-usd` 阻断。
8. 对比最近一次可信 baseline：

   ```bash
   python - <<'PY'
   import json
   from pathlib import Path
   from app.models import BacktestReport
   from app.report_compare import compare_reports

   old = BacktestReport.model_validate(json.loads(Path("output/baseline.json").read_text())["report"])
   new = BacktestReport.model_validate(json.loads(Path("output/local-regression.json").read_text())["report"])
   print(compare_reports(old, new).model_dump_json(indent=2))
   PY
   ```

### 3. 三种 Smoke 模式

| 命令 | 说明 | 需要 API Key | 执行 `forge_eval_agent` | 适用场景 |
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

Red-team cases live under `apps/eval-runner/eval_cases/red_team` and are filtered
out of normal CLI runs by default so product success rate is not mixed with
adversarial probes. Run them as a separate lane:

```bash
cd apps/eval-runner
uv run python -m app.cli \
  --cases eval_cases \
  --provider mock \
  --red-team-only \
  --max-red-team-failure-rate 0
```

Use `--include-red-team` only when you explicitly want normal and red-team cases
in the same report. The report `score_summary` includes red-team labels such as
`secret_leak_ok`, `prompt_injection_ok`, `scope_escape_ok`,
`future_state_leakage_ok`, and `unsafe_tool_use_ok`.

Cost budgets can be enforced in CI with `--max-total-cost-usd`. The mock runner
can carry `metadata.mock.cost_usd`, while real Forge traces may emit `cost_usd`
directly in the trace payload:

```bash
uv run python -m app.cli \
  --cases eval_cases \
  --provider mock \
  --max-total-cost-usd 1.00
```

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

Forge runner 会在 agent 结束后按顺序执行 `validation_commands`、
`pass_to_pass_commands`、`fail_to_pass_commands`、`post_validation_commands`。
`pass_to_pass_commands` 代表既有行为不能回归，失败时报告
`Regression validation failed`；`fail_to_pass_commands` 代表 bug-fix 测试必须
转绿，失败时报告 `Bug-fix validation failed`。报告的 `score_summary` 会分别给出
`regression_ok` 和 `bugfix_ok`。

`forge_eval_agent` 可以在 stdout 先输出少量日志，再输出最终 trace JSON
object；Python runner 会提取最后一个 JSON object。stdout 中完全没有 JSON、
JSON 不是 object、缺少 `final_answer`、`tool_calls` / `shell_outputs` 等字段类型
不符合 trace contract，都会归类为 `forge_contract_error`。这类失败的
`failure_reason` 会带上异常类型和 stdout/stderr 前 500 字符预览；完整 stdout
和 stderr 仍会保存在 trace 的 `shell_outputs` 里。

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

### 4. 查看 Eval 质量报告

```bash
# 查看最近 10 次运行汇总 + 回归对比
cd apps/desktop
npm run eval:report

# 只查看最新一次（不对比）
npm run eval:report:latest
```

报告输出示例：

```
╔══════════════════════════════════════════════════════════════╗
║           Forge Eval Report                                  ║
╚══════════════════════════════════════════════════════════════╝
Total artifacts on disk: 8

─ 2026-06-09T05-11-25Z  forge-session / forge ─
  success_rate=1.00  verification=1.00  scope_violation=0.00
  avg_model_rounds=24.0  avg_duration=59.9s  tasks=1

⚠️  REGRESSIONS DETECTED
  🔴 success_rate: 1.00 → 0.00 (Δ +1.00)
  🔴 scope_violation_rate: 0.00 → 1.00 (Δ +1.00)
  🟡 avg_model_rounds: 13.00 → 59.00 (Δ +46.00)
```

回归检测规则：
- 🔴 **critical**: `success_rate` 下降 ≥ 0.5 或 `scope_violation_rate` 上升 ≥ 0.5
- 🟡 **warning**: `avg_model_rounds` 暴涨 (>2x)、`avg_duration_ms` 暴涨 (>3x)、新增 failure category

### 5. 从生产 trace 晋升 Eval Case

失败 trace 可以直接转换成可回归的 eval case：

```bash
cd apps/eval-runner
uv run python -m app.cli promote-trace \
  --trace artifacts/{run_id}/trace.json \
  --output eval_cases/promoted
```

`--trace` 支持 API/SQLite 生成的 trace 列表，也支持 CLI backtest artifact 中的
`{"traces": [...]}` 结构。命令只会为失败 trace 写出 case，输出目录形如：

```text
eval_cases/promoted/
  real-user-failure/
    case.json
```

生成的 case 会保留原始 prompt、context files、expected/forbidden file 断言、
verification command，以及 `metadata.source=trace` 和失败原因，便于把线上问题
纳入后续回归集。

晋升后建议先以 mock lane 验证 case shape，再把它加入正常 regression 或
red-team lane。若 trace 只证明了外部 adapter contract，没有可执行断言，请保留
`metadata.contract_only: true`，避免把 contract-only case 当成可验证修复。

### 6. 清理

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
artifacts/{run_id}/    # trace.json, report.json, *.trajectory.json
```
