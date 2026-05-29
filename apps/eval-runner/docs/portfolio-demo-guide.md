# Portfolio Demo Guide

这份文档帮助你在作品集展示和面试中演示 forge-eval-runner。

## 项目一句话定位

> forge-eval-runner 是一个 Python/FastAPI Agent 评测服务，支持任务定义、结构化 trace 捕获、per-task metrics 和 failure analysis。

## 解决什么问题

Agent 产品不能只看 final answer。面试官会追问：

- 你怎么知道 Agent 做对了？
- 失败了怎么分析？
- trace 怎么设计？
- metrics 怎么算？

forge-eval-runner 用结构化数据回答这些问题。

## 2 分钟演示脚本

### 0:00 - 0:20 开场

> 这是 forge-eval-runner，一个独立的 Agent 评测服务。它不是 Forge 的附属品，而是一个独立项目，专门解决 Agent 评测和可观测性问题。

### 0:20 - 0:50 展示任务定义

```bash
curl http://localhost:8000/tasks | python3 -m json.tool
```

> 每个评测任务定义了 prompt、context files、verification command 和 expected success。这不是抽象的 golden set，而是具体的 coding agent 任务。

### 0:50 - 1:20 创建 Run 并展示 Trace

```bash
curl -X POST http://localhost:8000/runs \
  -H "Content-Type: application/json" \
  -d '{"provider": "mock", "model": "deterministic-agent-v1"}' \
  | python3 -m json.tool
```

> 执行后产出 AgentTrace，包含 tool calls、shell outputs、file diffs、verification result、failure reason 和 failure category。每一步都有结构化记录，不是日志文本。

### 1:20 - 1:50 展示 Metrics

```bash
curl http://localhost:8000/runs/{run_id}/metrics | python3 -m json.tool
```

> Metrics 包含 success rate、verification coverage、per-task pass/fail 和 failure categories 聚合。这样就能回答"这次 run 有多少任务是因为 verification_failed 失败的"。

### 1:50 - 2:00 收束

> 第一版用 deterministic mock runner，先把 trace schema 和 API contract 固定下来。后续替换为真实模型/provider 时，API shape 不需要变。整个项目有 pytest 覆盖、ruff 检查、Dockerfile 和 docker-compose。

## 面试讲解路径

### 路径 A：产品角度（AI Product Engineer）

1. 为什么做：Agent 产品不能只看 final answer
2. 解决什么：trace、metrics、failure analysis
3. 和 Forge 的关系：Forge 是产品，eval-runner 是基础设施
4. 结果：可量化的质量判断

### 路径 B：工程角度（LLM Application Engineer）

1. 技术栈：FastAPI + Pydantic v2 + pytest + Docker
2. Schema 设计：Pydantic strict mode、FailureCategory 枚举
3. API contract：OpenAPI 自动生成、类型安全
4. 工程化：测试覆盖、代码质量、容器化

### 路径 C：Agent 角度（Agent Engineer）

1. Trace schema：Agent 执行的每一步都有结构化记录
2. Failure categories：枚举而非自由文本，可聚合
3. Metrics：success_rate、verification_coverage、per-task pass/fail
4. 可扩展性：mock runner -> real provider adapter

## 如何解释 Mock Runner

> 第一版用 deterministic mock runner，原因是：先把 trace shape 固定、API contract 稳定、测试可复现，工程边界清楚之后再接真实 provider。如果一上来就接模型，容易把精力花在调 API 上，反而忽略了 eval 本身的工程设计。

关键点：
- Mock runner 不是偷懒，是工程策略
- 它让项目从第一天起就有完整测试
- 替换为真实 adapter 时，API shape 不需要变
- 类似于前端开发里的 mock service worker

## 如何解释 Trace

> Trace 的价值不是记录所有日志，而是让 Agent 执行的每一步都可以结构化查询和聚合。

展示 `sample-trace-response.json`，重点讲解：
- `tool_calls`：Agent 调用了什么工具
- `shell_outputs`：验证命令的输出
- `file_diffs`：文件变更
- `verification_result`：验证是否通过
- `failure_category`：失败分类（枚举）

## 如何解释 Metrics

> Metrics 让 Agent 质量可量化，不只是"看起来挺好的"。

展示 `sample-metrics-response.json`，重点讲解：
- `success_rate`：整体通过率
- `verification_coverage`：验证覆盖率
- `failure_categories`：失败类型聚合
- `tasks[]`：per-task pass/fail

## 如何解释 Failure Categories

> 把失败分类而不是只有自由文本，这样 failure analysis 可以做聚合统计。

| 分类 | 含义 |
|---|---|
| `none` | 无失败 |
| `verification_failed` | 验证命令未通过 |
| `no_verification` | 没有执行验证 |
| `runner_error` | Runner 执行错误 |
| `tool_error` | 工具调用错误 |
| `timeout` | 超时 |

## 和 Forge 的关系

| | Forge | forge-eval-runner |
|---|---|---|
| 定位 | Agent 产品工作台 | Agent 评测基础设施 |
| 语言 | Rust/Tauri/React | Python/FastAPI |
| 面向 | 终端用户 | 工程团队 |
| 证明 | Agent UX、上下文、权限、验证 | trace、metrics、failure analysis、API |

组合价值：Forge 证明我能做 Agent 产品，forge-eval-runner 证明我能做 Agent 评测和可观测性。

## 截图清单

1. **OpenAPI 文档页**：浏览器访问 `/docs`
2. **任务列表**：`GET /tasks` 响应
3. **创建 Run**：`POST /runs` 请求和响应
4. **Trace（成功）**：通过任务的 trace，展示 tool_calls + verification_result
5. **Trace（失败）**：失败任务的 trace，展示 failure_category
6. **Metrics**：metrics 汇总，展示 success_rate + failure_categories
7. **架构图**：`docs/architecture.md` 中的 Mermaid 图
