# Forge

[English](./README.en.md)

Forge 是一个本地优先的桌面端 AI Agent，用来帮助用户开发、维护和继续推进自己的本地项目。

它基于 Tauri 2、React、TypeScript 和 Rust 构建。当前的产品方向很明确：用户选择一个本地项目，直接描述想做的事，Forge 负责理解任务、带入合适上下文、在项目边界内行动、展示过程证据，并把有价值的项目背景沉淀下来，方便之后继续。

Forge 还处在早期产品阶段，功能和交互会持续快速变化。

## Forge 能做什么

- 围绕用户选择的本地项目文件夹工作。
- 运行多轮 coding agent 循环，并支持工具调用。
- 在桌面 UI 中流式展示思考、工具调用、Shell 输出、Diff、确认请求和交付总结。
- 支持 Provider 和模型选择，包括 DeepSeek、Anthropic、OpenAI、OpenRouter。
- 将项目记录、已保存背景、用户选中文件和连接资料作为本轮隐藏上下文。
- 支持用 `@file` 引用项目文件，并且文件搜索会限定在当前会话项目内。
- 对高风险 Shell 命令、文件写入和连接工具调用进行确认拦截。
- 记录每轮任务状态、上下文来源、工具证据、验证结果、检查点和交付状态。
- 支持 MCP 资源 / Prompt / 工具、Hooks、Skills 和本地能力管理。

## 产品骨架

Forge 希望用户侧只理解三个层级：

| 产品层级 | 含义 |
| --- | --- |
| 当前任务 | Forge 当前判断用户正在做什么。 |
| 项目档案 | 长期保存的项目说明、决策、资料、任务日志和背景信息。 |
| 交付 | 预览状态、检查点、验证结果和下一步动作。 |

Workflow Router、Context Activation、Memory、Auto Compact、Wiki Storage 等都属于内部实现名词，不应该成为用户理解产品的负担。

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

可以在设置页（`Cmd+,`）配置 Provider API Key，也可以写入 `~/.forge/config.json`：

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

## 开发命令

```bash
npm run dev          # 只启动 Vite 前端服务
npm run tauri dev    # 启动完整 Tauri 桌面应用
npm run build        # TypeScript 检查 + Vite 生产构建
npm run tauri:build  # 打包桌面应用
npm run test:e2e     # Playwright E2E 测试
```

Rust 测试：

```bash
cd src-tauri
cargo test
```

## 架构概览

```text
React 前端（Vite + TypeScript）
  -> Tauri IPC commands
  <- StreamEvent protocol
Rust 后端（Tokio）
  - AgentSession：agent 循环、上下文组装、压缩、验证
  - ContextBuilder：系统提示词、摘要、选中文件、项目记录、已保存背景、连接资料、历史消息
  - ToolExecutor / Harness：文件、Shell、MCP、Hooks、Skills、权限控制
  - Project Archive：本地 markdown-like 项目记录和写回建议
  - Snapshot storage：会话、轮次、当前任务、交付和恢复状态
```

流式协议是 Forge 的主干。新增后端到前端事件时，需要同时更新：

- `src-tauri/src/protocol/events.rs`
- `src/lib/protocol.ts`

然后更新 Zustand store，并在 `src/components/messages/` 中新增或调整对应渲染组件。

## 本地上下文模型

Forge 会在每一轮任务中组装多种上下文来源：

- 系统提示词和项目说明
- 压缩后的历史摘要
- 用户通过 `@file` 选择的文件
- 已保存背景
- 项目档案记录
- MCP 连接资料
- 最近对话历史

用户选中文件只会从当前工作区读取，并带有大小限制；绝对路径、符号链接逃逸和工作区外路径都会被阻止。

## 项目说明文件

Forge 会读取项目中的说明文件作为项目级指导，例如：

- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`

这些文件用于告诉 Forge 当前项目的构建方式、开发约定和注意事项。

## 仓库卫生

本地工具状态和开发工作流沉淀不要提交到产品仓库：

- `.forge/`
- `.agents/`
- `.claude/`
- `.superpowers/`
- `docs/superpowers/`
- `test-results/`

长期产品思考和研发记录应放在 Obsidian 或其他外部知识库中。

## 当前状态

Forge 还不是一个稳定公开发行版本。当前重点是：

- 让本地 agent 循环更可靠；
- 保持清晰的工作区边界；
- 改进上下文、恢复会话和继续任务能力；
- 让桌面 UI 更安静、专业、可信；
- 让不会写代码的人也能开始做自己的小工具，同时不牺牲专业开发者的使用效率。
