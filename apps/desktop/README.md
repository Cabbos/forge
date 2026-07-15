# Forge

桌面 WebView 现在启用显式生产 CSP、本机开发 CSP 与 prototype freeze；主窗口 capability 仅保留目录选择和会话事件 listen/unlisten，并已移除未使用的 shell 插件。

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
| 结果可判断 | 每轮任务围绕当前任务、项目档案和交付状态组织；预览地址会配套显示归属项目，帮助用户决定继续、验证、修复或停止。 |

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
- 将保存背景、项目档案、连接资料和最近对话历史组成每轮任务上下文；项目档案会统一展示已保存背景、用户事实和 continuity 经验，并可归档/忘记统一记忆行，send-input 也通过同一条记忆召回链路生成隐藏背景。
- 支持 MCP Resources、Prompts、Tools，以及本地 Hooks、Skills 和能力管理。
- 对危险命令、文件写入和外部连接动作进行确认拦截；确认卡会显示项目路径、影响文件、单次确认范围和后端权限依据，批准/取消响应会作为可回放事件写入历史。Composer 输入框旁现在提供 `手动确认`、`信任项目` 和 `完全访问` 模式入口；`信任项目` 会在当前运行期按项目继承到新对话并接管当前项目里已挂起的写入确认，`完全访问` 会接管当前项目里已挂起的确认并跳过常规非 Shell 写入、MCP 和工具确认，但项目定义的脚本/构建/测试、未知 Shell 执行和项目外读取仍要求对当前命令做一次显式决定，批准只绑定这一条规范化命令的一次执行；项目外写入、灾难命令和显式拒绝规则在所有模式下始终阻断。自动批准、手动确认、阻断和用户批准/取消都会带 workspace、操作、风险、权限模式和原因等后端 ledger 证据，工具详情会显示自动批准/阻断依据而不是生成额外噪音卡片。这些模式也可从项目状态卡或 Settings 调整，并可通过 `恢复手动确认` 关闭。ask-user 卡会说明当前只能继续或取消，具体偏好应作为新消息补充。
- 记录任务状态、上下文来源、工具证据、预览归属、检查点、验证结果和恢复状态。
- 记录 provider usage、legacy usage、上下文用量和累计成本；send-input 会先通过 `turn_prepared` 给 Composer 上下文指示器提供后端预分发估算、不含隐藏正文的记忆召回审计，以及可见输入、隐藏系统、记忆、文件、项目档案、压缩历史、连接资料和预留输出的 context budget buckets，provider usage 到达后仍作为最终 token 事实覆盖；本地 reload 后即使较早的 usage block 不再可用，成本展示也会跟随 session 快照或 Tauri transcript 恢复；如果历史 blocks 里有压缩记录，即使旧 session metadata 还停在压缩前，也会恢复压缩后的上下文估算。
- 通过 History 搜索、筛选、恢复、重命名、导出和清理本地 session 快照。
- 在 Settings 中查看诊断、Gateway runtime、调度任务、权限规则、记忆/资料和本机服务状态；用户管理 facts 仍在 Settings 维护，项目档案提供跨来源记忆概览。
- 最近 trigger run 的 executor、failure-category 和 lease 证据会显示在 Settings 诊断、Gateway dashboard 与 `forge_trigger show` 中。
- 在 Agent Workbench 和全局后台状态栏里查看子任务、审阅队列、completion/review-to-commit eligibility、调度任务和健康告警；同一会话恢复输出后，过期的 `会话无响应` 提醒会自动清除，正常 idle/completed 会话不会被 watchdog 重新误报。

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

`providers` 是 data-only profile：它可以添加或覆盖 provider 的 label、transport、base URL、key/model 环境变量、默认模型和基础能力标记，并会同步出现在 Settings provider 行和 Composer 模型菜单里，但不会加载可执行插件代码。Settings 现在也可以直接新增、编辑、删除这类自定义 Provider profile，并提供 NVIDIA NIM、本地 OpenAI-compatible endpoint、Anthropic-compatible gateway 等预填模板。`api_key_env: []` 表示本地兼容服务不需要鉴权；Forge 会跳过 missing-key gate，并且不会发送空 `Authorization` / `x-api-key` header。开始会话前的 readiness panel 也会读取同一份 provider catalog：no-auth local profile 不会被误报为缺密钥，当前模型不在 provider 目录里时会先给出提示，Provider 证据缺失只作为 warning，cached manual probe 失败才会阻断并引导用户回 Settings 的模型服务页重新检测。

Settings 的 Provider 行还支持手动刷新模型目录：OpenAI-compatible endpoint 会调用 `/models`，Anthropic 和用户配置的 Anthropic-compatible gateway 会调用 `/v1/models`，配置为 `native_gemini` 的 profile 会调用 Gemini `/v1beta/models` 并只保留支持 `generateContent` 的模型，Ollama/local 会调用本地 `/api/tags` 且不发送鉴权 header，DeepSeek、Kimi/Moonshot、GLM/Zhipu、MiniMax 这类暂未声明 live model-list endpoint 的 registry provider 会返回 Forge static fallback catalog。刷新结果会明确标注来源和刷新日期：`Live /models · 目录刷新 2024-06-09` 表示来自 endpoint，`Forge static catalog · not live-certified · 目录刷新 2024-06-09` 表示来自 registry fallback。可用结果会连同来源和时间证据写入本机 provider catalog cache，并出现在 Composer 模型菜单和 Settings provider 元信息里供选择；也可以在刷新结果里点某个模型，显式更新当前 Composer provider/model。对可编辑的自定义 Provider，还可以把刷新出的模型显式保存为 Provider 默认模型。Forge 不会自动改 profile 默认模型，static fallback 不代表 live endpoint certification，也不代表 Bedrock 或所有非兼容模型端点已经认证。

手动 Provider 兼容性检测也会保存一份脱敏证据摘要：`get_provider_catalog` 会带回上次用户触发的 probe 状态、检测日期、模型、Base URL 和检查项，Settings 可在重新打开后显示“上次手动检测通过/失败”。Provider 行还会把 manual probe、model catalog source 和目录刷新日期汇总成一条“证据摘要”，例如 `手动检测通过 · 检测 2024-06-09 · 目录 Live /models · 目录刷新 2024-06-09`、`尚未手动检测 · 目录 static fallback · 目录刷新时间未知`、`手动检测失败 · 检测时间未知 · 目录未验证`。通过证据或 live catalog 超过 14 天后会降级为 `证据需复核`，例如 `检测已超过 14 天`、`目录刷新已超过 14 天`，readiness 会给出打开 Settings 的复核入口，但不会自动发起付费 API probe；只有旧目录证据时，手动刷新模型目录也会清掉目录 stale 提示。开始会话前的 readiness panel 会复用这份摘要：手动检测失败会显示 `Provider 检测失败` 并提供打开 Settings 的动作；这个动作会直接回到“模型服务”页，即使用户上次把 Settings 停在通用或诊断页。尚未检测或只有 static fallback 只提示用户确认，不会自动发起 probe。这不是启动时自动探测，也不是对所有 provider 的 live certification；它只是让用户已经执行过的检测结果、检测时间、模型目录来源和目录刷新时间不随弹窗状态消失。

桌面后端使用 reference-only 的系统凭据存储层：macOS 生产构建使用 Keychain，不支持系统存储的平台会 fail closed。桌面启动和 headless 启动会先分别迁移旧 `config.json` / `profiles.json` 中的明文 key：写入确定性引用、读回逐项比对后才原子替换源文件；任一步失败都保留原文件字节，单个文件已完成而另一个失败时下次启动可幂等续迁。Provider 会话、恢复、手动 probe、模型目录和诊断均从注入的系统存储解析凭据，引用缺失或存储不可用不会降级成普通“未配置”，而会阻止 provider 启动并给出重新保存密钥的恢复提示；API-key status IPC 只返回 configured/source/status/error，不返回 preview 或 secret。普通 `app.log` 与结构化 JSONL 日志会在创建、追加或轮转文件之前统一脱敏已登记凭据、鉴权 header、敏感 JSON 字段和 URL query/fragment；脱敏失败会直接抑制该条持久化。OpenAI-compatible 适配器只记录 provider、消息/工具数量和请求字节数，不记录请求正文。

项目检查点使用 V2 Git 快照：保存完整 HEAD、porcelain-v2 状态、分离的 staged/unstaged full-index binary patch、未跟踪文件原始字节和 executable 位，可往返 staged-only、unstaged-only、同文件双层修改、rename/delete、二进制与 unborn 仓库。恢复会先验证 schema、路径、base64、大小、unsupported paths 和 HEAD；旧版、符号链接、特殊/超限路径或 HEAD 漂移会在修改工作区前拒绝。预检后的任何 apply/文件恢复失败都会把调用前状态完整恢复，并且不会用 `git clean` 删除未捕获路径。

当前内置中国 coding preset 跟随官方推荐的 coding 默认模型：Kimi/Moonshot 默认 `kimi-k2.7-code`，GLM/Zhipu 默认 `glm-5.2`。旧的 Kimi/GLM 模型仍保留在 static fallback catalog 中，便于兼容已有配置。

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

设置页的 Provider 行提供手动兼容性检测和手动模型目录刷新：只在用户点击后发起最小请求，检查 key、base URL、模型、streaming、工具 schema、`/models`、Anthropic-compatible `/v1/models`、Gemini `/v1beta/models` 或 Ollama `/api/tags` 列表，并显示对应 Provider 的错误或修复建议；启动时不会自动探测付费 API。

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
scripts/acceptance.sh          # matrix contract + build + eval + Level 3 runtime + desktop smoke
scripts/acceptance.sh --dry-run
scripts/acceptance.sh --list-json
scripts/acceptance.sh --only "<label>"
scripts/acceptance.sh --grep "<text>"
scripts/acceptance.sh --results-json gate-results.json
node scripts/release-confidence-summary.mjs --markdown
```

当前 acceptance smoke 会先运行 `node --test scripts/acceptance.test.mjs`，确保可执行 gate matrix、help、`--list-json`、backend authority domain metadata、tier/runtime-cost/manual-evidence metadata、`--only` 精确标签入口、`--grep` 大小写不敏感标签筛选和 `--ci-default` 子集入口保持一致；随后覆盖 loop event journal、projection replay、policy/budget preflight、durable human gate、runtime health snapshot、typed completion evidence、Completion Contract V2 eligibility facts、review-to-commit eligibility、gateway runner status、gateway/local parity、gateway eligibility dry-run 与 degraded fallback smoke、runtime journal authority/recovery、desktop eval promotion evidence、memory recall/archive coverage status、memory physical migration dry-run report、release confidence summary contract tests、subagent runtime projection、completion contract mocked desktop smoke，以及 resume、Settings 诊断、Provider probe、Provider 模型目录 live/static 刷新、选择、默认值保存、Provider metadata 紧凑渲染、Provider evidence start readiness、预览归属详情、项目档案统一记忆归档/忘记动作、stale 健康告警清除、stale banner 当前会话作用域、自定义 Provider profile 新增/编辑/删除、Gateway runtime、权限规则、确认响应回放、pending confirmation snapshot consistency、Trust Project、Composer 完全访问入口、权限 ledger evidence、调度任务、A2A 审阅、derived parent/child lineage badge 和后台任务列表。`scripts/acceptance.sh --list-json` 会把 gate 分成 `fast-contract`、`runtime-core`、`desktop-ui`、`manual-evidence` 和 `full-release`，并把 `fast-contract` / `runtime-core` 标为 CI-default；`scripts/acceptance.sh --ci-default` 会运行或 dry-run 同一组 CI-default gates；`scripts/acceptance.sh --grep runtime` 会包含 `runtime authority fast gate`，覆盖 journal、projection replay、completion、diagnostics、recovery 和 desktop helper smoke；`memory recall and archive coverage status` 现在也会运行 `memory::unified`、`ipc::unified_memory` 与 `ipc::send_input_context` 后端测试，覆盖 Memory Authority Map V2、UnifiedMemoryRecord V2 metadata、Unified Action API V2 typed result/error 和 body-free Recall Planner audit；`memory physical migration dry-run report` 会运行 `scripts/memory-migration-dry-run.mjs`，输出 physical store migration dry-run 的 id/action/recall/redaction invariants 与 rollback plan，同时明确真实物理迁移尚未开始；`release confidence summary contract tests` 会验证 PR 可贴的 JSON/Markdown confidence summary 能聚合 acceptance matrix、`--ci-default-only` 快速门禁 scope、CI-default gate 汇总、gate results、gate-results execution completeness、execution reason evidence、dashboard artifact output、acceptance domain/tier breakdowns、gate detail metadata、eval report、boundary evidence、verified capability evidence 和缺失或失败的 capability evidence，并验证 `--fail-on-attention` 能把需要人工关注的 summary 变成非零退出；`A2A child runtime event capsule and file IO bridge` 会运行 A2A bus/child 后端测试和 workbench normalization tests，覆盖 child runtime events、parent child capsules、review gate V2、recovery suggestions 和 live file-ish facts；`desktop eval promotion evidence smoke` 会验证 desktop headless trace、`forge_session export-eval`、eval import 和 scoring 都保留 ForgeRunEvidence V2、A2A child evidence completeness、context budget bucket scoring、schema identity scoring、permission decision evidence scoring、verification evidence quality scoring、gateway runtime safety、runtime recovery quality 与 completion eligibility evidence scoring。Acceptance 证据现在还包含 mocked desktop restart runtime smoke（macOS partial evidence，不是官方 Tauri/WebDriver force-quit proof）、desktop restart harness preflight、contract tests 和 blocker documentation status gate（明确报告当前 macOS official WKWebView WebDriver support gap，并要求非 macOS 在宣称官方 ready 前存在 desktop restart harness launch command）、provider usage known/unknown telemetry、transcript usage hydration、state consistency map status、bounded post-shell file-effect evidence、persisted A2A lineage、gated headless ownership policy/approval checks、真实 Rust `run_worktree_worker` harness（mock adapter/harness）、A2A child runtime 的 live file-ish facts，以及 direct ToolExecutor file-ish tools 的 `file_io` stream smoke。

Eval-runner 现在也包含 prepared-turn evidence scoring，用来检查 prompt/context-source 质量；file effects evidence scoring 会在 ForgeRunEvidence file effects 存在时检查 changed-file 重复、trace/evidence 对齐，以及 file diff 的 path/change-type/diff 完整性；tool/shell evidence scoring 会检查 replay identity、工具/命令事实、exit-code 一致性、trace 对齐和 secret-like 输出泄漏；usage unknown conflict scoring 会检查未知 provider usage 保持 explicit unknown reason 且不携带伪造 token/cost 数字；provider usage value validation 会拒绝负数或畸形 token/cost facts；failure evidence scoring 会检查失败 category/reason 与 trace 对齐；continuity lessons scoring 会检查 formed lesson metadata；memory recall audit scoring 会检查 recall candidate 的 decision reason 和 injected token/budget evidence。

`scripts/acceptance.sh --results-json <path>` 会在真实执行 gate 时写出 release confidence 可消费的 gate-results JSON，并为每个已执行 gate 保留 authority domain、tier、runtime cost、manual-evidence 和 CI-default metadata；`node scripts/release-confidence-summary.mjs --no-acceptance-matrix --gate-results <path>` 可以只从这个自描述结果文件生成 summary；dry-run、list-json 和 help 输出不会生成执行结果。

GitNexus CLI 或 index refresh 命令需要通过 `node scripts/gitnexus-safe.mjs -- <command>` 包一层 60 秒超时；如果 GitNexus MCP/CLI 卡住、过期或不可用，使用 `node scripts/gitnexus-safe.mjs --print-template` 输出 fallback impact report，并记录 command/error、index freshness、symbols/files/callers/tests、authority domains 和 residual risk。

这套 matrix 也包含 gateway session-host run evidence 与 backend restart-smoke dry-run 两个可靠性 gate。

Phase 8 desktop UI evidence helpers 会把标准化 recovery commands 和 `permissionScope` 从 preflight 透传到 disposable loop status/runbook 摘要，即使 UI preflight 被跳过、嵌套状态是 `not_checked` 也会保留 macOS privacy 边界说明和 strict preflight / `--require-live-ready` 恢复命令；preflight/status/runbook/doctor 会明确说明 Forge 的 Trust/Full Access 不会授予 macOS Screen Recording 或 Accessibility，并且恢复路径会在本机权限修复后指回 strict preflight 和 `--require-live-ready` 硬门禁；doctor 还提供 `--run-checks`，用于在修复权限后一次性复跑 strict preflight 和 live-ready gate；status/runbook JSON/Markdown 现在共用 `liveReadyGate.pass/reason`，用于解释 hard gate 为什么通过或被阻塞；acceptance matrix 现在运行 `--require-live-ready` 作为自动化硬门禁，这个门禁要求实际跑过 UI preflight，并且只有 validation/evidence/markdown 三件套都存在时才把 row 视为 archived complete。

Forge 会把模型调用用量和上下文窗口状态分开显示：Provider usage 行展示本次模型调用由 provider 回报的 token/cost；Composer 的上下文指示器展示估算的已用上下文和真正剩余上下文，`余` 表示 true remaining context。send-input 现在会先发出 `turn_prepared`，用后端整理出的可见输入、隐藏上下文来源、记忆/项目档案 id、记忆召回审计、context budget buckets、工作流和权限模式估算刷新 Composer label；provider usage 到达后仍会覆盖为模型调用的最终 token 事实。恢复会优先用 transcript/provider_usage 重建出的上下文用量刷新 Composer label，避免旧的持久化 metadata 覆盖更新的 token 证据。Auto-compact threshold distance 只放在 tooltip 里，避免被误读成 provider context remaining。

4C.4 fake headless owner executor fixture 当前由 focused runner/journal/projection/replay Rust tests 证明。它只在 runner test fixture 中记录 completed、pending confirmation blocker、pending tool-call blocker、interrupted、cancelled、expired 和 stale pending-view idempotency 状态链；还不是 acceptance matrix 或 e2e autonomous resume gate。

`run_gateway_read_only_owner_diagnostics` 是第一段 gated gateway read-only diagnostics owner slice：需要 explicit human approval 或 `dev_only_allow`，只读取 loop projection 生成 summary，写入 replayable requested/lease/completed owner evidence，并且只在这个 read-only result 里返回 `gateway_can_resume=true`；它不调用 provider/tool/shell，不写文件，不处理 confirmation，也不提交。Operator 可以用 `forge_trigger read-only-owner-diagnostics --task-id <id> --approved-by <name>` 或本地 dev-only flag 跑同一段 slice。

`forge_trigger ownership-eligibility --mode gateway_patch_proposal_owner` 现在会走 gateway patch proposal owner gate：结果只声明 proposal-only patch generation intent，并要求 patch review 与 diff evidence；`would_apply_patch=false`、`would_write_files=false`，direct-write gateway owner 仍默认阻塞。

边界语言保持明确：`commit remains human-gated`；`shell-internal tracing is not claimed`；`unknown provider token/cost remains unknown when adapters omit usage`；`gateway autonomous resume requires explicit policy and human approval`。Provider reported token 会保留为已知值，provider omitted usage 和 unknown pricing 会保留为 `unknown`/`null`；Gateway runtime status/CLI/Settings 现在明确显示 local-default ownership、gateway ownership opt-in gate、degraded fallback、`forge service restart` recovery command，以及嵌套 runtime health snapshot 中的 loop task、gateway queue、scheduler/runtime task、replay 和最新 recovery 事实；`forge_trigger ownership-eligibility --session-id <id> --task-id <id>` 只输出 gateway owner dry-run 的 deny/requires-approval、缺失证据、side-effect flags 和 required action，不会接管任务；session input 只有在 desktop owner 成功接手本地 turn 后才 ack。headless gate 现在记录和 replay approval intent，在授权、策略和预算事实之后做安全 coordinator dry run，并用 test-only fake executor fixture 证明 orchestration 状态链。它仍不创建真实 headless `AgentSession`、不调用 `eval_headless`、不调用模型/工具、不写文件、不设置 `gateway_can_resume=true`、不自动接受 confirmation/tool blocker。当前不包含官方 Tauri/WebDriver force-quit harness、syscall/file-descriptor tracing、full non-git workspace enumeration、billing-grade usage accounting、usage/pricing unknown 时的精确 cost、automatic creation of parent-session context、fuzzy parent/root-task selection 或 auto commit/merge/push。

Gateway trigger runs 会保留 executor、retry/dead-letter、failure-category、lease 与 restart-smoke 证据。这证明的是后端可见的 ownership 与持久化，不代表 unattended autonomous continuation 或官方 Tauri/WebDriver force-quit recovery。

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
- Gateway runtime、trigger run executor/failure/lease 证据、service facade、diagnostics doctor、session store 和 scheduler 的状态可以在 Settings/CLI 中观测。
- 子任务、审阅队列、健康告警和后台调度通过 Agent Workbench 与全局状态栏持续暴露。
- Level 3 runtime 用 append-only loop event journal、可重建 projection、durable human gate、policy/budget preflight、typed completion evidence、Completion Contract V2 eligibility facts、crash/replay regression coverage、gateway runner lease、runtime health snapshot、projected usage ledger、typed recovery state、`recover_loop_task` action family（mark-interrupted、read-only evidence export、orphan abandon、retry-safe waiting-task requeue、clear stale gateway input evidence、`forge_trigger clear-stale-session-input` operator command）、acceptance tier metadata、`forge_session export-eval` ForgeRunEvidence V2 export、release confidence summary、mocked restart runtime smoke、provider usage known/unknown telemetry、memory recall/archive coverage status、memory physical migration dry-run report、persisted A2A lineage、review-to-commit eligibility、gateway/local parity 与 degraded fallback smoke、gated headless ownership policy/coordinator dry run、test-only fake owner executor fixture、A2A child runtime events、parent child capsules、A2A review gate V2/recovery suggestions、A2A child evidence completeness scoring、memory recall quality scoring、memory recall audit scoring、context budget bucket scoring、schema identity scoring、permission decision evidence scoring、verification evidence quality scoring、gateway runtime safety scoring、runtime recovery quality scoring、completion eligibility evidence scoring、A2A child runtime file-ish tool facts、direct ToolExecutor file-ish `file_io` stream、bounded post-shell delta evidence 和真实 Rust `run_worktree_worker` harness 支撑长期 agent 工作；Memory Authority Map V2 已记录 wiki memory、memory fact、continuity experience、saved background、project archive、turn recall audit、future embedding index 的 owner/storage/scope/action/recall 边界，UnifiedMemoryRecord V2 已暴露 visibility/provenance/last-used/archive/forget-policy/recall-policy metadata，Unified Action API V2 已支持 archive/restore/forget/pin/unpin/mark_wrong_project/mark_low_value 与 memory-fact-only edit 的 typed evidence/error，Recall Planner 已在 `turn_prepared` 暴露 body-free candidate decisions、dedupe/filter reasons 和 injection budget，Context Budget Integration V2 已在 `turn_prepared.context_estimate` 暴露 visible input、hidden system、memory、files、project records、compacted transcript、connector context 和 reserved output buckets，`scripts/memory-migration-dry-run.mjs` 已提供 physical store migration dry-run、rollback plan 与 record id/archive-forget/recall/redaction invariants，但真实 physical store migration 仍未开始；Gateway runtime、diagnostics、CLI、dashboard、background loop summary 和 eval trace 会消费 backend projection 中的 usage/recovery 事实，并显式保留 local-default ownership、desktop runtime fallback 与 `forge service restart` degraded recovery command；eval-runner 会在 `ForgeRunEvidence` 存在时评分 confirmation/context/verification/scope/recovery/usage consistency、A2A child evidence completeness、memory recall quality、memory recall audit、context budget buckets、schema identity、permission decision evidence、verification evidence quality、gateway runtime safety、runtime recovery quality 和 completion eligibility consistency，并兼容 explicit V1 evidence、把缺失的 V2 completion eligibility 保持为 `unknown`；Completion Contract V2 当前会把 changed-file scope、permission、eval authority 缺口标记为 unknown；billing-grade usage accounting、usage/pricing unknown 时的精确 cost、shell-internal tracing、syscall/file-descriptor tracing、full non-git workspace enumeration、default gateway autonomous resume、real headless `AgentSession` execution、model/tool/file side effects、Tauri/WebDriver force-quit harness、automatic creation of parent-session context、fuzzy parent/root-task selection 和 auto commit/merge/push 仍未声明覆盖。

当前正在继续加强的方向：

- 更完整的真实 Tauri force-quit/reopen 恢复验收。
- 更直接的后台 gateway session-host 合约和真实 crash/reopen 验收。
- 更清晰的工具编排边界、P0/P1/P2 校准后的审阅动作契约和图片/富媒体 diff 预览。
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
