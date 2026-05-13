# Forge Workflow Router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Forge's first Workflow Router MVP so each user request is classified into a visible, explainable work mode.

**Architecture:** Add a small Rust `workflow` module with serializable models and a deterministic classifier. Emit workflow state through the existing stream protocol, mirror it in the Zustand store, and render a compact task status in the top bar, input bar, right context panel, and command palette. The first version is advisory: it does not block the agent loop, but strict routes are clearly marked as requiring confirmation in the UI.

**Tech Stack:** Tauri 2, Rust, serde, tokio `RwLock`, React 18, TypeScript, Zustand, Tailwind CSS, Lucide React, Playwright

---

## File Structure

| Action | File | Purpose |
|---|---|---|
| Create | `src-tauri/src/workflow/model.rs` | Shared workflow route, phase, gate, action, and state structs |
| Create | `src-tauri/src/workflow/router.rs` | Deterministic first-version classifier and unit tests |
| Create | `src-tauri/src/workflow/mod.rs` | Public workflow exports |
| Modify | `src-tauri/src/lib.rs` | Register `workflow` module and workflow IPC commands |
| Modify | `src-tauri/src/state.rs` | Store latest workflow state by session |
| Modify | `src-tauri/src/protocol/events.rs` | Add `workflow_updated` stream event |
| Modify | `src-tauri/src/ipc/handlers.rs` | Classify each `send_input` request and emit workflow state |
| Create | `src-tauri/src/ipc/workflow_handlers.rs` | IPC commands for reading state and manual route overrides |
| Modify | `src-tauri/src/ipc/mod.rs` | Export workflow handlers |
| Modify | `src/lib/protocol.ts` | Mirror workflow types and stream event |
| Modify | `src/lib/tauri.ts` | Add workflow IPC wrappers |
| Modify | `src/store/index.ts` | Persist and update workflow state per session |
| Create | `src/components/workflow/WorkflowStatusPill.tsx` | Compact top-bar workflow status |
| Create | `src/components/workflow/CurrentTaskCard.tsx` | Right-panel current task module with developer details |
| Modify | `src/components/layout/AppShell.tsx` | Render workflow status beside project/session metadata |
| Modify | `src/components/layout/HubPanel.tsx` | Render current task above Living Wiki |
| Modify | `src/components/session/InputBar.tsx` | Show soft/strict route notices without blocking input |
| Modify | `src/components/CommandPalette.tsx` | Add expert workflow override commands |
| Modify | `e2e/mock-ipc.ts` | Mock workflow IPC commands |
| Modify | `e2e/frontend.spec.ts` | Cover workflow status, context panel, and command override behavior |

---

### Task 1: Add Workflow Models And Classifier

**Files:**
- Create: `src-tauri/src/workflow/model.rs`
- Create: `src-tauri/src/workflow/router.rs`
- Create: `src-tauri/src/workflow/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the module shell**

Add this line near the other backend modules in `src-tauri/src/lib.rs`:

```rust
mod workflow;
```

Create `src-tauri/src/workflow/mod.rs`:

```rust
pub mod model;
pub mod router;

pub use model::{
    WorkflowGate, WorkflowOverrideAction, WorkflowPhase, WorkflowRoute, WorkflowState,
};
pub use router::{classify_workflow, workflow_state_from_override};
```

- [ ] **Step 2: Add workflow model types**

Create `src-tauri/src/workflow/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRoute {
    Direct,
    Light,
    Workflow,
    StrictWorkflow,
    Recovery,
    Verification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    Idle,
    Classifying,
    Clarifying,
    Designing,
    Spec,
    Planning,
    Executing,
    Debugging,
    Verifying,
    Done,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGate {
    None,
    Soft,
    ApprovalRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowOverrideAction {
    Direct,
    PlanFirst,
    Debug,
    Verify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowState {
    pub session_id: String,
    pub route: WorkflowRoute,
    pub phase: WorkflowPhase,
    pub beginner_label: String,
    pub developer_label: String,
    pub matched_signals: Vec<String>,
    pub reason: String,
    pub gate: WorkflowGate,
    pub override_actions: Vec<WorkflowOverrideAction>,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub checkpoint_id: Option<String>,
    pub updated_at: u64,
}
```

- [ ] **Step 3: Write classifier tests first**

Create `src-tauri/src/workflow/router.rs` with tests before implementation:

```rust
use super::model::{
    WorkflowGate, WorkflowOverrideAction, WorkflowPhase, WorkflowRoute, WorkflowState,
};

pub fn classify_workflow(session_id: &str, text: &str, updated_at: u64) -> WorkflowState {
    route_state(
        session_id,
        WorkflowRoute::Direct,
        vec!["classifier placeholder".to_string()],
        updated_at,
    )
}

pub fn workflow_state_from_override(
    session_id: &str,
    action: WorkflowOverrideAction,
    updated_at: u64,
) -> WorkflowState {
    let route = match action {
        WorkflowOverrideAction::Direct => WorkflowRoute::Direct,
        WorkflowOverrideAction::PlanFirst => WorkflowRoute::Workflow,
        WorkflowOverrideAction::Debug => WorkflowRoute::Recovery,
        WorkflowOverrideAction::Verify => WorkflowRoute::Verification,
    };
    let mut state = route_state(session_id, route, vec!["manual override".to_string()], updated_at);
    state.reason = "用户手动切换了当前工作方式。".to_string();
    state
}

fn route_state(
    session_id: &str,
    route: WorkflowRoute,
    matched_signals: Vec<String>,
    updated_at: u64,
) -> WorkflowState {
    WorkflowState {
        session_id: session_id.to_string(),
        route,
        phase: WorkflowPhase::Idle,
        beginner_label: "直接回答".to_string(),
        developer_label: "direct".to_string(),
        matched_signals,
        reason: "这是一个回答型请求，不需要改动项目。".to_string(),
        gate: WorkflowGate::None,
        override_actions: vec![WorkflowOverrideAction::PlanFirst, WorkflowOverrideAction::Debug],
        spec_path: None,
        plan_path: None,
        checkpoint_id: None,
        updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(text: &str) -> WorkflowState {
        classify_workflow("session-1", text, 42)
    }

    #[test]
    fn classifies_answer_only_request_as_direct() {
        let state = route("不要修改文件，不要执行命令。只回答：Workflow Router 是什么？");
        assert_eq!(state.route, WorkflowRoute::Direct);
        assert_eq!(state.gate, WorkflowGate::None);
        assert!(state.matched_signals.iter().any(|s| s.contains("不要修改文件")));
    }

    #[test]
    fn classifies_low_risk_copy_change_as_light() {
        let state = route("把右侧标题文案改成资料");
        assert_eq!(state.route, WorkflowRoute::Light);
        assert_eq!(state.phase, WorkflowPhase::Executing);
        assert_eq!(state.gate, WorkflowGate::None);
    }

    #[test]
    fn classifies_product_feature_as_workflow() {
        let state = route("我想做一个资料分析能力，支持 PDF 和 Word");
        assert_eq!(state.route, WorkflowRoute::Workflow);
        assert_eq!(state.phase, WorkflowPhase::Clarifying);
        assert_eq!(state.gate, WorkflowGate::Soft);
    }

    #[test]
    fn classifies_destructive_migration_as_strict_workflow() {
        let state = route("删除旧的 session 存储并迁移到新格式");
        assert_eq!(state.route, WorkflowRoute::StrictWorkflow);
        assert_eq!(state.phase, WorkflowPhase::Planning);
        assert_eq!(state.gate, WorkflowGate::ApprovalRequired);
    }

    #[test]
    fn classifies_failure_as_recovery() {
        let state = route("构建失败了，dev 状态下会挂掉");
        assert_eq!(state.route, WorkflowRoute::Recovery);
        assert_eq!(state.phase, WorkflowPhase::Debugging);
    }

    #[test]
    fn classifies_verification_request() {
        let state = route("帮我验收一下这个改动");
        assert_eq!(state.route, WorkflowRoute::Verification);
        assert_eq!(state.phase, WorkflowPhase::Verifying);
    }

    #[test]
    fn creates_manual_override_state() {
        let state = workflow_state_from_override("session-1", WorkflowOverrideAction::Debug, 99);
        assert_eq!(state.route, WorkflowRoute::Recovery);
        assert_eq!(state.updated_at, 99);
        assert!(state.reason.contains("手动切换"));
    }
}
```

- [ ] **Step 4: Run the failing classifier test**

Run:

```bash
cargo test workflow::router --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL. The tests for `light`, `workflow`, `strict_workflow`, `recovery`, and `verification` fail because `classify_workflow` still returns `direct`.

- [ ] **Step 5: Implement deterministic classification**

Replace the placeholder implementation in `src-tauri/src/workflow/router.rs` with:

```rust
use super::model::{
    WorkflowGate, WorkflowOverrideAction, WorkflowPhase, WorkflowRoute, WorkflowState,
};

pub fn classify_workflow(session_id: &str, text: &str, updated_at: u64) -> WorkflowState {
    let normalized = normalize(text);

    if let Some(signals) = collect_signals(&normalized, DIRECT_SIGNALS) {
        return route_state(session_id, WorkflowRoute::Direct, signals, updated_at);
    }
    if let Some(signals) = collect_signals(&normalized, RECOVERY_SIGNALS) {
        return route_state(session_id, WorkflowRoute::Recovery, signals, updated_at);
    }
    if let Some(signals) = collect_signals(&normalized, VERIFICATION_SIGNALS) {
        return route_state(session_id, WorkflowRoute::Verification, signals, updated_at);
    }
    if let Some(signals) = collect_signals(&normalized, STRICT_WORKFLOW_SIGNALS) {
        return route_state(session_id, WorkflowRoute::StrictWorkflow, signals, updated_at);
    }
    if let Some(signals) = collect_signals(&normalized, WORKFLOW_SIGNALS) {
        return route_state(session_id, WorkflowRoute::Workflow, signals, updated_at);
    }
    if let Some(signals) = collect_signals(&normalized, LIGHT_SIGNALS) {
        return route_state(session_id, WorkflowRoute::Light, signals, updated_at);
    }

    route_state(
        session_id,
        WorkflowRoute::Direct,
        vec!["fallback: no implementation signal".to_string()],
        updated_at,
    )
}

pub fn workflow_state_from_override(
    session_id: &str,
    action: WorkflowOverrideAction,
    updated_at: u64,
) -> WorkflowState {
    let route = match action {
        WorkflowOverrideAction::Direct => WorkflowRoute::Direct,
        WorkflowOverrideAction::PlanFirst => WorkflowRoute::Workflow,
        WorkflowOverrideAction::Debug => WorkflowRoute::Recovery,
        WorkflowOverrideAction::Verify => WorkflowRoute::Verification,
    };
    let mut state = route_state(session_id, route, vec!["manual override".to_string()], updated_at);
    state.reason = "用户手动切换了当前工作方式。".to_string();
    state
}

const DIRECT_SIGNALS: &[&str] = &[
    "不要修改文件",
    "不要执行命令",
    "只回答",
    "解释",
    "是什么",
    "现在做到哪",
    "status",
    "what is",
    "explain",
];

const LIGHT_SIGNALS: &[&str] = &[
    "改文案",
    "标题",
    "换成",
    "颜色",
    "按钮",
    "copy",
    "label",
    "color",
];

const WORKFLOW_SIGNALS: &[&str] = &[
    "我想做一个",
    "设计",
    "方案",
    "产品",
    "能力",
    "方向",
    "兼容",
    "资料系统",
    "feature",
    "build a",
    "create a",
];

const STRICT_WORKFLOW_SIGNALS: &[&str] = &[
    "重构整个",
    "删除旧的",
    "迁移到新格式",
    "权限系统",
    "sandbox",
    "permission",
    "migration",
    "delete old",
];

const RECOVERY_SIGNALS: &[&str] = &[
    "失败",
    "报错",
    "卡住",
    "挂掉",
    "打不开",
    "broken",
    "failed",
    "error",
    "stuck",
];

const VERIFICATION_SIGNALS: &[&str] = &[
    "验收",
    "检查结果",
    "跑一下检查",
    "确认这个改动",
    "verify",
    "validate",
    "test this",
];

fn normalize(text: &str) -> String {
    text.trim().to_lowercase()
}

fn collect_signals(text: &str, signals: &[&str]) -> Option<Vec<String>> {
    let matched = signals
        .iter()
        .filter(|signal| text.contains(**signal))
        .map(|signal| (*signal).to_string())
        .collect::<Vec<_>>();

    if matched.is_empty() {
        None
    } else {
        Some(matched)
    }
}

fn route_state(
    session_id: &str,
    route: WorkflowRoute,
    matched_signals: Vec<String>,
    updated_at: u64,
) -> WorkflowState {
    let profile = route_profile(&route);
    WorkflowState {
        session_id: session_id.to_string(),
        route,
        phase: profile.phase,
        beginner_label: profile.beginner_label.to_string(),
        developer_label: profile.developer_label.to_string(),
        matched_signals,
        reason: profile.reason.to_string(),
        gate: profile.gate,
        override_actions: profile.override_actions,
        spec_path: None,
        plan_path: None,
        checkpoint_id: None,
        updated_at,
    }
}

struct RouteProfile {
    phase: WorkflowPhase,
    beginner_label: &'static str,
    developer_label: &'static str,
    reason: &'static str,
    gate: WorkflowGate,
    override_actions: Vec<WorkflowOverrideAction>,
}

fn route_profile(route: &WorkflowRoute) -> RouteProfile {
    match route {
        WorkflowRoute::Direct => RouteProfile {
            phase: WorkflowPhase::Idle,
            beginner_label: "直接回答",
            developer_label: "direct",
            reason: "这是一个回答型请求，不需要改动项目。",
            gate: WorkflowGate::None,
            override_actions: vec![WorkflowOverrideAction::PlanFirst, WorkflowOverrideAction::Debug],
        },
        WorkflowRoute::Light => RouteProfile {
            phase: WorkflowPhase::Executing,
            beginner_label: "小改动，直接处理",
            developer_label: "light",
            reason: "这个请求范围较小，可以直接进入处理。",
            gate: WorkflowGate::None,
            override_actions: vec![WorkflowOverrideAction::PlanFirst, WorkflowOverrideAction::Verify],
        },
        WorkflowRoute::Workflow => RouteProfile {
            phase: WorkflowPhase::Clarifying,
            beginner_label: "先梳理想法",
            developer_label: "workflow",
            reason: "这个需求会影响多个部分，先拆清楚方案会更稳。",
            gate: WorkflowGate::Soft,
            override_actions: vec![WorkflowOverrideAction::Direct, WorkflowOverrideAction::Verify],
        },
        WorkflowRoute::StrictWorkflow => RouteProfile {
            phase: WorkflowPhase::Planning,
            beginner_label: "必须先确认方案",
            developer_label: "strict_workflow",
            reason: "这个请求涉及高风险改动，需要先确认方案和步骤。",
            gate: WorkflowGate::ApprovalRequired,
            override_actions: vec![WorkflowOverrideAction::Direct, WorkflowOverrideAction::Debug],
        },
        WorkflowRoute::Recovery => RouteProfile {
            phase: WorkflowPhase::Debugging,
            beginner_label: "遇到问题，正在排查",
            developer_label: "recovery",
            reason: "请求里出现了失败或异常信号，应该先定位问题。",
            gate: WorkflowGate::None,
            override_actions: vec![WorkflowOverrideAction::Direct, WorkflowOverrideAction::Verify],
        },
        WorkflowRoute::Verification => RouteProfile {
            phase: WorkflowPhase::Verifying,
            beginner_label: "正在检查结果",
            developer_label: "verification",
            reason: "用户正在要求检查或验收已有结果。",
            gate: WorkflowGate::None,
            override_actions: vec![WorkflowOverrideAction::Direct, WorkflowOverrideAction::Debug],
        },
    }
}
```

Keep the tests from Step 3 at the bottom of the file.

- [ ] **Step 6: Run classifier tests**

Run:

```bash
cargo test workflow::router --manifest-path src-tauri/Cargo.toml
```

Expected: PASS. All classifier tests pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add src-tauri/src/lib.rs src-tauri/src/workflow
git commit -m "feat: add workflow router classifier"
```

---

### Task 2: Add Workflow Stream Event And Backend State

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/protocol/events.rs`
- Modify: `src-tauri/src/ipc/handlers.rs`
- Create: `src-tauri/src/ipc/workflow_handlers.rs`
- Modify: `src-tauri/src/ipc/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add workflow state storage**

Modify `src-tauri/src/state.rs`:

```rust
use crate::workflow::WorkflowState;
```

Add this field to `AppState`:

```rust
pub workflow_states: Arc<RwLock<HashMap<String, WorkflowState>>>,
```

Initialize it inside `AppState::new`:

```rust
workflow_states: Arc::new(RwLock::new(HashMap::new())),
```

- [ ] **Step 2: Add the stream event**

Modify `src-tauri/src/protocol/events.rs`.

Add this import:

```rust
use crate::workflow::WorkflowState;
```

Add this enum variant after the memory events:

```rust
// ── Workflow Routing ──
#[serde(rename = "workflow_updated")]
WorkflowUpdated {
    session_id: String,
    state: WorkflowState,
},
```

Add `WorkflowUpdated { session_id, .. }` to the `session_id()` match arm.

- [ ] **Step 3: Add workflow IPC handlers**

Create `src-tauri/src/ipc/workflow_handlers.rs`:

```rust
use std::sync::Arc;
use tauri::Emitter;

use crate::protocol::events::StreamEvent;
use crate::state::AppState;
use crate::workflow::{workflow_state_from_override, WorkflowOverrideAction, WorkflowState};

#[tauri::command]
pub async fn get_workflow_state(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Option<WorkflowState>, String> {
    Ok(state.workflow_states.read().await.get(&session_id).cloned())
}

#[tauri::command]
pub async fn override_workflow_route(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    action: WorkflowOverrideAction,
) -> Result<WorkflowState, String> {
    let workflow = workflow_state_from_override(&session_id, action, now_ms());
    state
        .workflow_states
        .write()
        .await
        .insert(session_id.clone(), workflow.clone());
    let _ = app_handle.emit(
        "session-output",
        StreamEvent::WorkflowUpdated {
            session_id,
            state: workflow.clone(),
        },
    );
    Ok(workflow)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
```

- [ ] **Step 4: Export and register workflow commands**

Modify `src-tauri/src/ipc/mod.rs`:

```rust
pub mod workflow_handlers;
```

Add these commands to the `tauri::generate_handler![]` list in `src-tauri/src/lib.rs`:

```rust
ipc::workflow_handlers::get_workflow_state,
ipc::workflow_handlers::override_workflow_route,
```

- [ ] **Step 5: Classify each user request before the agent call**

Modify `send_input` in `src-tauri/src/ipc/handlers.rs`.

Add this import:

```rust
use crate::workflow::classify_workflow;
```

Add this helper near `resolve_working_dir`:

```rust
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
```

Inside `Some(s) => { ... }`, after `project_path` is calculated and before memory selection, add:

```rust
let workflow = classify_workflow(&session_id, &text, now_ms());
state
    .workflow_states
    .write()
    .await
    .insert(session_id.clone(), workflow.clone());
let _ = app_handle.emit(
    "session-output",
    StreamEvent::WorkflowUpdated {
        session_id: session_id.clone(),
        state: workflow,
    },
);
```

The request still continues to memory selection and `send_message_with_context`. This keeps the first version advisory and avoids adding a new blocking path to the agent loop.

- [ ] **Step 6: Run backend checks**

Run:

```bash
cargo test workflow --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS. The workflow module compiles and the classifier tests remain green.

- [ ] **Step 7: Commit**

Run:

```bash
git add src-tauri/src/state.rs src-tauri/src/protocol/events.rs src-tauri/src/ipc/handlers.rs src-tauri/src/ipc/workflow_handlers.rs src-tauri/src/ipc/mod.rs src-tauri/src/lib.rs
git commit -m "feat: emit workflow routing state"
```

---

### Task 3: Mirror Workflow State In TypeScript Store

**Files:**
- Modify: `src/lib/protocol.ts`
- Modify: `src/lib/tauri.ts`
- Modify: `src/store/index.ts`
- Modify: `e2e/mock-ipc.ts`

- [ ] **Step 1: Add TypeScript workflow protocol types**

Add these definitions to `src/lib/protocol.ts` above `StreamEvent`:

```ts
export type WorkflowRoute =
  | "direct"
  | "light"
  | "workflow"
  | "strict_workflow"
  | "recovery"
  | "verification";

export type WorkflowPhase =
  | "idle"
  | "classifying"
  | "clarifying"
  | "designing"
  | "spec"
  | "planning"
  | "executing"
  | "debugging"
  | "verifying"
  | "done"
  | "blocked";

export type WorkflowGate = "none" | "soft" | "approval_required";

export type WorkflowOverrideAction = "direct" | "plan_first" | "debug" | "verify";

export interface WorkflowState {
  session_id: string;
  route: WorkflowRoute;
  phase: WorkflowPhase;
  beginner_label: string;
  developer_label: string;
  matched_signals: string[];
  reason: string;
  gate: WorkflowGate;
  override_actions: WorkflowOverrideAction[];
  spec_path: string | null;
  plan_path: string | null;
  checkpoint_id: string | null;
  updated_at: number;
}
```

Add this union item after the memory stream events:

```ts
| { event_type: "workflow_updated"; session_id: string; state: WorkflowState }
```

- [ ] **Step 2: Add Tauri workflow wrappers**

Modify the protocol import in `src/lib/tauri.ts`:

```ts
import type { MemoryPatch, MemoryScope, SelectedContextMemory, WikiMemory, WorkflowOverrideAction, WorkflowState } from "./protocol";
```

Add wrappers near the memory IPC functions:

```ts
export async function getWorkflowState(sessionId: string): Promise<WorkflowState | null> {
  if (!hasTauriRuntime()) return null;
  return invoke("get_workflow_state", { sessionId });
}

export async function overrideWorkflowRoute(
  sessionId: string,
  action: WorkflowOverrideAction,
): Promise<WorkflowState> {
  return invoke("override_workflow_route", { sessionId, action });
}
```

- [ ] **Step 3: Store workflow state by session**

Modify the type import in `src/store/index.ts`:

```ts
import type { BlockState, SelectedContextMemory, StreamEvent, SessionState, WikiMemory, WorkflowState } from "../lib/protocol";
```

Add fields/actions to `AppStore`:

```ts
workflowBySession: Map<string, WorkflowState>;
setWorkflowState: (sessionId: string, workflow: WorkflowState) => void;
```

Add `workflowState` to `PersistedSession`:

```ts
workflowState?: WorkflowState | null;
```

Inside `persistSessions`, include:

```ts
workflowState: getWorkflowForPersist(s.id),
```

Add this helper near the persistence helpers:

```ts
function getWorkflowForPersist(sessionId: string): WorkflowState | null {
  return useStore.getState().workflowBySession.get(sessionId) ?? null;
}
```

Initialize store state:

```ts
workflowBySession: new Map(),
```

During hydrate, before constructing each session, create a workflow map:

```ts
const workflowBySession = new Map<string, WorkflowState>();
```

Inside the session loop:

```ts
if (s.workflowState) {
  workflowBySession.set(s.id, s.workflowState);
}
```

Include `workflowBySession` in the `set({ ... })` call that hydrates sessions.

Add the action:

```ts
setWorkflowState: (sessionId, workflow) => {
  const workflowBySession = new Map(get().workflowBySession);
  workflowBySession.set(sessionId, workflow);
  set({ workflowBySession });
  persistSessions(get().sessions);
},
```

In `removeSession`, delete workflow state:

```ts
const workflowBySession = new Map(get().workflowBySession);
workflowBySession.delete(id);
set({ sessions, activeSessionId, selectedContextBySession, workflowBySession });
```

In `dispatchOutputEvent`, add before memory handling:

```ts
if (event_type === "workflow_updated") {
  get().setWorkflowState(session_id, event.state);
  return;
}
```

- [ ] **Step 4: Mock workflow IPC for E2E**

Modify `e2e/mock-ipc.ts`.

Add handlers:

```ts
get_workflow_state?: (args: Record<string, unknown>) => unknown;
override_workflow_route?: (args: Record<string, unknown>) => unknown;
```

Add cases to `createMockIPC`:

```ts
case "get_workflow_state":
  return handlers.get_workflow_state?.(args) ?? null;
case "override_workflow_route":
  return handlers.override_workflow_route?.(args) ?? {
    session_id: String(args.sessionId ?? "session"),
    route: args.action === "debug" ? "recovery" : args.action === "verify" ? "verification" : args.action === "plan_first" ? "workflow" : "direct",
    phase: args.action === "debug" ? "debugging" : args.action === "verify" ? "verifying" : args.action === "plan_first" ? "clarifying" : "idle",
    beginner_label: args.action === "debug" ? "遇到问题，正在排查" : args.action === "verify" ? "正在检查结果" : args.action === "plan_first" ? "先梳理想法" : "直接回答",
    developer_label: String(args.action ?? "direct"),
    matched_signals: ["manual override"],
    reason: "用户手动切换了当前工作方式。",
    gate: "none",
    override_actions: ["direct", "plan_first", "debug", "verify"],
    spec_path: null,
    plan_path: null,
    checkpoint_id: null,
    updated_at: Date.now(),
  };
```

- [ ] **Step 5: Run TypeScript check**

Run:

```bash
npm run build
```

Expected: PASS. Vite may still print the existing chunk-size warning.

- [ ] **Step 6: Commit**

Run:

```bash
git add src/lib/protocol.ts src/lib/tauri.ts src/store/index.ts e2e/mock-ipc.ts
git commit -m "feat: mirror workflow state in frontend"
```

---

### Task 4: Render Workflow Status In Top Bar, Context Panel, And Input Bar

**Files:**
- Create: `src/components/workflow/WorkflowStatusPill.tsx`
- Create: `src/components/workflow/CurrentTaskCard.tsx`
- Modify: `src/components/layout/AppShell.tsx`
- Modify: `src/components/layout/HubPanel.tsx`
- Modify: `src/components/session/InputBar.tsx`

- [ ] **Step 1: Add compact top-bar status component**

Create `src/components/workflow/WorkflowStatusPill.tsx`:

```tsx
import { Compass, ShieldAlert } from "lucide-react";
import type { WorkflowState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function WorkflowStatusPill({ workflow }: { workflow: WorkflowState | null }) {
  if (!workflow) return null;

  const strict = workflow.gate === "approval_required";

  return (
    <span
      className={cn(
        "inline-flex max-w-[220px] shrink-0 items-center gap-1 rounded-md border px-2 py-0.5 text-[10px]",
        strict ? "border-amber-500/30 text-amber-300" : "border-border text-muted-foreground",
      )}
      title={`${workflow.developer_label}: ${workflow.reason}`}
    >
      {strict ? <ShieldAlert className="size-3" /> : <Compass className="size-3" />}
      <span className="truncate">{workflow.beginner_label}</span>
    </span>
  );
}
```

- [ ] **Step 2: Add right-panel current task card**

Create `src/components/workflow/CurrentTaskCard.tsx`:

```tsx
import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { WorkflowState } from "@/lib/protocol";

export function CurrentTaskCard({ workflow }: { workflow: WorkflowState | null }) {
  const [expanded, setExpanded] = useState(false);

  if (!workflow) {
    return (
      <section>
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-[11px] font-medium text-muted-foreground">当前任务</h3>
          <span className="text-[10px] text-muted-foreground/70">自动判断</span>
        </div>
        <div className="rounded-md border border-border bg-card px-3 py-3 text-xs text-muted-foreground">
          发送消息后会显示当前工作方式。
        </div>
      </section>
    );
  }

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">当前任务</h3>
        <span className="text-[10px] text-muted-foreground/70">自动判断</span>
      </div>
      <div className="rounded-md border border-border bg-card px-3 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{workflow.beginner_label}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{workflow.reason}</div>
          </div>
          <span className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {gateLabel(workflow.gate)}
          </span>
        </div>

        {workflow.gate !== "none" && (
          <div className="mt-2 rounded border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-200/90">
            {workflow.gate === "soft" ? "建议先梳理方案，也可以直接处理。" : "建议先确认方案，再进入实现。"}
          </div>
        )}

        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="mt-2 inline-flex items-center gap-1 text-[10px] text-muted-foreground transition-colors hover:text-foreground"
        >
          {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
          开发者详情
        </button>

        {expanded && (
          <div className="mt-2 space-y-1 rounded border border-border bg-background/40 p-2 font-mono text-[10px] text-muted-foreground">
            <Row label="route" value={workflow.developer_label} />
            <Row label="phase" value={workflow.phase} />
            <Row label="gate" value={workflow.gate} />
            <Row label="signals" value={workflow.matched_signals.join(", ") || "none"} />
            {workflow.spec_path && <Row label="spec" value={workflow.spec_path} />}
            {workflow.plan_path && <Row label="plan" value={workflow.plan_path} />}
            {workflow.checkpoint_id && <Row label="checkpoint" value={workflow.checkpoint_id} />}
          </div>
        )}
      </div>
    </section>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[72px_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground/60">{label}</span>
      <span className="truncate text-muted-foreground">{value}</span>
    </div>
  );
}

function gateLabel(gate: WorkflowState["gate"]) {
  switch (gate) {
    case "none":
      return "直接";
    case "soft":
      return "建议";
    case "approval_required":
      return "需确认";
  }
}
```

- [ ] **Step 3: Render workflow status in `AppShell`**

Modify `src/components/layout/AppShell.tsx`.

Add the import:

```tsx
import { WorkflowStatusPill } from "@/components/workflow/WorkflowStatusPill";
```

Read active workflow:

```tsx
const workflow = useStore((s) => activeSessionId ? s.workflowBySession.get(activeSessionId) ?? null : null);
```

Render it after the existing session status pill:

```tsx
<WorkflowStatusPill workflow={workflow} />
```

- [ ] **Step 4: Render current task in `HubPanel`**

Modify `src/components/layout/HubPanel.tsx`.

Add import:

```tsx
import { CurrentTaskCard } from "@/components/workflow/CurrentTaskCard";
```

Read workflow:

```tsx
const workflow = useStore((s) => activeId ? s.workflowBySession.get(activeId) ?? null : null);
```

Render it above `WikiSections`:

```tsx
<CurrentTaskCard workflow={workflow} />
```

- [ ] **Step 5: Add input notices**

Modify `src/components/session/InputBar.tsx`.

Read workflow:

```tsx
const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
```

Below the selected context hint, add:

```tsx
{workflow?.gate === "soft" && (
  <div className="mb-2 rounded-md border border-border bg-card px-3 py-2 text-[11px] text-muted-foreground">
    这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。
  </div>
)}
{workflow?.gate === "approval_required" && (
  <div className="mb-2 rounded-md border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-200">
    这个请求风险较高，建议先确认方案和步骤。
  </div>
)}
```

- [ ] **Step 6: Run frontend build**

Run:

```bash
npm run build
```

Expected: PASS. No TypeScript errors.

- [ ] **Step 7: Commit**

Run:

```bash
git add src/components/workflow src/components/layout/AppShell.tsx src/components/layout/HubPanel.tsx src/components/session/InputBar.tsx
git commit -m "feat: show workflow routing status"
```

---

### Task 5: Add Command Palette Workflow Overrides

**Files:**
- Modify: `src/components/CommandPalette.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/store/index.ts`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add override handler in command palette**

Modify `src/components/CommandPalette.tsx`.

Add imports:

```tsx
import { Compass, Bug, CheckCircle2, Zap } from "lucide-react";
import type { WorkflowOverrideAction } from "@/lib/protocol";
import { overrideWorkflowRoute } from "@/lib/tauri";
```

Read active session:

```tsx
const activeSessionId = useStore((s) => s.activeSessionId);
const setWorkflowState = useStore((s) => s.setWorkflowState);
```

Add handler:

```tsx
const handleWorkflowOverride = async (action: WorkflowOverrideAction) => {
  if (!activeSessionId) return;
  onOpenChange(false);
  try {
    const workflow = await overrideWorkflowRoute(activeSessionId, action);
    setWorkflowState(activeSessionId, workflow);
  } catch (error) {
    console.error("Failed to override workflow:", error);
  }
};
```

- [ ] **Step 2: Render workflow command group**

Add this group after the existing `操作` group:

```tsx
{activeSessionId && (
  <>
    <CommandSeparator />
    <CommandGroup heading="工作方式">
      <CommandItem onSelect={() => handleWorkflowOverride("plan_first")}>
        <Compass className="size-4" />
        先梳理方案
      </CommandItem>
      <CommandItem onSelect={() => handleWorkflowOverride("direct")}>
        <Zap className="size-4" />
        直接处理
      </CommandItem>
      <CommandItem onSelect={() => handleWorkflowOverride("debug")}>
        <Bug className="size-4" />
        排查问题
      </CommandItem>
      <CommandItem onSelect={() => handleWorkflowOverride("verify")}>
        <CheckCircle2 className="size-4" />
        检查结果
      </CommandItem>
    </CommandGroup>
  </>
)}
```

If the file already imports `Moon` and `Sun` from `lucide-react`, merge the new icons into the same import.

- [ ] **Step 3: Add E2E coverage for command override**

Append this test to `e2e/frontend.spec.ts`. Also update the import section with:

```ts
import type { WorkflowState } from "../src/lib/protocol";
```

Test code:

```ts
test.describe("Workflow Router", () => {
  test("shows workflow state and allows command palette override", async ({ page }) => {
    const sessionId = "workflow-router-session";
    const workflowState: WorkflowState = {
      session_id: sessionId,
      route: "workflow",
      phase: "clarifying",
      beginner_label: "先梳理想法",
      developer_label: "workflow",
      matched_signals: ["我想做一个"],
      reason: "这个需求会影响多个部分，先拆清楚方案会更稳。",
      gate: "soft",
      override_actions: ["direct", "verify"],
      spec_path: null,
      plan_path: null,
      checkpoint_id: null,
      updated_at: 1778620800000,
    };

    await setup(page);
    await page.addInitScript(({ sessionId }) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      const workingDir = "/Users/cabbos/project/crusted-spinning-lynx-agent";
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionId };
          case "get_default_working_dir":
            return workingDir;
          case "get_project_runtime_status":
            return {
              working_dir: workingDir,
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: "npm run dev",
              port: 1420,
              url: "http://localhost:1420",
              running: false,
              managed: false,
              pid: null,
              can_start: true,
              can_stop: false,
              can_open: true,
              message: "Preview not running",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: workingDir,
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "No checkpoint yet",
            };
          case "list_memories":
            return [];
          case "override_workflow_route":
            return {
              session_id: sessionId,
              route: "recovery",
              phase: "debugging",
              beginner_label: "遇到问题，正在排查",
              developer_label: "recovery",
              matched_signals: ["manual override"],
              reason: "用户手动切换了当前工作方式。",
              gate: "none",
              override_actions: ["direct", "verify"],
              spec_path: null,
              plan_path: null,
              checkpoint_id: null,
              updated_at: Date.now(),
            };
          default:
            return undefined;
        }
      };
    }, { sessionId });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "workflow_updated", session_id: sessionId, state: workflowState },
    ]);

    await expect(page.getByText("先梳理想法").first()).toBeVisible();
    await expect(page.getByText("这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。")).toBeVisible();

    await page.keyboard.press(process.platform === "darwin" ? "Meta+K" : "Control+K");
    await page.getByText("排查问题").click();
    await expect(page.getByText("遇到问题，正在排查").first()).toBeVisible();
  });
});
```

- [ ] **Step 4: Run E2E test and build**

Run:

```bash
npx playwright test e2e/frontend.spec.ts
npm run build
```

Expected: PASS. Existing Vite chunk-size warning may remain.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/components/CommandPalette.tsx e2e/frontend.spec.ts
git commit -m "feat: add workflow override commands"
```

---

### Task 6: Final Verification And Product Copy Pass

**Files:**
- Modify only files from earlier tasks if verification exposes issues.

- [ ] **Step 1: Re-read product copy**

Search for raw internal labels that should not be primary beginner-facing text:

```bash
rg -n "strict_workflow|workflow|verification|recovery|approval_required|debugging|classifying" src src-tauri e2e
```

Expected: Raw labels appear only in protocol/types/tests/developer-detail rows. User-facing top status and right-panel primary labels use Chinese product language such as `先梳理想法`, `排查问题`, and `检查结果`.

- [ ] **Step 2: Run backend verification**

Run:

```bash
cargo test workflow --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run frontend verification**

Run:

```bash
npm run build
npx playwright test e2e/frontend.spec.ts
git diff --check
```

Expected: PASS. Vite may print the existing chunk-size warning after `npm run build`; no TypeScript error should appear.

- [ ] **Step 4: Commit final copy/test fixes if needed**

If Step 1, Step 2, or Step 3 required edits, commit them:

```bash
git add src src-tauri e2e
git commit -m "fix: polish workflow router mvp"
```

If no edits were required, skip this commit.

- [ ] **Step 5: Report test prompts**

Use these prompts to manually exercise routing without intentionally breaking the dev server:

```text
不要修改文件，不要执行命令。只回答：Workflow Router 是什么？
```

```text
把右侧标题文案改成资料
```

```text
我想做一个能分析 PDF 和 Word 的资料系统
```

```text
删除旧的 session 存储并迁移到新格式
```

```text
不要修改文件，不要执行命令。只回答：如果构建失败了应该怎么排查？
```

```text
帮我验收一下刚才的改动
```

Expected UI behavior:

- The top bar shows a short Chinese task label.
- The right `上下文` panel shows `当前任务` with the reason.
- Developer details show route, phase, gate, and matched signals.
- The command palette can switch to `先梳理方案`, `直接处理`, `排查问题`, or `检查结果`.
