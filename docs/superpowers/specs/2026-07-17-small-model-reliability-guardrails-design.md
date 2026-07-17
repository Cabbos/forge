# Forge 小模型 Anthropic 优先可靠性设计

Date: 2026-07-17
Status: pending user review
Revision: 2 — 根据真实 Anthropic/OpenAI-compatible A/B 结果缩小范围
Scope: Forge Desktop Agent 的本地 Qwen 传输选择、工具失败记账、工作区路径身份和显式保护路径

## 目标

让远程 vLLM 上的 `qwen3.6-35b-a3b-nvfp4-fast` 通过最适合它的协议运行 Forge
Agent，同时只修复 A/B 之后仍能复现的协议无关底层缺陷。

本轮不再假设“20 轮耗尽主要需要一个新的重型收敛控制器”。真实 A/B 显示，切换到
Forge 已有的 Anthropic-compatible adapter 后，两道关键 R1 已在 11–12 轮内完成。

本轮目标因此缩小为：

1. 将该远程 Qwen 的默认评测传输改为 `custom_anthropic`；
2. 修复工具失败已经发生、但 Agent 仍记录为成功或零失败的问题；
3. 先复现 `/var` 与 `/private/var` 等工作区路径身份问题，只在确认权限判断不一致时修复；
4. 对用户明确保护的锁文件提供确定性的依赖命令预检；
5. 用严格单并发 R1 复测决定是否还需要后续收敛控制。

Forge 不能因为切换协议而降低权限边界，也不能把模型错误隐藏成成功。

## 配置与链路证据

CC Switch 的 `remote-vllm` Claude 配置已经包含：

- `ANTHROPIC_BASE_URL`，指向远程 vLLM 服务；
- `ANTHROPIC_AUTH_TOKEN`；
- Opus、Sonnet、Haiku 三个默认模型都映射到
  `qwen3.6-35b-a3b-nvfp4-fast`。

Forge 已经原生支持：

- provider：`custom_anthropic`；
- transport：`CustomAnthropicCompatible`；
- endpoint：`<base>/v1/messages`；
- Anthropic blocks 工具消息；
- `FORGE_CUSTOM_ANTHROPIC_API_KEY`、`FORGE_CUSTOM_ANTHROPIC_BASE_URL` 和
  `FORGE_CUSTOM_ANTHROPIC_MODEL`。

因此切换评测协议不需要修改 vLLM 服务端，也不需要修改 CC Switch 数据库。

## A/B 结果

使用相同模型、相同 R1 case、相同 Forge Agent 和严格单并发进行验证。

| 用例 | OpenAI-compatible 基线 | Anthropic 实测 | 结果 |
|---|---:|---:|---|
| `priority-labels` | 失败；18 轮；43.361 秒；8 确认 | 通过；11 轮；29.034 秒；5 确认 | 轮次下降 38.9%，锁文件零越界 |
| `due-date-labels` | 轮次耗尽；20 轮；50.232 秒；9 确认 | 通过；12 轮；38.116 秒；8 确认 | 轮次下降 40%，正常完成 |

Anthropic smoke 还证明服务端能够返回标准：

- `type=message`；
- `stop_reason=tool_use`；
- `content[].type=tool_use`；
- 工作区相对参数 `{"path":"package.json"}`。

完成两题后，远程服务状态为：

- health HTTP 200；
- `vllm:num_requests_running = 0`；
- `vllm:num_requests_waiting = 0`。

保存的原始产物：

- `apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-priority-labels-smoke.json`；
- `apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-due-date-smoke.json`。

## 结论边界

当前收益不能简单归因为 Anthropic 线协议本身。Forge 的 Anthropic adapter 同时使用了
更完整的默认系统提示、工具描述和 Anthropic block 消息格式。因此本轮结论是：

> 对当前 Qwen 和当前 Forge 实现，`custom_anthropic` 这条完整 adapter 链路明显优于
> `custom_openai` 链路。

本轮不会为了证明某一个因素而拆分系统提示、工具 schema 和消息编码做多变量实验。产品
决策以端到端 Agent 结果为准。

## A/B 后仍存在的真实缺陷

### 1. 工具失败记账不可信

`due-date-labels` 中：

- 首次 `npm test` 实际退出 1；
- 模型调用了不存在的截断工具名 `write_to_`；
- raw trace 中存在失败结果，但最终 `failed_tool_count` 仍为 0。

`priority-labels` 中：

- 两个 shell 调用被 `blocked_external_path` 阻止；
- raw `tool_call_result.is_error` 为 true；
- 最终 `failed_tool_count` 仍为 0。

这会污染循环进展判断、continuity 反思、评测归因和用户最终摘要，必须优先修复。

### 2. 路径身份存在待验证风险

同一临时工作区在 trace 中同时出现：

- `/var/folders/.../workspace`；
- `/private/var/folders/.../workspace`。

同一运行中还有两个 shell 调用被判为 `external_write`，但脱敏 trace 没有保留这两个调用
的命令参数，因此当前证据不能证明阻止事件由路径别名导致。实现前必须用定向测试构造
等价路径；只有测试能够复现权限判断不一致时，才修改路径核心。若现有逻辑已经正确，
本轮只补充安全的诊断证据，不做推测性权限改动。

### 3. 显式保护路径仍依赖模型自律

Anthropic 本轮没有再次安装测试依赖，因此没有修改 `package-lock.json`。这说明协议选择
降低了发生概率，但没有建立运行时保证。用户明确写出“不要修改 package-lock.json”时，
Forge 仍应阻止已知会修改该锁文件的依赖命令。

## 已选方案

采用“Anthropic 优先 + 三个最小协议无关修复”。

```text
远程 Qwen
  -> custom_anthropic /v1/messages
  -> 现有 Anthropic 工具链
  -> CanonicalWorkspaceIdentity
  -> ExplicitProtectedPathPreflight
  -> ToolExecutor
  -> SharedToolOutcomeClassifier
  -> AgentTurn / LoopGuard / Continuity / Eval evidence
```

不在本轮引入新的通用规划器、动作新颖度数据库或最终轮次状态机。

## 设计细节

### 1. 远程 Qwen 默认使用 custom_anthropic

真实评测使用以下映射：

- `FORGE_HEADLESS_PROVIDER=custom_anthropic`；
- `FORGE_HEADLESS_MODEL=qwen3.6-35b-a3b-nvfp4-fast`；
- `FORGE_CUSTOM_ANTHROPIC_BASE_URL=<CC Switch ANTHROPIC_BASE_URL>`；
- `FORGE_CUSTOM_ANTHROPIC_API_KEY=<CC Switch ANTHROPIC_AUTH_TOKEN>`；
- `FORGE_CUSTOM_ANTHROPIC_MODEL=qwen3.6-35b-a3b-nvfp4-fast`。

认证令牌只进入子进程环境，不写入命令产物、报告或仓库。报告只保存 provider、model、
安全 endpoint 元数据和健康指标。

`custom_openai` 继续保留为兼容和对照通道，但不再作为该 Qwen 的默认评测通道。

### 2. 共享工具结果分类器

新增或收敛一个 `tool_result_is_error(tool_name, result)` 语义，供以下位置共同使用：

- `AgentSession` 累积 `failed_tool_count`；
- `completed_tool_trace`；
- LoopGuard 的 `made_progress`；
- continuity 工具结果与反思输入；
- headless trace 投影。

第一版保持最小、确定性规则：

- shell 结果中可解析到非零 `Exit code` 时为失败；
- executor 明确返回 `is_error=true` 时为失败；
- `Unknown tool:` 和 `Unknown MCP tool:` 为失败；
- `Error:`、`Denied:`、`Permission denied`、`Tool execution blocked` 为失败；
- 现有中文阻止结果使用稳定错误代码或结构化标志，不依赖展示文案翻译；
- 缺失工具结果继续为失败。

若当前执行 API 只能返回字符串，先复用 shell exit code 和稳定前缀完成最小修复；不为本轮
重写整个 ToolExecutor 返回类型。后续再独立设计结构化 `ToolExecutionOutcome`。

同一工具结果在 UI、turn state 和 eval trace 中必须得到一致失败结论。

### 3. CanonicalWorkspaceIdentity 定向复现

先为权限外部路径判断和 FileExecutor 建立定向契约测试。只有测试复现不一致时，才提取
小型共享路径身份函数。

输入：

- working directory；
- 工具参数中的文件路径，或 shell policy 提取出的目标路径。

输出：

- canonical workspace root；
- canonical requested path；
- workspace-relative identity；
- `inside_workspace`；
- 稳定错误原因。

规则：

1. 已存在路径使用 filesystem canonicalization；
2. 新文件使用最近存在父目录的 canonical path 加文件名；
3. `/var` 与 `/private/var` 等别名必须得到同一 identity；
4. `..` 越界、symlink 越界和无法唯一解析的路径继续拒绝；
5. 不自动猜测模型漏字符形成的任意路径；
6. shell 命令中的冗余 `cd <canonical workspace>` 不应被视为外部写入证据。

如果无法复现，则不改变安全判定，只让 permission evidence 保存稳定分类和脱敏后的路径
identity，供下一次真实失败归因。本轮不改变用户界面展示路径。

### 4. 显式保护路径预检

从当前用户消息中只提取高置信度保护路径表达：

- “不要修改 X”；
- “不得改动 X”；
- “do not modify X”。

第一版只将明确出现的文件名变成当前 turn 的保护集合，不推断目录或通配符。

在 shell 权限确认之前识别 npm、pnpm、yarn、bun 的 install/add/remove/update 类命令。
如果对应锁文件位于保护集合，则：

- 阻止命令；
- 返回稳定原因代码和命中的保护文件；
- 建议使用已有依赖、项目 test script 或 `--no-install` 形式；
- 不自动改写或执行替代命令。

用户明确要求依赖变更且没有保护锁文件时，继续走现有人工确认，不自动放行。

### 5. 暂缓重型收敛控制

原设计中的 `ActionNoveltyTracker`、进度胶囊和保留最终 provider round 暂不实施。

原因：

- Anthropic 已将最难用例从 20 轮降至 12 轮；
- 当前更直接的问题是失败工具没有进入失败计数；
- 在失败记账修正前设计收敛策略会使用不可信输入；
- 过早限制重复读写可能误伤正常探索和修复。

完成本轮修复后，如果完整 R1 两轮仍出现 `model_round_limit` 或重复验证，再以新的 trace
单独设计收敛控制器。

## 错误处理

- Anthropic endpoint 或模型不可用：停止远程评测，不自动回退 OpenAI-compatible 并生成
  不可比结果。
- Anthropic 响应不符合 message/tool_use 契约：保存脱敏协议错误，停止该 case。
- 工具结果无法分类：保留 executor 原始 `is_error`；若该字段也不可用，标记分类未知，
  不假定成功。
- 路径无法 canonicalize：使用现有 fail-closed 路径，不扩大权限。
- 保护路径解析有歧义：不生成新的硬规则，保留现有权限确认。
- vLLM `waiting > 0` 持续增长或 health 非 200：停止后续 case，不并发追加请求，也不自动
  重启服务器。

## 测试策略

遵循测试先行。每个运行时代码修改前必须先新增失败测试。

### 单元测试

- `custom_anthropic` 路由到 `AnthropicAdapter` 和 `<base>/v1/messages`；
- 非零 shell exit code 在 turn trace、累计计数和 LoopGuard 中都算失败；
- `Unknown tool: write_to_` 算失败；
- raw executor `is_error=true` 不能在 turn projection 中变成成功；
- 定向测试证明 `/var/.../workspace` 与 `/private/var/.../workspace` 是否得到同一 identity；
- 新文件父目录 canonicalization 不允许 symlink/`..` 越界；
- canonical workspace 前的冗余 `cd` 不触发外部路径误判；
- 明确保护 `package-lock.json` 时阻止 npm install/add；
- `npm test`、`npx tsc --noEmit` 和无锁文件副作用的命令不因此被阻止；
- 未保护锁文件的依赖变更仍进入现有人工确认。

### 集成测试

使用 fake adapter 驱动完整 `AgentSession`：

- 一个失败 shell + 一个未知工具得到 `failed_tool_count=2`；
- LoopGuard 不把只有失败工具的批次记为进展；
- permission ledger、turn trace 和 headless trace 的失败数一致；
- 若定向测试先复现失败，则等价 macOS 路径修复后可以读写工作区内文件；
- 真正外部路径继续拒绝；
- 保护锁文件的依赖命令不会改变工作区快照。

### 远程回归

只使用一个 vLLM 并发槽：

1. 按 `storage-validation`、`priority-labels`、`due-date-labels` 顺序运行 R1；
2. 每题结束后检查 health、running、waiting；
3. 第一轮全部满足硬标准后，再按相同顺序完整重复一轮；
4. 不同时运行 OpenAI-compatible 对照或其他远程 Agent；
5. 保存完整脱敏 trace 和优化前后报告。

## 验收标准

硬标准：

- 三个 R1 用例通过 `custom_anthropic` 连续两轮全部通过；
- 所有 case 都没有 forbidden file diff；
- `priority-labels` 不再修改 `package-lock.json`；
- `due-date-labels` 不以 `model_round_limit` 结束；
- raw `tool_call_result.is_error`、shell exit code、`failed_tool_count` 和 continuity 失败工具数
  一致；
- `/var` 与 `/private/var` 等价路径不再产生错误的 external-path 判断；
- 真正越界路径和危险 shell 保护没有回归；
- vLLM 全程满足 health 200、`running <= 1`、`waiting = 0`；
- Desktop 相关 Rust 测试、格式检查、Clippy 和现有 acceptance 门禁通过。

目标指标：

- `storage-validation` 不高于 10 个模型轮次；
- `priority-labels` 不高于 14 个模型轮次；
- `due-date-labels` 不高于 16 个模型轮次；
- 确认请求数不高于本次 Anthropic smoke 基线；
- 不通过放宽权限或隐藏失败来达到轮次指标。

## 影响边界

预期只涉及：

- 远程评测运行配置或脚本；
- Agent 工具结果分类与 turn metrics；
- 权限/执行器共享的工作区路径 identity；
- shell 依赖变更预检和当前 turn 的显式保护路径；
- 对应 Rust 测试和 R1 报告。

实现前必须对每个拟修改 symbol 执行 GitNexus upstream impact。HIGH 或 CRITICAL 风险必须
先向用户报告，再继续编辑。提交前运行 `detect_changes(scope: compare, base_ref: main)`。

## 非目标

- 不修改 vLLM 服务端、模型权重或采样参数；
- 不修改 CC Switch 当前 provider 或保存的认证值；
- 不增加远程并发或自动重启服务器；
- 不删除 OpenAI-compatible 支持；
- 不在本轮统一所有 provider 的完整工具 catalog；
- 不在本轮实现通用 ActionNoveltyTracker 或最终轮次状态机；
- 不为 R1 题目硬编码答案、源文件路径或验证结果；
- 不自动接受高风险权限；
- 不把 eval 私有的 expected/forbidden 元数据泄漏给生产 Agent。

## 交付顺序

1. 固化 `custom_anthropic` 单并发评测运行方式；
2. 测试先行修复共享工具失败分类；
3. 测试先行复现 canonical workspace identity；仅在红灯成立时修复；
4. 测试先行增加显式保护路径的依赖命令预检；
5. 完成 Rust 回归、格式、Clippy 和 acceptance；
6. 严格单并发完成 R1 两轮；
7. 输出 A/B 与修复前后报告，再决定是否启动独立收敛控制设计。
