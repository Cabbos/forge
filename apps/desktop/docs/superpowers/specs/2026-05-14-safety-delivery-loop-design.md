# Safety Delivery Loop Design

Date: 2026-05-14

Status: design spec

Owner: Forge product direction

## Summary

Forge has landed the first coherent entry loop: choose a workspace, start from one sentence, and shape a small tool toward a previewable first version.

The next loop should make the user trust Forge enough to let it act.

Product name for this loop:

> 安全交付闭环

Core feeling:

> Forge 准备改什么、改在哪、改完能不能看、出问题能不能回来。

This design does not add a new product layer. It strengthens the existing three layers:

- 当前任务: shows what Forge is doing now.
- 项目档案: shows what Forge knows, referenced, and may update.
- 交付: shows whether the result runs and whether there is a safe return point.

## Problem

The first loop can now start work, but the user still lacks confidence at the point where Forge may touch project files.

For a beginner maker, the fear is:

> 我不知道它会不会把我的项目弄坏。

For a professional developer, the concern is:

> I need to know the workspace boundary, file impact, and recovery state before I trust an agent with edits.

The current permission confirmation is too close to an internal tool prompt. Delivery is present, but it does not yet close the loop after actions. Project Archive can show records, but it does not yet clearly summarize what changed after the first prototype attempt.

## Goals

1. Before writes, show a product-level boundary confirmation.
2. During or after writes, keep delivery status compact but useful.
3. After a meaningful step, summarize what happened and what should be remembered.
4. Keep this inside Current Task, Project Archive, and Delivery.
5. Avoid turning the right panel into a dashboard or the confirmation flow into a developer-only permission console.

## Non-Goals

Do not build in this pass:

- full shell command static analysis
- complete file diff review UI
- git restore UI
- document parsing
- new memory system
- new right-panel product layer
- agent marketplace or plugin expansion
- multi-agent orchestration UI

## Considered Approaches

### Approach A: Product-level write boundary first

Add a richer confirmation card for writes and risky commands. Keep delivery compact, then refresh status after the action.

Pros:

- directly addresses trust
- small enough for one implementation pass
- reuses existing permission and delivery IPC
- helps beginners and professionals

Cons:

- file impact may be partial for shell commands
- does not solve full rollback yet

Recommendation: use this approach.

### Approach B: Full delivery dashboard

Build a larger delivery panel with preview, checkpoints, test status, file changes, and history.

Pros:

- powerful for developers
- could become a serious control center later

Cons:

- risks reversing the Project Archive convergence
- right panel becomes status-heavy again
- too much surface before the trust loop is proven

Recommendation: defer.

### Approach C: Completion summary first

Focus only on the end-of-turn summary and Project Archive update prompts.

Pros:

- improves polish quickly
- makes the conversation feel more productized

Cons:

- does not reduce fear before writes
- trust problem remains at the most sensitive moment

Recommendation: do after the write boundary foundation.

## User Experience

### 1. Write Boundary Confirmation

When Forge is about to modify the workspace, the confirmation should read like a product surface, not a raw permission prompt.

User-facing title:

> 准备修改项目

Rows:

- 工作空间: workspace name and compact path
- 操作: create, edit, delete, or run command
- 影响范围: known files or "可能影响当前工作空间"
- 风险: normal, caution, or high
- 恢复点: checkpoint state if available

Primary actions:

- 继续
- 取消

If the active workspace appears to be Forge's own source repository, show a stronger warning:

> 这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。

The card should still use the existing confirmation flow underneath.

### 2. Compact Delivery Confidence

Delivery remains inside Project Archive. It should answer three questions:

1. 预览能不能打开？
2. 最近有没有检查点？
3. 如果失败，下一步是什么？

The compact card should show:

- preview state: running, stopped, unavailable, failed
- primary preview action when available: 打开预览
- checkpoint state: 已就绪, 未创建, 有未保存改动, 不可用
- one plain-language next action

Avoid long logs by default. Logs and detailed status can stay behind "打开交付详情".

### 3. Turn Completion Summary

After a meaningful action, Forge should make the end of the turn feel closed.

The summary can appear as a compact message or Project Archive update proposal:

- 本轮完成了什么
- 当前能否预览
- 是否已有检查点
- 下一步建议
- 是否建议写入项目档案

This should not appear after every tiny tool call. It should appear after a visible delivery attempt, a first-version milestone, or a user-visible failure.

## Architecture

### Existing Surfaces To Reuse

- Backend permission gate emits `ConfirmAsk` and waits for `confirm_response`.
- Frontend already renders confirmation blocks through `ConfirmCard`.
- Project delivery status already flows through project runtime and checkpoint IPC.
- Project Archive already has compact delivery placement.
- Project records already have proposal/update events.

### Proposed Units

#### `WriteBoundary`

Purpose:

Build a normalized description of what Forge is about to do.

Inputs:

- session id
- workspace path
- tool name
- tool input
- permission risk kind
- checkpoint state if available

Outputs:

- title
- operation label
- affected files
- workspace display
- risk label
- caution text
- recovery text

For direct file tools, affected files should be explicit.

For shell commands, v0 should not pretend to know exact file impact. It should show the command and mark impact as possibly workspace-wide unless the existing classifier can confidently identify a narrow action.

#### `WriteBoundaryCard`

Purpose:

Render the confirmation in user-facing product language.

Behavior:

- compact layout
- no raw JSON
- no internal permission terms
- clear workspace path
- stronger warning for Forge source workspace
- reuse `confirm_response`

#### `DeliveryConfidence`

Purpose:

Turn runtime/checkpoint state into one plain-language status.

Inputs:

- project runtime status
- checkpoint status
- active workspace

Outputs:

- preview label
- checkpoint label
- next action
- optional open preview command

#### `TurnClosure`

Purpose:

Create a lightweight end-of-turn summary when the work reaches a meaningful point.

Inputs:

- workflow state
- delivery state
- recent tool outcomes
- first loop draft if present
- project record proposal state

Outputs:

- user-visible summary text or structured card
- optional project record proposal

V0 can start with frontend-friendly display logic and existing backend signals. It does not need a new long-term memory system.

## Event And Data Flow

### Write Confirmation Flow

1. Agent asks to execute a write-capable tool or risky shell command.
2. Permission gate classifies the request.
3. Backend builds `WriteBoundary` from tool input and workspace.
4. Backend emits an enriched confirmation event.
5. Frontend renders `WriteBoundaryCard`.
6. User chooses `继续` or `取消`.
7. Frontend calls `confirm_response`.
8. Backend continues or blocks the action.
9. Project Archive delivery module refreshes after the action.

### Delivery Refresh Flow

1. A meaningful tool action completes.
2. Frontend or backend requests runtime/checkpoint status for the active workspace.
3. Project Archive updates the compact Delivery module.
4. If preview is available, the primary action is `打开预览`.
5. If preview is unavailable, show one concrete next step.

### Turn Closure Flow

1. A first-version attempt, verification step, or risky write finishes.
2. Forge builds a short closure summary.
3. If the closure includes durable project knowledge, Forge proposes a Project Archive update.
4. The user can ignore, edit, or let it become an automatic record depending on existing record rules.

## Error Handling

### Missing Workspace

If no active workspace exists, write tools should not proceed. The user should see:

> 先选择一个项目，Forge 才能修改文件。

### Unknown File Impact

For shell commands or unclear tools, do not invent a file list. Show:

> 这个命令可能影响当前工作空间。

### Checkpoint Unavailable

If checkpoint status cannot be read, do not block all writes. Show:

> 还无法确认恢复点。建议先创建检查点。

High-risk commands may still require stronger confirmation.

### Preview Unavailable

If preview cannot start or open, Delivery should show one reason and one next action, not a long log.

### User Cancels

If the user cancels, the conversation should receive a compact blocked result. Forge should not retry silently.

## UI Copy

Use these user-facing terms:

- 准备修改项目
- 工作空间
- 影响范围
- 风险
- 恢复点
- 继续
- 取消
- 打开预览
- 检查点已就绪
- 还没有检查点
- 建议先创建检查点

Avoid:

- PermissionDecision
- ConfirmAsk
- runtime status
- checkpoint internals
- Project Status
- tool permission

## Acceptance Criteria

The loop is acceptable when:

- a user can see which workspace will be modified before approving a write
- direct file writes show affected files
- shell commands avoid fake precision and show workspace-wide caution when needed
- Forge source workspace gets a stronger warning
- Delivery answers preview and checkpoint state compactly
- after a first-version attempt, the user sees what happened and what the next step is
- no new primary product terms are introduced
- existing Workspace Safety and First Loop tests still pass

## Test Plan

Suggested automated tests:

1. Write confirmation shows workspace name, path, operation, and file impact for file writes.
2. Shell confirmation shows command and workspace-wide caution.
3. Forge-source workspace warning appears when active workspace matches Forge source.
4. Canceling confirmation does not execute the write and shows a blocked result.
5. Delivery module shows preview running with `打开预览`.
6. Delivery module shows checkpoint unavailable with a plain next action.
7. First-loop completion summary appears after a meaningful delivery attempt.
8. Product language scan confirms no internal terms appear in primary UI.

Suggested commands:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npx playwright test e2e/frontend.spec.ts
```

## Implementation Notes For Later

Keep the implementation small:

- extend existing confirmation event shape only if needed
- prefer deriving display metadata near the permission boundary
- keep `ConfirmCard` compatibility for older confirmation events
- do not add a new delivery dashboard
- do not create project-local files as part of this loop
- keep Project Archive as the right-panel home for delivery confidence

## Open Decisions

1. Whether `WriteBoundary` should be a new event or an enriched `ConfirmAsk`.
2. Whether checkpoint state should be fetched synchronously before confirmation or shown as best-effort async metadata.
3. Whether turn closure should be a chat block, a Project Archive card, or both.

Recommended defaults:

- enrich `ConfirmAsk` to reduce event sprawl
- fetch checkpoint state best-effort
- start with a compact chat closure plus optional Project Archive proposal
