# Forge

[English](./README.en.md)

![Forge mark](./src/assets/forge-mark.svg)

Forge 是一个本地优先的 AI Agent Workbench，用来在真实项目里创建、维护、修复和继续推进软件工作。

它把 CLI coding agent 的能力放进一个可审计、可恢复、可持续的桌面工作台：选择一个本地项目，描述目标，Forge 负责带入项目上下文，在工作区边界内执行文件和 Shell 操作，展示过程证据，并把有价值的项目背景沉淀下来，方便下一次继续。

> 当前状态：Forge 仍处于早期产品和 internal beta 打磨阶段。它不是稳定公开发行版，但核心方向已经明确：让本地 agent 工作变得更安全、更可见、更容易延续。

## 为什么做 Forge

Coding agent 很强，但真实使用时常见的问题不是模型不会写代码，而是工作流难以长期信任：

- 任务上下文散落在对话、文件、终端和笔记里。
- Agent 容易误读当前项目，甚至在错误工作区里行动。
- Shell、文件写入、连接工具调用缺少清晰的风险确认。
- 过程证据分散，用户很难判断结果是否真的完成。
- 一段工作中断后，下一轮很难诚实地接上进度。

Forge 的产品假设是：本地 agent 不应该只是一个聊天窗口，也不应该只是终端包装。它应该是一个围绕项目、证据、权限和交付状态组织起来的工作台。

## 核心承诺

| 承诺 | Forge 怎么做 |
| --- | --- |
| 只在当前项目里工作 | 每个会话绑定本地 workspace，`@file` 搜索和文件读取限定在当前项目边界内。 |
| 过程可见 | 思考摘要、工具调用、Shell 输出、Diff、检查点、验证结果和交付状态都以结构化事件流展示。 |
| 风险动作可确认 | 高风险 Shell 命令、文件写入和连接工具调用会触发确认，不把危险操作藏进自动执行里。 |
| 上下文可延续 | Forge 会组合项目说明、已保存背景、项目档案、用户选中文件、连接资料和压缩历史。 |
| 结果可判断 | 每轮任务围绕当前任务、项目档案和交付状态组织，帮助用户决定继续、验证、修复或停止。 |

## 产品体验

Forge 希望用户只需要理解三个产品层级：

| 层级 | 含义 |
| --- | --- |
| 当前任务 | Forge 当前判断用户正在推进的具体目标。 |
| 项目档案 | 长期保存的项目说明、决策、背景、任务日志和可复用资料。 |
| 交付 | 预览状态、检查点、验证结果、风险提示和下一步动作。 |

内部能力，例如 Workflow Router、Context Activation、Memory、Auto Compact、Wiki Storage、MCP、Hooks 和 Skills，不应该成为用户理解产品的负担。它们是 Forge 背后的能力层，不是用户必须学习的新概念。

## 能做什么

- 打开一个本地项目，并围绕该项目运行多轮 agent 任务。
- 支持 DeepSeek、Anthropic、Kimi/Moonshot、GLM/Zhipu、Alibaba/Qwen、MiniMax、OpenAI、OpenRouter、Gemini、xAI、Groq、Mistral、Ollama/local 和自定义兼容 Provider 的配置与模型选择。
- 在桌面 UI 中流式展示 agent 输出、工具活动、Shell 结果、Diff、确认请求和交付总结。
- 支持 `@file` 引用当前项目文件，并把选中文件作为隐藏上下文带入本轮任务。
- 读取项目级说明文件，例如 `AGENTS.md`、`CLAUDE.md`、`GEMINI.md`。
- 将保存背景、项目档案、连接资料和最近对话历史组成每轮任务上下文。
- 支持 MCP Resources、Prompts、Tools，以及本地 Hooks、Skills 和能力管理。
- 对危险命令、文件写入和外部连接动作进行确认拦截。
- 记录任务状态、上下文来源、工具证据、检查点、验证结果和恢复状态。
- 通过 History 搜索、筛选、恢复、重命名、导出和清理本地 session 快照。
- 在 Settings 中查看诊断、Gateway runtime、调度任务、权限规则、记忆/资料和本机服务状态。
- 在 Agent Workbench 和全局后台状态栏里查看子任务、审阅队列、completion/review-to-commit eligibility、调度任务和健康告警。

## 适合的场景

- 在一个本地项目里做小功能、修 bug、补测试、整理文档。
- 让 agent 先读项目上下文，再进行最小修改和验证。
- 给非专业开发者一个从想法到本地可运行小工具的入口。
- 给专业开发者一个更可控、更有证据链的 agent 桌面工作流。
- 在长期项目里沉淀背景、约定、决策和可继续推进的任务线索。

## 当前边界

Forge 目前有意保持边界清晰：

- 不做云端协作、组织管理、托管执行、企业网关或计费系统。
- 不承诺完全自动化地长时间无人值守执行任务。
- 不替代 Git、IDE、终端或代码审查流程，而是把 agent 工作放进可检查的本地工作台。
- 不鼓励用户理解内部上下文工程术语，UI 应尽量使用任务、项目和交付语言。

## 快速开始

```bash
npm install
npm run tauri dev
```

只启动前端开发服务：

```bash
npm run dev
```

Vite 默认运行在 `http://localhost:1420`。完整桌面端需要通过 Tauri 启动。

## API Key

可以在设置页配置 Provider API Key，也可以写入 `~/.forge/config.json`：

```json
{
  "api_keys": {
    "deepseek": "sk-...",
    "anthropic": "sk-ant-...",
    "openai": "sk-...",
    "openrouter": "sk-or-...",
    "nvidia": "nvapi-..."
  },
  "providers": [
    {
      "id": "nvidia",
      "label": "NVIDIA NIM",
      "transport": "openai_chat_completions",
      "base_url": "https://integrate.api.nvidia.com/v1",
      "api_key_env": "NVIDIA_API_KEY",
      "base_url_env": "NVIDIA_BASE_URL",
      "default_model": "nvidia/llama-3.1-nemotron",
      "supports_tools": true,
      "supports_streaming": true,
      "aliases": ["nim"]
    },
    {
      "id": "local-openai",
      "label": "Local OpenAI-Compatible",
      "transport": "openai_chat_completions",
      "base_url": "http://127.0.0.1:1234/v1",
      "default_model": "local-model",
      "api_key_env": [],
      "supports_tools": true,
      "supports_streaming": true
    }
  ]
}
```

`providers` 是 data-only profile：它可以添加或覆盖 provider 的 label、transport、base URL、key/model 环境变量、默认模型和基础能力标记，并会同步出现在 Settings provider 行和 Composer 模型菜单里，但不会加载可执行插件代码。`api_key_env: []` 表示本地兼容服务不需要鉴权；Forge 会跳过 missing-key gate，并且不会发送空 `Authorization` / `x-api-key` header。

也可以只保存 API key：

```json
{
  "api_keys": {
    "deepseek": "sk-...",
    "anthropic": "sk-ant-...",
    "openai": "sk-...",
    "openrouter": "sk-or-..."
  }
}
```

也支持从常见环境变量读取：

```bash
DEEPSEEK_API_KEY=...
ANTHROPIC_API_KEY=...
OPENAI_API_KEY=...
OPENROUTER_API_KEY=...
```

默认 Provider 是 DeepSeek，默认模型是 `deepseek-v4-flash[1m]`。

设置页的 Provider 行提供手动兼容性检测：只在用户点击后发起最小请求，检查 key、base URL、模型、streaming 和工具 schema，并显示对应 Provider 的错误或修复建议；启动时不会自动探测付费 API。

## 开发命令

```bash
npm run dev            # 只启动 Vite 前端服务
npm run tauri dev      # 启动完整 Tauri 桌面应用
npm run build          # TypeScript 检查 + Vite 生产构建
npm run tauri:build    # 打包桌面应用
npm run test:e2e       # Playwright E2E 测试
npm run check:backend  # Rust fmt + clippy + test
```

仓库根目录提供 Level 3 runtime 验收脚本：

```bash
scripts/acceptance.sh          # build + eval + Level 3 runtime + desktop smoke
scripts/acceptance.sh --dry-run
```

当前 acceptance smoke 覆盖 loop event journal、projection replay、policy/budget preflight、durable human gate、typed completion evidence、review-to-commit eligibility、gateway runner status、subagent runtime projection、completion contract mocked desktop smoke，以及 resume、Settings 诊断、Provider probe、Gateway runtime、权限规则、调度任务、A2A 审阅、derived parent/child lineage badge 和后台任务列表。Acceptance 证据现在还包含 mocked desktop restart runtime smoke（macOS partial evidence，不是官方 Tauri/WebDriver force-quit proof）、provider usage known/unknown telemetry、bounded post-shell file-effect evidence、persisted A2A lineage、gated headless ownership policy/approval checks、真实 Rust `run_worktree_worker` harness（mock adapter/harness）、A2A child runtime 的 live file-ish tool facts，以及 direct ToolExecutor file-ish tools 的 `file_io` stream smoke。

4C.4 fake headless owner executor fixture 当前由 focused runner/journal/projection/replay Rust tests 证明。它只在 runner test fixture 中记录 completed、pending confirmation blocker、pending tool-call blocker、interrupted、cancelled、expired 和 stale pending-view idempotency 状态链；还不是 acceptance matrix 或 e2e autonomous resume gate。

边界语言保持明确：`commit remains human-gated`；`shell-internal tracing is not claimed`；`unknown provider token/cost remains unknown when adapters omit usage`；`gateway autonomous resume requires explicit policy and human approval`。Provider reported token 会保留为已知值，provider omitted usage 和 unknown pricing 会保留为 `unknown`/`null`；headless gate 现在记录和 replay approval intent，在授权、策略和预算事实之后做安全 coordinator dry run，并用 test-only fake executor fixture 证明 orchestration 状态链。它仍不创建真实 headless `AgentSession`、不调用 `eval_headless`、不调用模型/工具、不写文件、不设置 `gateway_can_resume=true`、不自动接受 confirmation/tool blocker。当前不包含官方 Tauri/WebDriver force-quit harness、syscall/file-descriptor tracing、full non-git workspace enumeration、billing-grade usage accounting、usage/pricing unknown 时的精确 cost、automatic creation of parent-session context、fuzzy parent/root-task selection 或 auto commit/merge/push。

LoopTaskPanel 中的 headless readiness/lease-pending 行只是 derived-only UI/status：不代表自动继续或自动恢复，不创建 headless `AgentSession`，commit/merge/push 仍保持 human-gated。

也可以直接运行 Rust 测试：

```bash
cd src-tauri
cargo test
```

## 技术架构

Forge 是一个 Tauri 2 桌面应用：

```text
React frontend (Vite + TypeScript)
  -> Tauri IPC commands
  <- StreamEvent protocol

Rust backend (Tokio)
  - AgentSession: agent loop, tool orchestration, context assembly, compaction, verification
  - ContextBuilder: system prompt, summaries, selected files, project records, saved background, connectors, history
  - ToolExecutor / Harness: file, shell, MCP, hooks, skills, permission control
  - Snapshot storage: session, turn, current task, delivery, checkpoint, resume state
  - Project Archive: local project records and writeback proposals
```

流式协议是 Forge 的主干。后端通过 Tauri `emit("session-output", StreamEvent)` 向前端发送结构化事件，前端 store 将事件累积为可渲染的 `BlockState[]`。

新增后端到前端事件时，需要同时更新：

- `src-tauri/src/protocol/events.rs`
- `src/lib/protocol.ts`

然后更新 Zustand store，并在 `src/components/messages/` 中新增或调整对应渲染组件。

## 本地上下文模型

每一轮任务会组合多种上下文来源：

- 系统提示词和项目说明
- 压缩后的历史摘要
- 用户通过 `@file` 选择的文件
- 已保存背景
- 项目档案记录
- MCP 连接资料
- 最近对话历史

用户选中文件只会从当前工作区读取，并带有大小限制。绝对路径、符号链接逃逸和工作区外路径都会被阻止。

## 可靠性方向

Forge 的可信度来自几个工程约束：

- 前后端共享的 `StreamEvent` 协议契约。
- 工作区边界检查和 session 绑定。
- 文件写入、危险 Shell 命令和连接工具调用的确认门。
- 检查点、验证结果和交付状态的显式展示。
- 后端使用 Rust 和 Tokio 承载 agent loop、IPC、PTY、MCP 和本地存储。
- Playwright E2E 与 Rust 后端检查共同覆盖关键路径。
- Gateway runtime、service facade、diagnostics doctor、session store 和 scheduler 的状态可以在 Settings/CLI 中观测。
- 子任务、审阅队列、健康告警和后台调度通过 Agent Workbench 与全局状态栏持续暴露。
- Level 3 runtime 用 append-only loop event journal、可重建 projection、durable human gate、policy/budget preflight、typed completion evidence、crash/replay regression coverage、gateway runner lease、mocked restart runtime smoke、provider usage known/unknown telemetry、persisted A2A lineage、review-to-commit eligibility、gated headless ownership policy/coordinator dry run、test-only fake owner executor fixture、A2A child runtime file-ish tool facts、direct ToolExecutor file-ish `file_io` stream、bounded post-shell delta evidence 和真实 Rust `run_worktree_worker` harness 支撑长期 agent 工作；billing-grade usage accounting、usage/pricing unknown 时的精确 cost、shell-internal tracing、syscall/file-descriptor tracing、full non-git workspace enumeration、default gateway autonomous resume、real headless `AgentSession` execution、model/tool/file side effects、Tauri/WebDriver force-quit harness、automatic creation of parent-session context、fuzzy parent/root-task selection 和 auto commit/merge/push 仍未声明覆盖。

当前正在继续加强的方向：

- 更完整的真实 Tauri force-quit/reopen 恢复验收。
- 更直接的后台 gateway session-host 合约和真实 crash/reopen 验收。
- 更清晰的工具编排边界、审阅动作契约和图片/富媒体 diff 预览。
- 更稳定的会话恢复、长期上下文行为和跨入口 profile/runtime 状态。

## 产品路线

V1 的目标不是堆更多面板，而是让本地 agent 的核心循环可信：

- 选择项目，明确当前 workspace。
- 描述任务，Forge 自动带入必要上下文。
- 过程证据可见，风险动作可确认。
- 结果可验证，失败可恢复。
- 下一轮可以从可靠记录继续。

V2 的方向是更深的 project-native intelligence：Forge 越了解一个项目，越能自动选择正确上下文、遵守项目约定、识别风险文件，并在不暴露内部机制的情况下帮助用户继续推进。

更多产品细节见：

- [`docs/product/forge-v1-v2-roadmap.md`](./docs/product/forge-v1-v2-roadmap.md)
- [`docs/product/forge-v1-internal-beta-playbook.md`](./docs/product/forge-v1-internal-beta-playbook.md)
- [`docs/product/forge-design-language.md`](./docs/product/forge-design-language.md)

## 仓库卫生

本地工具状态和开发工作流沉淀不要提交到产品仓库：

- `.forge/`
- `.agents/`
- `.claude/`
- `.superpowers/`
- `docs/superpowers/`
- `test-results/`

长期产品思考和研发记录建议放在 Obsidian 或其他外部知识库中。
