# First Loop v1 Design

Date: 2026-05-15

Status: design spec

Owner: Forge product direction

## Summary

First Loop v0 proves the main direction:

> Describe a small tool, then move toward a previewable first version.

First Loop v1 should make that loop feel dependable. The user should not need to infer whether Forge is ready, what phase it is in, or what they should do after a turn. Forge should show a calm, small amount of product state around the conversation.

Core feeling:

> Forge is ready to make, shows what it is doing, and closes each turn with a clear way to inspect or continue.

## Product Layers

This design keeps the existing three product layers.

1. **当前任务**
   Shows the current making phase: ready, understanding, preparing, making, previewing, checking, blocked, or delivered.

2. **项目档案**
   Shows project continuity and references. It should not become the place where first-loop readiness lives.

3. **交付**
   Shows preview and checkpoint confidence. It should remain compact and actionable.

## Goals

1. Show a lightweight "准备开始" readiness state before the user sends the first meaningful request.
2. Make the process visible with a small first-loop progress strip.
3. Make successful or blocked turns end with clear next actions.
4. Make common failure states actionable and product-language-first.
5. Avoid adding new product layers or new internal terminology.

## Non-Goals

Do not build in this pass:

- a full task graph
- multi-agent orchestration UI
- file upload or document parsing
- a full diff review experience
- a full project dashboard
- new memory/wiki concepts
- deployment, packaging, or hosting
- custom generated app templates

## User Experience

### 1. Prepare To Start

When a session is open but no real work has started, Forge should show a compact readiness card.

Title:

> 准备开始

Rows:

- 工作空间
- 模型密钥
- 预览
- 检查点

Each row has a plain status and, when useful, one action.

Examples:

- 工作空间: 已选择 `forge`
- 模型密钥: DeepSeek 已配置
- 模型密钥: 还没有配置 DeepSeek, action `打开设置`
- 预览: 可启动
- 预览: 没有检测到 dev 脚本
- 检查点: 可创建
- 检查点: 当前不是 Git 项目

This card should not block every request. It should help the user understand readiness before they act.

### 2. Progress Strip

During the first loop, show a compact phase strip above or near the conversation.

Stages:

1. 理解目标
2. 准备修改
3. 正在制作
4. 可以预览
5. 等你验收

Rules:

- It should be one line, not a dashboard.
- It should derive from existing signals where possible: first-loop draft, workflow state, confirmation events, delivery summary, runtime/checkpoint status.
- It should not expose route names, task mode names, or tool names.

### 3. Turn Closure

After a meaningful request, Forge already shows `本轮交付`. First Loop v1 should make this feel more like a product handoff by adding actions where possible:

- 打开预览
- 启动预览
- 创建检查点
- 检查风险
- 继续优化

Actions can load prompts into the input rather than trigger complex workflows. The important part is that the user knows what to do next.

### 4. Actionable Failure States

Common failures should appear as calm cards or inline notices with one next step.

Priority failures:

- Missing API key
- No workspace selected
- Preview cannot start because no dev script exists
- Preview start fails
- Checkpoint creation fails
- Session is stopped and needs resume

Rules:

- Explain what happened in user language.
- Offer one clear action.
- Keep raw technical detail secondary.

## Architecture

### Existing Units To Reuse

- `src/hooks/useSession.ts` already emits missing API key and delivery summary events.
- `src/components/messages/MissingApiKeyCard.tsx` already provides an actionable setup card.
- `src/components/messages/DeliverySummaryCard.tsx` already renders turn closure.
- `src/components/layout/ProjectStatusCard.tsx` already knows preview/checkpoint actions.
- `src/components/session/TaskProgressPopover.tsx` already has task progress placement.
- `src/lib/delivery-confidence.ts` already derives compact delivery labels and actions.
- `src/store/index.ts` already stores first-loop drafts and workflow state.

### New Units

#### `start-readiness.ts`

Purpose:

Derive a small readiness view model from workspace, API key status, runtime status, and checkpoint status.

Outputs:

- rows with label, value, tone, optional action
- primary issue count
- compact title/subtitle

This file should not call IPC. It should be a pure helper for UI and tests.

#### `StartReadinessCard`

Purpose:

Render `准备开始` inside an empty or early session. Keep it compact and dark.

Inputs:

- session id
- active workspace
- selected provider

It may call existing IPC wrappers to fetch API key status, runtime status, and checkpoint status.

#### `first-loop-progress.ts`

Purpose:

Derive the visible phase strip from existing store state and recent blocks.

Outputs:

- ordered phase list
- current phase id
- optional short note

#### `FirstLoopProgressStrip`

Purpose:

Render the five-stage one-line progress strip near `TaskProgressPopover`.

## Testing Strategy

Use focused e2e tests because the value is user-visible.

Minimum tests:

1. Empty first-loop session shows `准备开始`, workspace, model key, preview, and checkpoint rows.
2. Missing API key readiness row offers `打开设置`.
3. First-loop progress strip advances from `理解目标` to a later phase after a request and delivery summary.
4. Delivery summary offers next actions without exposing internal wording.
5. Full frontend build passes.

Rust changes are not required unless a missing backend signal is discovered. Prefer existing IPC.

## Acceptance

Forge is moving in the right direction when:

- a new user can see whether Forge is ready before typing
- the first-loop process feels like making progress, not silent chatting
- the end of a turn gives obvious next actions
- common failures explain what to do next
- no new product layer or internal name appears in the UI

Product sentence:

> First Loop v1 makes Forge feel ready, guided, and recoverable during the first small-tool build.
