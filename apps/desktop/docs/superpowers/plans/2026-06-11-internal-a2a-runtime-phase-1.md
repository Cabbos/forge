# Forge Internal A2A Runtime Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote Forge's existing `delegate_task` subagent path into a structured internal A2A control plane with task identity, lifecycle messages, projection events, snapshot persistence, and GoalLedger linkage while keeping child agents read-only.

**Architecture:** Add `agent/a2a/` as a focused backend module. Keep `delegate_task` as the model-facing tool, but route it through `AgentSupervisor` and `AgentA2ABus`. Emit `agent_a2a_updated` projections to the frontend and persist A2A state in session snapshots. Reserve `WorktreeWorker` in the data model for the final multi-worktree direction, but do not enable workspace writes in Phase 1.

**Tech Stack:** Rust/Tokio/Tauri backend, serde, existing `AiAdapter` and `Harness`, React/TypeScript frontend, Zustand store, existing Forge eval and backend checks.

---

## Scope

This phase implements the A2A control plane only:

- A2A Rust types, bus, projection, and supervisor shell.
- `delegate_task` routed through supervisor while preserving current parent model behavior.
- `agent_a2a_updated` Rust and TypeScript protocol event.
- Frontend store and compact timeline rendering.
- Snapshot roundtrip and resume normalization.
- GoalLedger best-effort child task association.

This phase does not implement:

- child agent writes to the parent workspace
- isolated worktree workers
- automatic patch merge
- recursive child delegation
- external A2A protocol compatibility

## Pre-Flight

- [ ] **Step 1: Confirm branch and cleanliness**

Run:

```bash
cd /Users/cabbos/project/forge
git status --short --branch
git branch --show-current
```

Expected:

```text
## cabbos/internal-a2a-runtime-plan
```

or a clean feature branch created from `origin/main`.

- [ ] **Step 2: Run GitNexus impact before editing symbols**

Run impact for the symbols that will be edited:

```text
gitnexus impact upstream AgentSession
gitnexus impact upstream AgentSessionSnapshot
gitnexus impact upstream StreamEvent
```

If impact is HIGH or CRITICAL, report the blast radius before editing. `AgentSession` is expected to be high-risk because it is the core loop.

## File Structure

- Create: `apps/desktop/src-tauri/src/agent/a2a/mod.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/types.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/projection.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session.rs`
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/agent/goal_state.rs` only if association helpers are needed
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/store/types.ts`
- Modify: `apps/desktop/src/store/event-dispatch.ts`
- Create: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Add tests in the closest existing test modules:
  - `apps/desktop/src-tauri/src/agent/a2a/*`
  - `apps/desktop/src-tauri/src/agent/snapshot.rs`
  - `apps/desktop/src-tauri/src/agent/session_tests.rs`
  - frontend `.test.mjs` files near existing component/store tests

## Task 1: Add A2A Core Types

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/a2a/mod.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/types.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`

- [ ] **Step 1: Write failing serialization tests**

Create `apps/desktop/src-tauri/src/agent/a2a/types.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_mode_serializes_snake_case_and_reserves_worktree_worker() {
        let mode = AgentExecutionMode::WorktreeWorker;

        let json = serde_json::to_string(&mode).expect("serialize mode");

        assert_eq!(json, r#""worktree_worker""#);
    }

    #[test]
    fn task_record_defaults_to_read_only_permissions() {
        let record = AgentTaskRecord::new(
            AgentTaskId::new("task-1"),
            AgentId::new("agent-1"),
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect compact flow",
            "Find where compact triggers",
            10,
        );

        assert_eq!(record.status, AgentTaskStatus::Pending);
        assert_eq!(record.permissions.execution_mode, AgentExecutionMode::ReadOnly);
        assert!(record.permissions.allow_read_files);
        assert!(!record.permissions.allow_workspace_write);
        assert!(!record.permissions.allow_shell);
        assert!(!record.permissions.allow_delegate);
    }
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
npm --prefix apps/desktop run check:backend -- --skip-clippy
```

If `check:backend` does not support `--skip-clippy`, run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::types --lib
```

Expected: failure because the module/types do not exist yet.

- [ ] **Step 3: Implement core types**

Implement `apps/desktop/src-tauri/src/agent/a2a/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AgentTaskId(String);

impl AgentTaskId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AgentId(String);

impl AgentId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentRole {
    Researcher,
    Reviewer,
    TestPlanner,
    Implementer,
}

impl AgentRole {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Researcher => "researcher",
            Self::Reviewer => "reviewer",
            Self::TestPlanner => "test_planner",
            Self::Implementer => "implementer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentExecutionMode {
    ReadOnly,
    PatchProposal,
    WorktreeWorker,
}

impl AgentExecutionMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::PatchProposal => "patch_proposal",
            Self::WorktreeWorker => "worktree_worker",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl AgentTaskStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentArtifactKind {
    Evidence,
    PatchProposal,
    TestReport,
    DiffSummary,
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPermissionSet {
    pub execution_mode: AgentExecutionMode,
    pub allow_read_files: bool,
    pub allow_web: bool,
    pub allow_git_diff: bool,
    pub allow_workspace_write: bool,
    pub allow_shell: bool,
    pub allow_delegate: bool,
}

impl AgentPermissionSet {
    pub(crate) fn for_mode(execution_mode: AgentExecutionMode) -> Self {
        match execution_mode {
            AgentExecutionMode::ReadOnly => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: false,
                allow_shell: false,
                allow_delegate: false,
            },
            AgentExecutionMode::PatchProposal => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: false,
                allow_shell: false,
                allow_delegate: false,
            },
            AgentExecutionMode::WorktreeWorker => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: true,
                allow_shell: true,
                allow_delegate: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentArtifact {
    pub artifact_id: String,
    pub task_id: AgentTaskId,
    pub kind: AgentArtifactKind,
    pub title: String,
    pub content: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentTaskFailure {
    pub kind: String,
    pub message: String,
    pub retryable: bool,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentTaskRecord {
    pub task_id: AgentTaskId,
    pub parent_task_id: Option<AgentTaskId>,
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub execution_mode: AgentExecutionMode,
    pub title: String,
    pub prompt: String,
    pub status: AgentTaskStatus,
    pub permissions: AgentPermissionSet,
    pub artifacts: Vec<AgentArtifact>,
    pub failure: Option<AgentTaskFailure>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub resume_note: Option<String>,
}

impl AgentTaskRecord {
    pub(crate) fn new(
        task_id: AgentTaskId,
        agent_id: AgentId,
        role: AgentRole,
        execution_mode: AgentExecutionMode,
        title: impl Into<String>,
        prompt: impl Into<String>,
        timestamp_ms: u64,
    ) -> Self {
        let permissions = AgentPermissionSet::for_mode(execution_mode.clone());
        Self {
            task_id,
            parent_task_id: None,
            agent_id,
            role,
            execution_mode,
            title: title.into(),
            prompt: prompt.into(),
            status: AgentTaskStatus::Pending,
            permissions,
            artifacts: Vec::new(),
            failure: None,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
            started_at_ms: None,
            ended_at_ms: None,
            resume_note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentMessageKind {
    TaskAssigned,
    Started,
    Progress,
    Evidence,
    ArtifactCreated,
    FinalResult,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentMessage {
    pub message_id: String,
    pub task_id: AgentTaskId,
    pub agent_id: AgentId,
    pub kind: AgentMessageKind,
    pub content: String,
    pub created_at_ms: u64,
}
```

Create `apps/desktop/src-tauri/src/agent/a2a/mod.rs`:

```rust
pub(crate) mod bus;
pub(crate) mod child;
pub(crate) mod projection;
pub(crate) mod supervisor;
pub(crate) mod types;
```

Modify `apps/desktop/src-tauri/src/agent/mod.rs`:

```rust
pub(crate) mod a2a;
```

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::types --lib
```

Expected: type tests pass.

## Task 2: Add Bus And Projection

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/projection.rs`

- [ ] **Step 1: Write bus tests**

Create tests in `bus.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

    #[test]
    fn bus_records_lifecycle_and_projection() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect session loop",
            "Find where delegate_task is executed",
            10,
        );

        bus.start_task(&task_id, 20);
        bus.record_progress(&task_id, "Reading session.rs", 30);
        bus.complete_task(&task_id, "delegate_task is split before regular tools", 40);

        let projection = bus.projection();

        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.running_count, 0);
        assert_eq!(projection.completed_count, 1);
        assert_eq!(projection.tasks[0].status, "completed");
        assert_eq!(
            projection.tasks[0].latest_message.as_deref(),
            Some("delegate_task is split before regular tools")
        );
    }

    #[test]
    fn resume_normalization_interrupts_running_tasks() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Review compact behavior",
            "Check compact edge cases",
            10,
        );
        bus.start_task(&task_id, 20);

        bus.normalize_for_resume(30);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.status, crate::agent::a2a::types::AgentTaskStatus::Interrupted);
        assert_eq!(
            task.resume_note.as_deref(),
            Some("child task was running when the session was restored")
        );
    }
}
```

- [ ] **Step 2: Implement projection types**

Create `projection.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentA2AProjection {
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub interrupted_count: usize,
    pub tasks: Vec<AgentA2ATaskProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentA2ATaskProjection {
    pub task_id: String,
    pub agent_id: String,
    pub role: String,
    pub execution_mode: String,
    pub status: String,
    pub title: String,
    pub latest_message: Option<String>,
    pub failure_message: Option<String>,
    pub updated_at_ms: u64,
}
```

- [ ] **Step 3: Implement bus**

Create `bus.rs`:

```rust
use crate::agent::a2a::projection::{AgentA2AProjection, AgentA2ATaskProjection};
use crate::agent::a2a::types::{
    AgentExecutionMode, AgentId, AgentMessage, AgentMessageKind, AgentRole, AgentTaskFailure,
    AgentTaskId, AgentTaskRecord, AgentTaskStatus,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ABus {
    pub tasks: Vec<AgentTaskRecord>,
    pub messages: Vec<AgentMessage>,
    next_task_index: u64,
    next_agent_index: u64,
    next_message_index: u64,
}

impl AgentA2ABus {
    pub(crate) fn assign_task(
        &mut self,
        role: AgentRole,
        execution_mode: AgentExecutionMode,
        title: impl Into<String>,
        prompt: impl Into<String>,
        timestamp_ms: u64,
    ) -> AgentTaskId {
        self.next_task_index += 1;
        self.next_agent_index += 1;
        let task_id = AgentTaskId::new(format!("a2a-task-{}", self.next_task_index));
        let agent_id = AgentId::new(format!("a2a-agent-{}", self.next_agent_index));
        let title = title.into();
        let prompt = prompt.into();
        let record = AgentTaskRecord::new(
            task_id.clone(),
            agent_id.clone(),
            role,
            execution_mode,
            title.clone(),
            prompt,
            timestamp_ms,
        );
        self.tasks.push(record);
        self.push_message(task_id.clone(), agent_id, AgentMessageKind::TaskAssigned, title, timestamp_ms);
        task_id
    }

    pub(crate) fn task(&self, task_id: &AgentTaskId) -> Option<&AgentTaskRecord> {
        self.tasks.iter().find(|task| task.task_id == *task_id)
    }

    pub(crate) fn start_task(&mut self, task_id: &AgentTaskId, timestamp_ms: u64) {
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Running;
            task.started_at_ms = Some(timestamp_ms);
            task.resume_note = None;
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(task_id.clone(), agent_id, AgentMessageKind::Started, "Started".to_string(), timestamp_ms);
    }

    pub(crate) fn record_progress(&mut self, task_id: &AgentTaskId, message: impl Into<String>, timestamp_ms: u64) {
        let Some(agent_id) = self.task(task_id).map(|task| task.agent_id.clone()) else {
            return;
        };
        self.push_message(task_id.clone(), agent_id, AgentMessageKind::Progress, message.into(), timestamp_ms);
    }

    pub(crate) fn complete_task(&mut self, task_id: &AgentTaskId, result: impl Into<String>, timestamp_ms: u64) {
        let result = result.into();
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Completed;
            task.ended_at_ms = Some(timestamp_ms);
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(task_id.clone(), agent_id, AgentMessageKind::FinalResult, result, timestamp_ms);
    }

    pub(crate) fn fail_task(
        &mut self,
        task_id: &AgentTaskId,
        kind: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
        timestamp_ms: u64,
    ) {
        let kind = kind.into();
        let message = message.into();
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Failed;
            task.ended_at_ms = Some(timestamp_ms);
            task.failure = Some(AgentTaskFailure {
                kind: kind.clone(),
                message: message.clone(),
                retryable,
                created_at_ms: timestamp_ms,
            });
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(task_id.clone(), agent_id, AgentMessageKind::Failed, message, timestamp_ms);
    }

    pub(crate) fn normalize_for_resume(&mut self, timestamp_ms: u64) {
        let mut interrupted = Vec::new();
        for task in &mut self.tasks {
            if task.status == AgentTaskStatus::Running {
                task.status = AgentTaskStatus::Interrupted;
                task.resume_note = Some("child task was running when the session was restored".to_string());
                task.updated_at_ms = timestamp_ms;
                task.ended_at_ms = Some(timestamp_ms);
                interrupted.push((task.task_id.clone(), task.agent_id.clone()));
            }
        }
        for (task_id, agent_id) in interrupted {
            self.push_message(
                task_id,
                agent_id,
                AgentMessageKind::Interrupted,
                "Child task was interrupted by session restore".to_string(),
                timestamp_ms,
            );
        }
    }

    pub(crate) fn projection(&self) -> AgentA2AProjection {
        let mut projection = AgentA2AProjection::default();
        projection.tasks = self
            .tasks
            .iter()
            .map(|task| {
                match task.status {
                    AgentTaskStatus::Running => projection.running_count += 1,
                    AgentTaskStatus::Completed => projection.completed_count += 1,
                    AgentTaskStatus::Failed => projection.failed_count += 1,
                    AgentTaskStatus::Interrupted => projection.interrupted_count += 1,
                    AgentTaskStatus::Pending | AgentTaskStatus::Cancelled => {}
                }
                AgentA2ATaskProjection {
                    task_id: task.task_id.as_str().to_string(),
                    agent_id: task.agent_id.as_str().to_string(),
                    role: task.role.as_str().to_string(),
                    execution_mode: task.execution_mode.as_str().to_string(),
                    status: task.status.as_str().to_string(),
                    title: task.title.clone(),
                    latest_message: self.latest_message_for(&task.task_id),
                    failure_message: task.failure.as_ref().map(|failure| failure.message.clone()),
                    updated_at_ms: task.updated_at_ms,
                }
            })
            .collect();
        projection
    }

    fn latest_message_for(&self, task_id: &AgentTaskId) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.task_id == *task_id)
            .map(|message| message.content.clone())
    }

    fn update_task<T>(
        &mut self,
        task_id: &AgentTaskId,
        timestamp_ms: u64,
        update: impl FnOnce(&mut AgentTaskRecord) -> T,
    ) -> Option<T> {
        let task = self.tasks.iter_mut().find(|task| task.task_id == *task_id)?;
        task.updated_at_ms = timestamp_ms;
        Some(update(task))
    }

    fn push_message(
        &mut self,
        task_id: AgentTaskId,
        agent_id: AgentId,
        kind: AgentMessageKind,
        content: String,
        timestamp_ms: u64,
    ) {
        self.next_message_index += 1;
        self.messages.push(AgentMessage {
            message_id: format!("a2a-message-{}", self.next_message_index),
            task_id,
            agent_id,
            kind,
            content,
            created_at_ms: timestamp_ms,
        });
    }
}
```

Do not add dependencies for enum string conversion. Use the `as_str()` helpers from Task 1.

- [ ] **Step 4: Run bus tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus --lib
```

Expected: pass.

## Task 3: Persist A2A State In Snapshots

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`

- [ ] **Step 1: Write snapshot tests**

Add tests to `snapshot.rs`:

```rust
#[test]
fn old_snapshot_json_without_a2a_state_deserializes() {
    let json = r#"{
      "session_id":"s1",
      "provider":"anthropic",
      "model":"claude",
      "working_dir":"/tmp/project",
      "messages":[],
      "summary":null,
      "context_window_tokens":200000,
      "updated_at_ms":10
    }"#;

    let restored: AgentSessionSnapshot = serde_json::from_str(json)
        .expect("old snapshot should deserialize without a2a_state");

    assert!(restored.a2a_state.is_none());
}

#[test]
fn snapshot_with_a2a_state_roundtrips() {
    use crate::agent::a2a::bus::AgentA2ABus;
    use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

    let mut bus = AgentA2ABus::default();
    let task_id = bus.assign_task(
        AgentRole::Researcher,
        AgentExecutionMode::ReadOnly,
        "Inspect A2A",
        "Read A2A files",
        10,
    );
    bus.start_task(&task_id, 20);
    bus.complete_task(&task_id, "done", 30);

    let snapshot = snapshot().with_a2a_state(bus);
    let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let restored: AgentSessionSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");

    assert_eq!(restored.a2a_state.expect("a2a state").tasks.len(), 1);
}
```

- [ ] **Step 2: Modify snapshot struct**

In `AgentSessionSnapshot`, add:

```rust
use crate::agent::a2a::bus::AgentA2ABus;
```

Add field:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub a2a_state: Option<AgentA2ABus>,
```

Initialize to `None` in `new()`.

Add builder:

```rust
pub fn with_a2a_state(mut self, a2a_state: AgentA2ABus) -> Self {
    self.a2a_state = Some(a2a_state);
    self.updated_at_ms = now_ms();
    self
}
```

- [ ] **Step 3: Add session storage field**

In `AgentSession`, add:

```rust
pub(crate) a2a_bus: Mutex<crate::agent::a2a::bus::AgentA2ABus>,
```

Initialize in `new()`:

```rust
a2a_bus: Mutex::new(crate::agent::a2a::bus::AgentA2ABus::default()),
```

In `snapshot()`, include:

```rust
let a2a_state = lock_unpoisoned(&self.a2a_bus).clone();
if !a2a_state.tasks.is_empty() || !a2a_state.messages.is_empty() {
    snapshot = snapshot.with_a2a_state(a2a_state);
}
```

- [ ] **Step 4: Restore and resume state**

Extend `restore_state` signature to accept:

```rust
a2a_state: Option<crate::agent::a2a::bus::AgentA2ABus>,
```

Inside restore:

```rust
*lock_unpoisoned(&self.a2a_bus) = a2a_state.unwrap_or_default();
```

In resume normalization:

```rust
lock_unpoisoned(&self.a2a_bus).normalize_for_resume(now_ms());
```

Update `ipc/session_lifecycle.rs` restore call sites to pass `snapshot.a2a_state`.

- [ ] **Step 5: Run snapshot/session tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml snapshot --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml restore_state --lib
```

Expected: pass.

## Task 4: Add StreamEvent And Frontend Protocol

**Files:**
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/store/types.ts`
- Modify: `apps/desktop/src/store/event-dispatch.ts`

- [ ] **Step 1: Add Rust event**

In `protocol/events.rs`, import:

```rust
use crate::agent::a2a::projection::AgentA2AProjection;
```

Add to `StreamEvent`:

```rust
#[serde(rename = "agent_a2a_updated")]
AgentA2AUpdated {
    session_id: String,
    state: AgentA2AProjection,
},
```

- [ ] **Step 2: Add TypeScript mirror**

In `src/lib/protocol.ts`, add:

```ts
export interface AgentA2ATaskProjection {
  task_id: string;
  agent_id: string;
  role: string;
  execution_mode: string;
  status: string;
  title: string;
  latest_message: string | null;
  failure_message: string | null;
  updated_at_ms: number;
}

export interface AgentA2AProjection {
  running_count: number;
  completed_count: number;
  failed_count: number;
  interrupted_count: number;
  tasks: AgentA2ATaskProjection[];
}
```

Add union member:

```ts
| {
    event_type: "agent_a2a_updated";
    session_id: string;
    state: AgentA2AProjection;
  }
```

- [ ] **Step 3: Add store state**

In `src/store/types.ts`, add:

```ts
agentA2ABySession: Map<string, AgentA2AProjection>;
```

Initialize it wherever the store initial state is built.

- [ ] **Step 4: Handle event dispatch**

In `event-dispatch.ts`, before generic block handling:

```ts
if (event_type === "agent_a2a_updated") {
  const agentA2ABySession = new Map(get().agentA2ABySession);
  agentA2ABySession.set(session_id, event.state);
  set({ agentA2ABySession });
  return;
}
```

- [ ] **Step 5: Add a frontend reducer test**

Use the existing frontend test style. Create or extend a store/event-dispatch test to assert:

```ts
dispatchOutputEvent({
  event_type: "agent_a2a_updated",
  session_id: "s1",
  state: {
    running_count: 1,
    completed_count: 0,
    failed_count: 0,
    interrupted_count: 0,
    tasks: [{
      task_id: "a2a-task-1",
      agent_id: "a2a-agent-1",
      role: "researcher",
      execution_mode: "read_only",
      status: "running",
      title: "Inspect session",
      latest_message: "Reading files",
      failure_message: null,
      updated_at_ms: 10,
    }],
  },
});
```

Expected: `agentA2ABySession.get("s1")?.running_count === 1`.

- [ ] **Step 6: Run frontend checks**

Run the narrow test, then:

```bash
npm --prefix apps/desktop run build
```

Expected: TypeScript build passes.

## Task 5: Add Supervisor Wrapper Around Existing SubAgent

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Create: `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session.rs`

- [ ] **Step 1: Write supervisor tests**

In `supervisor.rs`, add unit tests for result summarization and failure recording without requiring a real model call:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::bus::AgentA2ABus;

    #[test]
    fn delegate_result_for_model_extracts_json_result() {
        let raw = serde_json::json!({
            "result": "Found compact trigger in auto_compact.rs",
            "steps": []
        })
        .to_string();

        assert_eq!(
            delegate_result_for_model(&raw),
            "Found compact trigger in auto_compact.rs"
        );
    }

    #[test]
    fn join_error_records_failed_task() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            crate::agent::a2a::types::AgentRole::Researcher,
            crate::agent::a2a::types::AgentExecutionMode::ReadOnly,
            "Read files",
            "Read files",
            10,
        );

        record_child_failure(&mut bus, &task_id, "join_error", "subagent panicked", 20);

        let projection = bus.projection();
        assert_eq!(projection.failed_count, 1);
        assert_eq!(
            projection.tasks[0].failure_message.as_deref(),
            Some("subagent panicked")
        );
    }
}
```

- [ ] **Step 2: Implement child wrapper**

Create `child.rs`:

```rust
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::adapters::base::AiAdapter;
use crate::agent::event_sink::EventEmitter;
use crate::harness::Harness;

pub(crate) struct ChildAgentRuntime;

impl ChildAgentRuntime {
    pub(crate) async fn run_read_only(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_with_emitter(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }
}
```

- [ ] **Step 3: Implement supervisor helpers**

Create `supervisor.rs`:

```rust
use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::a2a::types::{AgentExecutionMode, AgentRole, AgentTaskId};

pub(crate) fn delegate_result_for_model(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("result")
                .and_then(|result| result.as_str())
                .map(|result| result.to_string())
        })
        .unwrap_or_else(|| raw.to_string())
}

pub(crate) fn assign_delegate_task(
    bus: &mut AgentA2ABus,
    title: &str,
    prompt: &str,
    timestamp_ms: u64,
) -> AgentTaskId {
    bus.assign_task(
        AgentRole::Researcher,
        AgentExecutionMode::ReadOnly,
        title,
        prompt,
        timestamp_ms,
    )
}

pub(crate) fn record_child_failure(
    bus: &mut AgentA2ABus,
    task_id: &AgentTaskId,
    kind: &str,
    message: &str,
    timestamp_ms: u64,
) {
    bus.fail_task(task_id, kind, message, true, timestamp_ms);
}
```

Keep this small in Phase 1. A later task can turn it into a struct with injected dependencies.

- [ ] **Step 4: Route `delegate_task` through A2A bus**

In `session.rs`, replace the local `SubAgent::run_with_emitter` setup inside `execute_tools` with:

```rust
let a2a_task_id = {
    let mut bus = lock_unpoisoned(&self.a2a_bus);
    crate::agent::a2a::supervisor::assign_delegate_task(
        &mut bus,
        "Delegated research task",
        &task,
        started_at_ms,
    )
};
self.emit_a2a_projection(emitter);
```

Before spawning:

```rust
{
    let mut bus = lock_unpoisoned(&self.a2a_bus);
    bus.start_task(&a2a_task_id, now_ms());
}
self.emit_a2a_projection(emitter);
```

Inside successful join:

```rust
{
    let mut bus = lock_unpoisoned(&self.a2a_bus);
    bus.complete_task(&a2a_task_id, &api_text, now_ms());
}
self.emit_a2a_projection(emitter);
```

Inside join error:

```rust
{
    let mut bus = lock_unpoisoned(&self.a2a_bus);
    crate::agent::a2a::supervisor::record_child_failure(
        &mut bus,
        &a2a_task_id,
        "join_error",
        &message,
        now_ms(),
    );
}
self.emit_a2a_projection(emitter);
```

Add helper method:

```rust
fn emit_a2a_projection(&self, emitter: &dyn crate::agent::event_sink::EventEmitter) {
    let state = lock_unpoisoned(&self.a2a_bus).projection();
    emitter.emit(crate::protocol::events::StreamEvent::AgentA2AUpdated {
        session_id: self.id.clone(),
        state,
    });
}
```

- [ ] **Step 5: Run targeted tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml delegate --lib
```

Expected: pass.

## Task 6: Add Compact A2A Timeline UI

**Files:**
- Create: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: the session/composer/message area where `agentTurnBySession` is consumed
- Modify: `apps/desktop/src/styles/process.css`

- [ ] **Step 1: Create component**

Create `AgentA2ATimeline.tsx`:

```tsx
import { CheckCircle2, CircleDashed, XCircle, PauseCircle } from "lucide-react";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "@/lib/protocol";

function iconFor(status: string) {
  if (status === "completed") return CheckCircle2;
  if (status === "failed") return XCircle;
  if (status === "interrupted") return PauseCircle;
  return CircleDashed;
}

function TaskRow({ task }: { task: AgentA2ATaskProjection }) {
  const Icon = iconFor(task.status);
  return (
    <div className="forge-a2a-task-row" data-status={task.status}>
      <Icon className="size-3" />
      <span className="forge-a2a-task-title">{task.title}</span>
      <span className="forge-a2a-task-role">{task.role}</span>
      {task.latest_message && (
        <span className="forge-a2a-task-message">{task.latest_message}</span>
      )}
      {task.failure_message && (
        <span className="forge-a2a-task-failure">{task.failure_message}</span>
      )}
    </div>
  );
}

export function AgentA2ATimeline({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  return (
    <div className="forge-a2a-timeline" data-testid="agent-a2a-timeline">
      <div className="forge-a2a-summary">
        <span>子任务</span>
        <span>{state.running_count} 运行中</span>
        <span>{state.completed_count} 完成</span>
        {state.failed_count > 0 && <span>{state.failed_count} 失败</span>}
      </div>
      <div className="forge-a2a-task-list">
        {state.tasks.map((task) => (
          <TaskRow key={task.task_id} task={task} />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add minimal CSS using existing tokens**

Add CSS to `apps/desktop/src/styles/process.css` near the existing `.forge-sub-agent-*` styles:

```css
.forge-a2a-timeline {
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px 10px;
  background: var(--card);
}

.forge-a2a-summary,
.forge-a2a-task-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  font-size: 12px;
}

.forge-a2a-task-title,
.forge-a2a-task-message,
.forge-a2a-task-failure {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

- [ ] **Step 3: Wire component into session view**

Where session-level state is available:

```tsx
const agentA2A = useStore((s) => s.agentA2ABySession.get(sessionId) ?? null);

<AgentA2ATimeline state={agentA2A} />
```

Place it near current agent turn/tool status, not as a new dominant dashboard.

- [ ] **Step 4: Add component test**

Add a narrow render test that asserts:

- no DOM when state is null
- running task title renders
- failed message renders

Run:

```bash
node --test apps/desktop/src/components/messages/AgentA2ATimeline.test.mjs
```

Expected: pass.

## Task 7: GoalLedger Association

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/session.rs`
- Modify: `apps/desktop/src-tauri/src/agent/goal_state.rs` only if helper methods are needed
- Add tests: `apps/desktop/src-tauri/src/agent/session_tests.rs`

- [ ] **Step 1: Add failing test**

In `session_tests.rs`, add a test that:

- creates a session
- sets an active GoalLedger with one pending task
- simulates a delegate assignment through the new helper
- asserts the matching GoalTask becomes in progress
- completes child task
- asserts GoalTask becomes completed

Use pure helper methods where possible instead of invoking real model calls.

- [ ] **Step 2: Add minimal association helper**

Avoid expanding GoalLedger schema in Phase 1. Add a small session helper:

```rust
fn mark_active_goal_task_for_a2a(&self, status: GoalTaskStatus) {
    let Some(task_id) = lock_unpoisoned(&self.goal_ledger)
        .as_ref()
        .and_then(|ledger| {
            ledger
                .active_goal()
                .and_then(|goal| goal.tasks.iter().find(|task| task.status == GoalTaskStatus::Pending))
                .map(|task| task.id.clone())
        })
    else {
        return;
    };

    if let Some(ledger) = lock_unpoisoned(&self.goal_ledger).as_mut() {
        ledger.update_task_status(&task_id, status, now_ms());
    }
}
```

Call this when child task starts and completes. If more exact mapping is needed, add it in Phase 2.

- [ ] **Step 3: Run tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml goal_ledger --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml a2a --lib
```

Expected: pass.

## Task 8: Full Verification And Obsidian Sync

**Files:**
- Update Obsidian if implementation changes direction:
  - `/Users/cabbos/cabbosAI/code-cli/Forge/02 System/Forge Agent Loop 当前状态.md`
  - `/Users/cabbos/cabbosAI/code-cli/Forge/02 System/Forge Internal A2A Runtime 设计.md`
  - `/Users/cabbos/cabbosAI/code-cli/Forge/00 LLM Wiki/Forge Current Handoff.md`

- [ ] **Step 1: Run backend and frontend verification**

Run:

```bash
npm run check:ci
npm --prefix apps/desktop run check:backend
npm --prefix apps/desktop run build
```

Expected: all pass.

- [ ] **Step 2: Run eval checks**

Run:

```bash
npm --prefix apps/desktop run eval:forge:test
npm run eval:report:latest -- --failures
npm run eval:forge:smoke:real -- --dry-run
```

Expected: pass.

- [ ] **Step 3: Run GitNexus detect changes before commit**

Run:

```text
gitnexus detect_changes scope=all repo=forge
```

Expected: affected symbols match A2A/session/protocol/frontend store changes. If risk is HIGH or CRITICAL, report the reason and the verification coverage.

- [ ] **Step 4: Sync Obsidian**

Update Obsidian with:

```markdown
## 2026-06-11 A2A Phase 1 Update

- Branch / PR:
- Commits:
- What changed:
- Validation:
- Known gaps:
- Next:
```

This is mandatory for Forge work. See `/Users/cabbos/cabbosAI/code-cli/Forge/05 Maintenance/Development Completion Obsidian Sync.md`.

- [ ] **Step 5: Commit**

Stage only intended files:

```bash
git status --short
git add apps/desktop/src-tauri/src/agent/a2a \
  apps/desktop/src-tauri/src/agent/mod.rs \
  apps/desktop/src-tauri/src/agent/session.rs \
  apps/desktop/src-tauri/src/agent/snapshot.rs \
  apps/desktop/src-tauri/src/ipc/session_lifecycle.rs \
  apps/desktop/src-tauri/src/protocol/events.rs \
  apps/desktop/src/lib/protocol.ts \
  apps/desktop/src/store \
  apps/desktop/src/components/messages/AgentA2ATimeline.tsx
git commit -m "feat(agent): add internal a2a control plane"
```

If this plan itself is committed separately first:

```bash
git add apps/desktop/docs/superpowers/specs/2026-06-11-internal-a2a-runtime-design.md \
  apps/desktop/docs/superpowers/plans/2026-06-11-internal-a2a-runtime-phase-1.md
git commit -m "docs(agent): plan internal a2a runtime"
```

## Execution Notes

- Keep Phase 1 narrow. The win is durable lifecycle state, not more autonomy.
- Preserve `delegate_task` tool behavior so existing model prompts and evals keep working.
- If `AgentSession` becomes too noisy, extract helper methods instead of adding long inline blocks.
- Do not introduce worktree writes until `WorktreeLease` has a separate design and tests.
- Do not skip Obsidian sync at the end.
