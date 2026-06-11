# Forge Internal A2A Runtime Design Spec

**Date**: 2026-06-11
**Status**: Ready for implementation planning
**Scope**: Internal A2A control plane for Forge subagents, designed to evolve into worktree workers

---

## Overview

Forge already has `delegate_task` and a read-only `SubAgent`, but the current implementation is closer to a parallel research tool than a real A2A runtime. The model calls `delegate_task`, the backend spawns an ephemeral child loop, and the final JSON is returned as a tool result.

The next architecture should keep `delegate_task` as the model-facing entrypoint while promoting the backend behavior into an internal A2A runtime:

```text
delegate_task -> AgentSupervisor -> AgentBus -> ChildAgentRuntime
              -> AgentA2AProjection -> StreamEvent -> frontend timeline
              -> GoalLedger / snapshot / resume
              -> structured result back to parent model
```

The first implementation phase should build the control plane, not the full worker swarm. The final destination is still a worktree-worker system where child agents can implement candidate patches in isolated worktrees, run tests, and return artifacts for supervisor review. Phase 1 must reserve that shape in the data model without enabling direct writes yet.

## Product Principle

User-facing language should avoid "A2A", "swarm", "runtime", and "mailbox" unless the user opens a developer/debug surface. The visible product concept is:

- 子任务
- 并行检查
- 后台研究
- 候选方案

The user should understand that Forge is still working and what each child task is doing. The user should not be asked to manage an agent graph.

## Goals

- Give child agents durable identity: `agent_id`, `task_id`, role, execution mode, status, timestamps, budget, permissions, and artifacts.
- Introduce an internal message bus that records child-task lifecycle messages and can generate a stable projection.
- Wrap existing `delegate_task` execution in an `AgentSupervisor` while preserving the current parent model contract.
- Emit a structured `agent_a2a_updated` stream event so the frontend does not parse tool-result JSON for subagent state.
- Persist A2A state in session snapshots and normalize running tasks on resume.
- Link child tasks to `GoalLedger` so long-running goals can see child work.
- Reserve `WorktreeWorker` as a future execution mode while keeping Phase 1 read-only.

## Non-Goals

- Do not allow child agents to write the parent workspace in Phase 1.
- Do not allow recursive child delegation in Phase 1.
- Do not implement automatic patch merge in Phase 1.
- Do not introduce external A2A protocol compatibility.
- Do not create a cloud worker pool.
- Do not redesign the full chat UI around agent graphs.

## Final Direction

The long-term target is the strongest version:

```text
Supervisor
  -> planner / researcher / implementer / reviewer / tester
  -> isolated worktree workers
  -> diff / commit / test report artifacts
  -> supervisor review
  -> user or parent-agent confirmation
  -> selected patch applied to the real workspace
```

Phase 1 is intentionally smaller:

```text
Supervisor
  -> read-only researcher / reviewer / test_planner child tasks
  -> lifecycle messages and evidence
  -> projection, snapshot, GoalLedger
  -> structured result returned to parent model
```

This makes Phase 1 the control plane for the final worktree-worker system.

## Current State

Relevant current files:

- `apps/desktop/src-tauri/src/agent/sub.rs`
  - Defines the current lightweight `SubAgent`.
  - Runs read-only tools.
  - Returns JSON with `result` and `steps`.
- `apps/desktop/src-tauri/src/agent/session.rs`
  - Detects `delegate_task`.
  - Spawns subagents concurrently.
  - Emits ordinary tool call events.
  - Returns text result back into model tool results.
- `apps/desktop/src-tauri/src/adapters/anthropic.rs`
  - Defines `delegate_task`.
  - Filters dangerous tools for subagents.
- `apps/desktop/src/components/messages/SubAgentTrace.tsx`
  - Parses delegate result JSON and displays a small trace.
- `apps/desktop/src-tauri/src/agent/goal_state.rs`
  - Stores `GoalLedger`.
  - Already persists through session snapshots.
- `apps/desktop/src-tauri/src/agent/snapshot.rs`
  - Persists session state and `goal_ledger`.
- `apps/desktop/src-tauri/src/protocol/events.rs`
  - Backend stream event source of truth.
- `apps/desktop/src/lib/protocol.ts`
  - TypeScript mirror of backend stream events.
- `apps/desktop/src/store/event-dispatch.ts`
  - Central frontend stream event reducer.

## Architecture

### Core Modules

Create a new backend module:

```text
apps/desktop/src-tauri/src/agent/a2a/
  mod.rs
  types.rs
  bus.rs
  supervisor.rs
  child.rs
  projection.rs
```

Responsibilities:

- `types.rs`: stable serializable types.
- `bus.rs`: append-only task/message state and projection generation.
- `supervisor.rs`: dispatch, cancel, timeout, retry, and result summarization.
- `child.rs`: child runtime wrapper around the existing `SubAgent` behavior.
- `projection.rs`: compact frontend/snapshot view.

### Data Model

The data model must support current and future execution:

```text
AgentExecutionMode:
  - read_only
  - patch_proposal
  - worktree_worker
```

Phase 1 executes only `read_only`. `patch_proposal` can be represented as an artifact but does not write files. `worktree_worker` is reserved and must not be available through model tool calls yet.

### Permissions

Phase 1 child permissions:

```text
read_file: allowed
search_content: allowed
search_files: allowed
list_directory: allowed
web_search: allowed
web_fetch: allowed
git_diff: allowed
write_to_file: blocked
edit_file: blocked
run_shell / bash: blocked
delegate_task: blocked
```

When `WorktreeWorker` is implemented later, writes and shell commands are allowed only inside an isolated worktree lease and must produce artifacts for review.

### Bus Messages

Use explicit lifecycle messages:

- `task_assigned`
- `started`
- `progress`
- `evidence`
- `artifact_created`
- `final_result`
- `failed`
- `cancelled`
- `interrupted`

The bus should keep the raw message list and generate a compact projection for UI.

### Stream Events

Add a new event:

```text
agent_a2a_updated
```

Payload:

```text
session_id: String
state: AgentA2AProjection
```

The frontend should store the projection by `session_id`.

### Snapshot And Resume

`AgentSessionSnapshot` should gain:

```text
a2a_state: Option<AgentA2AState>
```

On resume:

- `running` tasks become `interrupted`.
- `pending` tasks remain `pending`.
- `completed`, `failed`, and `cancelled` stay terminal.
- A resume note records that the session was restored before the child task completed.

Phase 1 does not auto-restart interrupted child tasks. The parent model receives context that the child task was interrupted and can decide whether to re-dispatch.

### GoalLedger Integration

When a child task is dispatched during an active goal:

- Create or associate a goal task.
- Mark it `in_progress` when the child starts.
- Mark it `completed` when the child returns a final result.
- Leave failure metadata in A2A state; current `GoalTaskStatus` does not yet include `failed`.
- On resume, GoalLedger already normalizes in-progress tasks back to pending. A2A state should mirror that interruption.

Do not expand GoalLedger aggressively in Phase 1 unless required. If `failed` task status becomes necessary, introduce it in a separate small change with migration tests.

## Frontend

Phase 1 frontend should be compact:

- Add protocol types for `AgentA2AProjection`.
- Store `agentA2ABySession`.
- Render a compact A2A timeline component near the agent turn / tool evidence area.
- Keep current `SubAgentTrace` support for backward compatibility.

Default view:

- child task title
- role
- status
- latest progress
- final result preview
- failure message if failed

Developer details can show:

- `agent_id`
- `task_id`
- execution mode
- permission summary
- artifacts
- raw messages

## Error Handling

Child failures should not disappear into a generic tool result.

Failures must include:

- failure kind
- user-visible message
- retryable flag
- recovery advice when possible
- task status transition
- projection update

If a child runtime panics or joins with an error, supervisor records a failed task and still returns a structured result to the parent model.

## Testing Strategy

Backend tests:

- A2A types serialize with snake_case statuses and execution modes.
- Bus records lifecycle messages and generates projection.
- Resume normalization turns running child tasks into interrupted.
- Snapshot roundtrip preserves A2A state.
- Supervisor wraps a read-only child task and records started/final messages.
- Failed child join records failed task and returns a parent-model result.
- `delegate_task` still returns a valid tool result to the parent model.

Frontend tests:

- Protocol accepts `agent_a2a_updated`.
- Store records `agentA2ABySession`.
- Timeline renders running, completed, failed, and interrupted tasks.
- Legacy `SubAgentTrace` still renders existing delegate JSON.

Regression checks:

```bash
npm run check:ci
npm --prefix apps/desktop run check:backend
npm --prefix apps/desktop run eval:forge:test
npm run eval:report:latest -- --failures
npm run eval:forge:smoke:real -- --dry-run
```

## Rollout

Phase 1 should preserve model-visible behavior:

- The model still sees a `delegate_task` tool result.
- Users get additional A2A timeline state.
- Existing evals should not need prompt rewrites.

After Phase 1:

- Add structured `PatchProposal` artifacts.
- Add isolated `WorktreeWorker`.
- Add supervisor review and merge recommendations.

## Open Decisions

- Whether `AgentA2AProjection` should live inside `AgentTurnState` or as a separate store map. Recommendation: separate event/store map in Phase 1 to minimize churn, then optionally fold into `AgentTurnState`.
- Whether GoalLedger needs `Failed` task status. Recommendation: defer unless A2A failure display becomes confusing.
- Whether frontend timeline belongs in the message stream or side inspector. Recommendation: start near tool evidence; move to inspector only when it gets visually noisy.
