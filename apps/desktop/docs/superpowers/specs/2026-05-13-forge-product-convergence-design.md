# Forge Product Convergence - Design Spec

## Product Definition

Forge is an AI workbench for building personal tools with project memory.

At this stage, Forge should not feel like a collection of internal systems. Users should feel three things:

1. Forge knows what I am trying to do now.
2. Forge remembers useful background from this project.
3. Forge can move the work toward something I can preview, check, and continue.

## Three User-Facing Layers

### 1. Current Task

User question: "What is Forge doing for me right now?"

User-facing names:

- 当前任务
- 工作方式
- 继续会话
- 直接回答
- 梳理想法
- 拆成步骤
- 开始制作
- 排查问题
- 检查结果

Internal concepts hidden here:

- Workflow Router
- Task Mode
- route
- phase
- workflow gate
- resume state

Developer-only details may remain behind "开发者详情".

### 2. Context

User question: "What background is Forge using this turn?"

User-facing names:

- 上下文
- 本轮上下文
- 项目记录
- 已保存背景
- 个人偏好
- 资料
- 建议更新记录

Internal concepts hidden here:

- Living Wiki
- Forge Wiki
- Memory
- Context Activation
- RAG
- selected context

This layer should stay visible and controllable. Forge may suggest context, but the user must be able to see whether it was used.

### 3. Delivery

User question: "Can I see, check, or safely continue the result?"

User-facing names:

- 交付
- 预览
- 检查点
- 检查结果
- 需要确认
- 最近状态

Internal concepts hidden here:

- ProjectDashboard
- checkpoint id
- tool call
- IPC
- auto compact internals

This layer should make the work feel grounded. It should show whether the preview is running, whether a checkpoint exists, and whether recent checks are ready.

## Information Architecture

The right-side panel becomes a compact workbench:

1. 当前任务
2. 上下文
   - 本轮上下文
   - 项目记录
   - 建议更新记录
   - 已保存背景
   - 资料 placeholder
3. 交付
   - preview status
   - checkpoint status
   - model/context budget metadata

The panel title should become "工作台". "上下文" remains a layer inside the panel, not the whole product surface.

## Scope

Now:

- Rename visible UI copy to the three-layer language.
- Reorder and group the right panel around 当前任务 / 上下文 / 交付.
- Hide internal vocabulary from primary UI.
- Keep existing backend, IPC, store, memory, wiki, workflow, resume, and compact behavior unchanged.
- Keep the resources section as a placeholder only.

Not now:

- No resource upload implementation.
- No new memory model.
- No new workflow route.
- No new provider capability.
- No new auto compact or resume backend behavior.
- No vector database or RAG layer.

Later:

- Resource upload can become one source under 上下文.
- Resume can become stronger under 当前任务.
- Auto compact can become "自动整理上下文" under developer details or delivery notes.

## Acceptance Criteria

1. A non-technical user can scan the main UI and see 当前任务, 上下文, and 交付 as the primary product model.
2. Primary UI no longer says Forge Wiki, Living Wiki, Memory, Task Mode, Workflow Router, Context Activation, route, or phase.
3. Existing behavior remains unchanged.
4. Focused e2e tests cover the renamed panel structure and existing context/task behavior.
