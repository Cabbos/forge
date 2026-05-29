# API Examples

这份文档展示 forge-eval-runner 的核心 API 用法，适合截图和面试演示。

## 前置条件

```bash
cd forge-eval-runner
uv sync
uv run uvicorn app.main:app --port 8000
```

服务启动后访问 `http://localhost:8000/docs` 可查看自动生成的 OpenAPI 文档。

---

## 1. Health Check

```bash
curl http://localhost:8000/health
```

响应：

```json
{
  "status": "ok",
  "service": "forge-eval-runner"
}
```

**截图建议**：终端截图，展示服务正常运行。

---

## 2. 列出评测任务

```bash
curl http://localhost:8000/tasks
```

响应（截取一个任务）：

```json
[
  {
    "id": "python-cli-dry-run",
    "title": "Add a dry-run flag to a Python CLI",
    "prompt": "Add a --dry-run flag to the CLI and verify that it avoids writes while still reporting planned changes.",
    "context_files": ["src/cli.py", "tests/test_cli.py"],
    "verification_command": "pytest tests/test_cli.py",
    "expected_success": true,
    "tags": ["python", "cli", "regression-test"],
    "metadata": {}
  }
]
```

**字段说明**：

| 字段 | 含义 |
|---|---|
| `id` | 任务唯一标识 |
| `title` | 任务标题 |
| `prompt` | 给 Agent 的自然语言指令 |
| `context_files` | Agent 执行前应读取的文件 |
| `verification_command` | 验证命令（如 pytest） |
| `expected_success` | 预期是否通过（用于 mock runner 判断） |
| `tags` | 分类标签 |

**截图建议**：浏览器访问 `/docs` 展示 OpenAPI 页面，或终端 curl 输出。

---

## 3. 创建 Eval Run

```bash
curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run", "parser-regression-failure"],
    "provider": "mock",
    "model": "deterministic-agent-v1"
  }'
```

请求体说明：

| 字段 | 说明 |
|---|---|
| `task_ids` | 要执行的任务 ID 列表（不传则执行全部） |
| `provider` | 模拟的 provider 名称 |
| `model` | 模拟的模型名称 |

响应包含完整的 `EvaluationRun` 对象，其中包含：
- `run_id`：本次运行的唯一 ID
- `status`：运行状态（completed / failed）
- `traces`：每个任务的 AgentTrace（详见下一节）
- `metrics`：汇总指标（详见第 5 节）

**截图建议**：终端 curl 输出，或 `/docs` 页面的 POST /runs 交互界面。

---

## 4. 获取 Trace

```bash
curl http://localhost:8000/runs/{run_id}/trace
```

响应是一个 `AgentTrace[]` 数组。每个 trace 包含：

```json
{
  "task_id": "python-cli-dry-run",
  "user_prompt": "Add a --dry-run flag to the CLI...",
  "model": "deterministic-agent-v1",
  "provider": "mock",
  "context_files": ["src/cli.py", "tests/test_cli.py"],
  "tool_calls": [
    {
      "command": "read_context",
      "stdout": "Loaded 2 context file(s).",
      "stderr": "",
      "exit_code": 0,
      "duration_ms": 25
    },
    {
      "command": "edit_files",
      "stdout": "Prepared deterministic patch for src/cli.py.",
      "stderr": "",
      "exit_code": 0,
      "duration_ms": 35
    }
  ],
  "shell_outputs": [
    {
      "command": "pytest tests/test_cli.py",
      "stdout": "All verification checks passed.",
      "stderr": "",
      "exit_code": 0,
      "duration_ms": 120
    }
  ],
  "file_diffs": [
    {
      "path": "src/cli.py",
      "change_type": "modified",
      "diff": "diff --git a/src/cli.py b/src/cli.py\n--- a/src/cli.py\n+++ b/src/cli.py\n@@ -1,3 +1,4 @@\n+# Deterministic mock change produced by forge-eval-runner\n"
    }
  ],
  "final_answer": "Mock agent completed task python-cli-dry-run with deterministic trace data.",
  "verification_result": {
    "command": "pytest tests/test_cli.py",
    "passed": true,
    "stdout": "All verification checks passed.",
    "stderr": "",
    "exit_code": 0,
    "duration_ms": 120
  },
  "error": null,
  "failure_reason": null,
  "failure_category": "none",
  "started_at": "2026-05-29T03:20:23.625550+00:00",
  "ended_at": "2026-05-29T03:20:23.625785+00:00",
  "duration_ms": 0
}
```

**失败 trace 示例**（parser-regression-failure）：

```json
{
  "task_id": "parser-regression-failure",
  "error": "verification_failed",
  "failure_reason": "Mock verification command returned a non-zero exit code.",
  "failure_category": "verification_failed",
  "verification_result": {
    "command": "pytest tests/test_parser.py",
    "passed": false,
    "stdout": "1 test failed.",
    "stderr": "AssertionError: simulated failure",
    "exit_code": 1,
    "duration_ms": 120
  }
}
```

**截图建议**：格式化 JSON 输出，重点展示 trace 的完整字段结构。可以用 `| python3 -m json.tool` 或 `| jq .` 格式化。

---

## 5. 获取 Metrics

```bash
curl http://localhost:8000/runs/{run_id}/metrics
```

响应：

```json
{
  "total_tasks": 2,
  "passed_tasks": 1,
  "failed_tasks": 1,
  "success_rate": 0.5,
  "verification_coverage": 1.0,
  "average_tool_calls": 2.0,
  "average_duration_ms": 0.0,
  "failure_categories": {
    "verification_failed": 1
  },
  "tasks": [
    {
      "task_id": "python-cli-dry-run",
      "passed": true,
      "verification_passed": true,
      "tool_calls": 2,
      "duration_ms": 0,
      "failure_category": "none"
    },
    {
      "task_id": "parser-regression-failure",
      "passed": false,
      "verification_passed": false,
      "tool_calls": 2,
      "duration_ms": 0,
      "failure_category": "verification_failed"
    }
  ]
}
```

**字段说明**：

| 字段 | 含义 | 面试价值 |
|---|---|---|
| `success_rate` | 通过率 | "Agent 任务整体质量如何" |
| `verification_coverage` | 执行了验证的任务比例 | "是否每个任务都跑了验证" |
| `average_tool_calls` | 平均工具调用次数 | "Agent 执行复杂度" |
| `failure_categories` | 失败类型聚合 | "失败集中在哪些类型" |
| `tasks[].passed` | 单任务通过/失败 | "哪些任务失败了，为什么" |

**截图建议**：这是最适合截图的端点——数据量小、信息密度高、能直接展示评测结果。

---

## 6. 完整演示流程（面试用）

```bash
# 1. 启动服务
uv run uvicorn app.main:app --port 8000

# 2. 查看可用任务
curl http://localhost:8000/tasks | python3 -m json.tool

# 3. 创建 eval run
curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{"provider": "mock", "model": "deterministic-agent-v1"}' \
  | python3 -m json.tool

# 4. 从响应中提取 run_id，查看 trace
curl http://localhost:8000/runs/{run_id}/trace | python3 -m json.tool

# 5. 查看 metrics
curl http://localhost:8000/runs/{run_id}/metrics | python3 -m json.tool

# 6. 打开浏览器查看 OpenAPI 文档
open http://localhost:8000/docs
```

完整流程约 2 分钟，可在面试中实时演示。
