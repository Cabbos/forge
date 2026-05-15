# Project Archive v1 Design

## Product Goal

Forge should help a user return to a project without feeling like they are starting from a blank chat again.

The product feeling is:

> I made a first version yesterday. Today Forge can tell me what this project is, where it stopped, and how to continue.

This is not a new knowledge system. It is a clearer surface for the existing project archive, first-loop draft, delivery summary, saved background, and persisted conversation blocks.

## User Language

Use product-facing Chinese names:

- 项目档案
- 项目概览
- 当前版本
- 下一步
- 继续上次任务
- 检查当前版本
- 继续优化

Avoid internal terms in primary UI:

- Workflow Router
- Task Mode
- Living Wiki
- Forge Wiki
- Context Activation
- Memory
- Project Status

## Scope

Build a lightweight Project Overview card at the top of the right-side 项目档案 panel.

The card should show:

- project name from the active workspace or active session path
- current project goal, preferably from the first-loop draft or latest user request
- current version, preferably from the latest delivery summary or first-loop scope
- next step, preferably from the latest delivery summary or first-loop next step
- compact continuation actions:
  - 继续上次任务
  - 检查当前版本
  - 继续优化

Clicking a continuation action should place a useful prompt into the input box. It should not send automatically.

## Architecture

Add a pure derivation helper:

- `src/lib/project-archive-overview.ts`

It derives a view model from existing local state:

- active workspace
- active session
- persisted `BlockState[]`
- current `FirstLoopDraft | null`

Add a small UI component:

- `src/components/context/ProjectOverviewCard.tsx`

It renders the view model and uses `useStore().setPendingInput()` for continuation actions.

Wire it into:

- `src/components/layout/HubPanel.tsx`

The card should sit near the top of 项目档案, before 当前任务 and 第一版, because it is the returning user's entry point.

## Data Flow

Inputs:

1. Workspace/session path gives project identity.
2. `firstLoopDraftBySession` gives live first-loop goal/scope/next step.
3. Persisted blocks provide continuity after reload:
   - latest `user_message`
   - latest `delivery_summary`

Fallback order:

Goal:

1. first-loop draft goal
2. latest first-loop-like user message, converted to draft by `deriveFirstLoopDraft`
3. latest user message
4. `等待你描述这个项目要做什么。`

Current version:

1. latest delivery summary preview/checkpoint labels
2. first-loop draft scope
3. `还没有形成可验收版本`

Next step:

1. latest delivery summary next action
2. first-loop draft next step
3. `描述一个小工具，Forge 会先推进到可预览第一版。`

## Error Handling

This feature should not call new backend APIs, so backend failure should not affect it.

If data is missing, show calm fallback copy instead of an empty card.

## Testing

Add e2e coverage for:

1. A restored session with persisted blocks shows 项目概览 in 项目档案.
2. The overview shows current version and next step from delivery summary.
3. Clicking 继续上次任务 places a continuation prompt into the input box.

Existing full e2e should continue passing.

## Non-Goals

- Do not implement document uploads.
- Do not implement semantic memory retrieval.
- Do not write project archive files.
- Do not add a new product concept or sidebar section.
- Do not auto-send continuation prompts.
