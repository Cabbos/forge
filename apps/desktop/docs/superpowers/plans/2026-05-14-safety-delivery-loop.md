# Safety Delivery Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Safety Delivery Loop so users can see what Forge will modify, whether preview/checkpoints are healthy, and what happened after meaningful work.

**Architecture:** Enrich the existing confirmation flow instead of creating a new permission system. Add a small Rust write-boundary model, carry it through `ConfirmAsk`, render it in the existing confirmation card, then improve the compact Delivery module and add a lightweight turn-closure card. Keep everything inside the existing product layers: 当前任务, 项目档案, 交付.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Zustand, Playwright, existing runtime/checkpoint IPC.

---

## Scope Check

This plan implements one coherent loop with three connected surfaces:

1. write-boundary confirmation
2. compact delivery confidence
3. turn closure after meaningful work

It does not implement a full diff viewer, git restore UI, document parsing, new memory system, or a separate delivery dashboard.

## File Structure

- Create: `src-tauri/src/harness/write_boundary.rs`
  - Owns the Rust model and derivation logic for workspace, operation, file impact, risk, recovery text, and Forge-source warning.

- Modify: `src-tauri/src/harness/mod.rs`
  - Registers `write_boundary` module.
  - Emits enriched `ConfirmAsk` events.

- Modify: `src-tauri/src/protocol/events.rs`
  - Adds `WriteBoundary` to `ConfirmAsk` as an optional field.

- Modify: `src/lib/protocol.ts`
  - Mirrors `WriteBoundary` and enriched `confirm_ask`.
  - Adds `delivery_summary` event if Task 5 is implemented through the stream event path.

- Modify: `src/store/index.ts`
  - Stores `boundary` metadata for confirmation blocks.
  - Stores delivery summary blocks if added.

- Create: `src/lib/write-boundary.ts`
  - Normalizes unknown metadata from persisted blocks into a typed frontend shape.

- Modify: `src/components/messages/ConfirmCard.tsx`
  - Renders `准备修改项目` card with workspace, operation, impact, risk, recovery, and actions.

- Create: `src/lib/delivery-confidence.ts`
  - Turns runtime/checkpoint status into compact labels, colors, and next actions.

- Modify: `src/components/layout/ProjectStatusCard.tsx`
  - Uses delivery confidence helper.
  - Adds `打开预览`, `启动预览`, and `创建检查点` actions where available.

- Create: `src/components/messages/DeliverySummaryCard.tsx`
  - Shows a compact closure after meaningful work.

- Modify: `src/components/chat/MessageList.tsx`
  - Renders delivery summary blocks.

- Modify: `src/hooks/useSession.ts`
  - After `sendInput` completes, refreshes delivery status and emits a local delivery summary block.

- Modify: `e2e/frontend.spec.ts`
  - Adds product-level acceptance coverage.

- Modify: `src-tauri/tests/harness_test.rs`
  - Adds Rust coverage for write-boundary derivation.

## Task 1: Backend Write Boundary Model

**Files:**

- Create: `src-tauri/src/harness/write_boundary.rs`
- Modify: `src-tauri/src/harness/mod.rs`
- Test: `src-tauri/tests/harness_test.rs`

- [ ] **Step 1: Write failing Rust tests for file, shell, and Forge-source boundaries**

Add imports near the top of `src-tauri/tests/harness_test.rs` inside `mod harness`:

```rust
use forge::harness::write_boundary::{build_write_boundary, WriteBoundaryRisk};
```

Add these tests before `test_summary`:

```rust
#[test]
fn test_write_boundary_for_file_write_shows_workspace_and_file() {
    let workspace = std::env::temp_dir().join("forge-boundary-project");
    let _ = std::fs::create_dir_all(&workspace);

    let boundary = build_write_boundary(
        "write_to_file",
        &serde_json::json!({"path":"src/app.tsx","content":"hello"}),
        &workspace,
        "file_write",
    );

    assert_eq!(boundary.title, "准备修改项目");
    assert_eq!(boundary.operation, "写入文件");
    assert_eq!(boundary.workspace_path, workspace.to_string_lossy());
    assert_eq!(boundary.affected_files, vec!["src/app.tsx".to_string()]);
    assert_eq!(boundary.impact, "将修改 1 个文件");
    assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn test_write_boundary_for_shell_command_uses_workspace_wide_caution() {
    let workspace = std::env::temp_dir().join("forge-boundary-shell");
    let _ = std::fs::create_dir_all(&workspace);

    let boundary = build_write_boundary(
        "run_shell",
        &serde_json::json!({"command":"npm install"}),
        &workspace,
        "shell_cmd",
    );

    assert_eq!(boundary.operation, "执行命令");
    assert_eq!(boundary.command.as_deref(), Some("npm install"));
    assert!(boundary.affected_files.is_empty());
    assert_eq!(boundary.impact, "这个命令可能影响当前工作空间");
    assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn test_write_boundary_warns_for_forge_source_workspace() {
    let workspace = std::env::temp_dir().join("forge-source-like");
    let _ = std::fs::create_dir_all(workspace.join("src-tauri"));
    let _ = std::fs::write(
        workspace.join("package.json"),
        r#"{"name":"forge","version":"0.1.0"}"#,
    );

    let boundary = build_write_boundary(
        "write_to_file",
        &serde_json::json!({"path":"src/main.tsx","content":"hello"}),
        &workspace,
        "file_write",
    );

    assert_eq!(boundary.risk, WriteBoundaryRisk::High);
    assert_eq!(
        boundary.warning.as_deref(),
        Some("这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。")
    );

    let _ = std::fs::remove_dir_all(&workspace);
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_write_boundary --test harness_test
```

Expected: compile failure because `forge::harness::write_boundary` does not exist.

- [ ] **Step 3: Add the write-boundary module**

Create `src-tauri/src/harness/write_boundary.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteBoundaryRisk {
    Normal,
    Caution,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriteBoundary {
    pub title: String,
    pub workspace_name: String,
    pub workspace_path: String,
    pub operation: String,
    pub affected_files: Vec<String>,
    pub command: Option<String>,
    pub impact: String,
    pub risk: WriteBoundaryRisk,
    pub recovery: String,
    pub warning: Option<String>,
}

pub fn build_write_boundary(
    tool_name: &str,
    input: &serde_json::Value,
    working_dir: &Path,
    kind: &str,
) -> WriteBoundary {
    let canonical = canonical_tool(tool_name);
    let workspace_path = working_dir.to_string_lossy().to_string();
    let workspace_name = working_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("当前项目")
        .to_string();

    let affected_files = affected_files_for(canonical, input);
    let command = if canonical == "run_shell" {
        input
            .get("command")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    } else {
        None
    };
    let operation = operation_label(canonical).to_string();
    let is_forge_source = is_forge_source_workspace(working_dir);
    let risk = if is_forge_source || kind == "dangerous_cmd" {
        WriteBoundaryRisk::High
    } else if kind == "file_write" || kind == "shell_cmd" {
        WriteBoundaryRisk::Caution
    } else {
        WriteBoundaryRisk::Normal
    };
    let warning = is_forge_source.then(|| {
        "这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。".to_string()
    });
    let impact = if affected_files.is_empty() {
        "这个命令可能影响当前工作空间".to_string()
    } else {
        format!("将修改 {} 个文件", affected_files.len())
    };

    WriteBoundary {
        title: "准备修改项目".to_string(),
        workspace_name,
        workspace_path,
        operation,
        affected_files,
        command,
        impact,
        risk,
        recovery: "交付区会显示预览和检查点状态。".to_string(),
        warning,
    }
}

fn canonical_tool(tool: &str) -> &str {
    match tool {
        "write" | "write_file" => "write_to_file",
        "edit" => "edit_file",
        "bash" | "execute_command" | "shell" => "run_shell",
        other => other,
    }
}

fn operation_label(tool: &str) -> &'static str {
    match tool {
        "edit_file" => "修改文件",
        "write_to_file" => "写入文件",
        "run_shell" => "执行命令",
        _ => "执行操作",
    }
}

fn affected_files_for(tool: &str, input: &serde_json::Value) -> Vec<String> {
    if matches!(tool, "write_to_file" | "edit_file") {
        input
            .get("path")
            .and_then(|value| value.as_str())
            .filter(|path| !path.trim().is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn is_forge_source_workspace(working_dir: &Path) -> bool {
    if !working_dir.join("src-tauri").is_dir() {
        return false;
    }

    std::fs::read_to_string(working_dir.join("package.json"))
        .map(|content| content.contains(r#""name":"forge""#) || content.contains(r#""name": "forge""#))
        .unwrap_or(false)
}
```

Modify `src-tauri/src/harness/mod.rs` module declarations:

```rust
pub mod write_boundary;
```

- [ ] **Step 4: Run tests and verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_write_boundary --test harness_test
```

Expected: 3 write-boundary tests pass.

- [ ] **Step 5: Commit Task 1**

```bash
git add src-tauri/src/harness/write_boundary.rs src-tauri/src/harness/mod.rs src-tauri/tests/harness_test.rs
git commit -m "feat: derive write boundary metadata"
```

## Task 2: Carry Write Boundary Through ConfirmAsk

**Files:**

- Modify: `src-tauri/src/protocol/events.rs`
- Modify: `src/lib/protocol.ts`
- Modify: `src-tauri/src/harness/mod.rs`
- Modify: `src/store/index.ts`
- Test: `src-tauri/tests/harness_test.rs`

- [ ] **Step 1: Add protocol fields in Rust**

Modify `src-tauri/src/protocol/events.rs` imports:

```rust
use crate::harness::write_boundary::WriteBoundary;
```

Modify `StreamEvent::ConfirmAsk`:

```rust
#[serde(rename = "confirm_ask")]
ConfirmAsk {
    session_id: String,
    block_id: String,
    question: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    boundary: Option<WriteBoundary>,
},
```

Update any existing `ConfirmAsk` constructors to include `boundary: None` temporarily. Use `rg "ConfirmAsk"` to find all constructors.

- [ ] **Step 2: Add protocol fields in TypeScript**

Modify `src/lib/protocol.ts` near the stream types:

```ts
export type WriteBoundaryRisk = "normal" | "caution" | "high";

export interface WriteBoundary {
  title: string;
  workspace_name: string;
  workspace_path: string;
  operation: string;
  affected_files: string[];
  command?: string | null;
  impact: string;
  risk: WriteBoundaryRisk;
  recovery: string;
  warning?: string | null;
}
```

Replace the `confirm_ask` event union member:

```ts
| {
    event_type: "confirm_ask";
    session_id: string;
    block_id: string;
    question: string;
    kind: string;
    boundary?: WriteBoundary | null;
  }
```

- [ ] **Step 3: Emit boundary metadata from the harness**

Modify imports in `src-tauri/src/harness/mod.rs`:

```rust
use crate::harness::write_boundary::build_write_boundary;
```

Inside `execute_tool_with_block_id`, where `StreamEvent::ConfirmAsk` is emitted, build and include the boundary:

```rust
let boundary = build_write_boundary(tool_name, &input, &self.working_dir, &kind);
let _ = app_handle.emit(
    "session-output",
    crate::protocol::events::StreamEvent::ConfirmAsk {
        session_id: session_id.to_string(),
        block_id: block_id.clone(),
        question,
        kind,
        boundary: Some(boundary),
    },
);
```

Keep other constructors as `boundary: None`.

- [ ] **Step 4: Store boundary metadata in frontend blocks**

Modify `eventToBlock` in `src/store/index.ts`:

```ts
case "confirm_ask":
  return {
    ...base,
    event_type: "confirm_ask",
    content: event.question,
    metadata: {
      kind: event.kind,
      boundary: event.boundary ?? null,
    },
  };
```

- [ ] **Step 5: Run compile checks**

Run:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: both commands exit 0.

- [ ] **Step 6: Commit Task 2**

```bash
git add src-tauri/src/protocol/events.rs src/lib/protocol.ts src-tauri/src/harness/mod.rs src/store/index.ts
git commit -m "feat: stream write boundary confirmations"
```

## Task 3: Product-Level Confirmation Card

**Files:**

- Create: `src/lib/write-boundary.ts`
- Modify: `src/components/messages/ConfirmCard.tsx`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add failing Playwright test for enriched confirmation copy**

In `e2e/frontend.spec.ts`, add this test inside `test.describe("Timeline Message Flow", ...)` after the existing internal-context test:

```ts
test("write confirmation shows project boundary before approving", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await page.addInitScript((sessionId) => {
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await expect(page.locator("textarea")).toBeVisible();
  await page.waitForFunction(() => {
    // @ts-expect-error listeners
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    {
      event_type: "confirm_ask",
      session_id: sessionId,
      block_id: "confirm-write-boundary",
      question: "AI 想要写入文件：src/App.tsx",
      kind: "file_write",
      boundary: {
        title: "准备修改项目",
        workspace_name: "forge",
        workspace_path: "/Users/cabbos/project/forge",
        operation: "写入文件",
        affected_files: ["src/App.tsx"],
        command: null,
        impact: "将修改 1 个文件",
        risk: "caution",
        recovery: "交付区会显示预览和检查点状态。",
        warning: null,
      },
    },
  ], 5);

  await expect(page.getByText("准备修改项目")).toBeVisible();
  await expect(page.getByText("工作空间")).toBeVisible();
  await expect(page.getByText("forge")).toBeVisible();
  await expect(page.getByText("/Users/cabbos/project/forge")).toBeVisible();
  await expect(page.getByText("写入文件")).toBeVisible();
  await expect(page.getByText("src/App.tsx")).toBeVisible();
  await expect(page.getByRole("button", { name: "继续" })).toBeVisible();
  await expect(page.getByRole("button", { name: "取消" })).toBeVisible();
});
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "write confirmation shows project boundary"
```

Expected: fail because `ConfirmCard` still uses old labels like `同意继续`.

- [ ] **Step 3: Add frontend boundary helper**

Create `src/lib/write-boundary.ts`:

```ts
import type { WriteBoundary, WriteBoundaryRisk } from "@/lib/protocol";

export function parseWriteBoundary(value: unknown): WriteBoundary | null {
  if (!value || typeof value !== "object") return null;
  const data = value as Partial<WriteBoundary>;
  if (
    typeof data.title !== "string" ||
    typeof data.workspace_name !== "string" ||
    typeof data.workspace_path !== "string" ||
    typeof data.operation !== "string" ||
    typeof data.impact !== "string" ||
    typeof data.recovery !== "string" ||
    !isWriteBoundaryRisk(data.risk)
  ) {
    return null;
  }

  return {
    title: data.title,
    workspace_name: data.workspace_name,
    workspace_path: data.workspace_path,
    operation: data.operation,
    affected_files: Array.isArray(data.affected_files) ? data.affected_files.map(String) : [],
    command: typeof data.command === "string" ? data.command : null,
    impact: data.impact,
    risk: data.risk,
    recovery: data.recovery,
    warning: typeof data.warning === "string" ? data.warning : null,
  };
}

export function riskLabel(risk: WriteBoundaryRisk) {
  if (risk === "high") return "高";
  if (risk === "caution") return "需要确认";
  return "普通";
}

function isWriteBoundaryRisk(value: unknown): value is WriteBoundaryRisk {
  return value === "normal" || value === "caution" || value === "high";
}
```

- [ ] **Step 4: Update `ConfirmCard` UI**

In `src/components/messages/ConfirmCard.tsx`, import the helper:

```ts
import { parseWriteBoundary, riskLabel } from "@/lib/write-boundary";
```

Inside the component, derive:

```ts
const boundary = parseWriteBoundary(block.metadata.boundary);
```

Replace the old card body with a branch:

```tsx
{boundary ? (
  <div className="px-4 py-3">
    {boundary.warning && (
      <div className="mb-3 rounded border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs leading-relaxed text-destructive">
        {boundary.warning}
      </div>
    )}
    <div className="space-y-2 text-xs">
      <BoundaryLine label="工作空间" value={`${boundary.workspace_name} · ${boundary.workspace_path}`} />
      <BoundaryLine label="操作" value={boundary.operation} />
      <BoundaryLine label="影响范围" value={boundary.impact} />
      <BoundaryLine label="风险" value={riskLabel(boundary.risk)} />
      <BoundaryLine label="恢复点" value={boundary.recovery} />
    </div>
    {boundary.affected_files.length > 0 && (
      <div className="mt-3 rounded bg-background/60 px-3 py-2">
        <div className="mb-1 text-[11px] text-muted-foreground">文件</div>
        <div className="space-y-1">
          {boundary.affected_files.map((file) => (
            <div key={file} className="truncate font-mono text-[11px] text-foreground/85">{file}</div>
          ))}
        </div>
      </div>
    )}
    {boundary.command && (
      <div className="mt-3 rounded bg-background/60 px-3 py-2 font-mono text-[11px] text-foreground/85">
        {boundary.command}
      </div>
    )}
  </div>
) : (
  <div className="px-4 py-3">
    <p className="whitespace-pre-wrap text-sm leading-relaxed" style={{ color: "#E4E7EC" }}>{question}</p>
    <p className="mt-2 text-xs leading-relaxed" style={{ color: "var(--muted-foreground)" }}>{helperText}</p>
  </div>
)}
```

Change the unresolved action labels to:

```tsx
继续
取消
```

Add this helper below the component:

```tsx
function BoundaryLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[64px_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate text-foreground/85">{value}</span>
    </div>
  );
}
```

- [ ] **Step 5: Run focused test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "write confirmation shows project boundary"
```

Expected: pass.

- [ ] **Step 6: Commit Task 3**

```bash
git add src/lib/write-boundary.ts src/components/messages/ConfirmCard.tsx e2e/frontend.spec.ts
git commit -m "feat: show project write boundary confirmations"
```

## Task 4: Compact Delivery Confidence

**Files:**

- Create: `src/lib/delivery-confidence.ts`
- Modify: `src/components/layout/ProjectStatusCard.tsx`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add failing e2e test for actionable delivery**

Add a test inside `test.describe("Project records context panel", ...)`:

```ts
test("delivery shows preview action and checkpoint next step", async ({ page }) => {
  const sessionId = "delivery-confidence-session";
  await setup(page);
  await page.addInitScript((sessionId) => {
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await expect(page.locator("textarea")).toBeVisible();

  await page.getByTitle("打开项目档案").click();
  const delivery = page.locator("section").filter({ has: page.getByText("预览") }).last();

  await expect(delivery.getByText("预览未运行")).toBeVisible();
  await expect(delivery.getByRole("button", { name: "启动预览" })).toBeVisible();
  await expect(delivery.getByText("还没有检查点")).toBeVisible();
  await expect(delivery.getByRole("button", { name: "创建检查点" })).toBeVisible();
});
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "delivery shows preview action"
```

Expected: fail because `ProjectStatusCard` does not show action buttons.

- [ ] **Step 3: Add delivery confidence helper**

Create `src/lib/delivery-confidence.ts`:

```ts
import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";

export interface DeliveryConfidence {
  previewLabel: string;
  previewColor: string;
  previewAction: "open" | "start" | null;
  checkpointLabel: string;
  checkpointColor: string;
  checkpointAction: "create" | null;
  nextAction: string;
}

export function getDeliveryConfidence(
  runtime: ProjectRuntimeStatus | null,
  checkpoint: ProjectCheckpointStatus | null,
): DeliveryConfidence {
  const previewRunning = runtime?.running ?? false;
  const previewAction = runtime?.can_open ? "open" : runtime?.can_start ? "start" : null;
  const checkpointReady = Boolean(checkpoint?.last_checkpoint);
  const checkpointAction = checkpoint?.is_git_repo && !checkpointReady ? "create" : null;

  return {
    previewLabel: runtime
      ? previewRunning
        ? "预览运行中"
        : "预览未运行"
      : "预览状态未知",
    previewColor: previewRunning ? "#4A9E6B" : "#8C93A0",
    previewAction,
    checkpointLabel: checkpoint
      ? checkpointReady
        ? checkpoint.dirty
          ? "有检查点，当前有改动"
          : "检查点已就绪"
        : checkpoint.is_git_repo
          ? "还没有检查点"
          : "不是 Git 项目"
      : "检查点状态未知",
    checkpointColor: checkpointReady ? "#D4A853" : "#8C93A0",
    checkpointAction,
    nextAction: previewRunning
      ? "可以打开预览检查结果。"
      : runtime?.can_start
        ? "建议先启动预览，再检查第一版。"
        : "当前项目还不能直接预览。",
  };
}
```

- [ ] **Step 4: Wire actions into `ProjectStatusCard`**

Modify imports:

```ts
import { Play, ExternalLink, ShieldCheck } from "lucide-react";
import { getDeliveryConfidence } from "@/lib/delivery-confidence";
import {
  createProjectCheckpoint,
  getProjectCheckpointStatus,
  getProjectRuntimeStatus,
  openProjectPreview,
  startProjectDevServer,
  type ProjectCheckpointStatus,
  type ProjectRuntimeStatus,
} from "@/lib/tauri";
```

Inside the component, derive:

```ts
const confidence = getDeliveryConfidence(runtime, checkpoint);
```

Add action handlers:

```ts
const handlePreviewAction = async () => {
  setLoading(true);
  setError("");
  try {
    const nextRuntime = confidence.previewAction === "open"
      ? await openProjectPreview(sessionId ?? undefined)
      : await startProjectDevServer(sessionId ?? undefined);
    setRuntime(nextRuntime);
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  } finally {
    setLoading(false);
  }
};

const handleCheckpointAction = async () => {
  setLoading(true);
  setError("");
  try {
    const nextCheckpoint = await createProjectCheckpoint(sessionId ?? undefined);
    setCheckpoint(nextCheckpoint);
  } catch (err) {
    setError(err instanceof Error ? err.message : String(err));
  } finally {
    setLoading(false);
  }
};
```

Render below status lines:

```tsx
<div className="text-[11px] leading-relaxed text-muted-foreground">
  {confidence.nextAction}
</div>
<div className="flex flex-wrap gap-2">
  {confidence.previewAction && (
    <button type="button" onClick={handlePreviewAction} className="inline-flex items-center gap-1.5 rounded border border-border px-2 py-1 text-[11px] text-foreground hover:bg-secondary">
      {confidence.previewAction === "open" ? <ExternalLink className="size-3" /> : <Play className="size-3" />}
      {confidence.previewAction === "open" ? "打开预览" : "启动预览"}
    </button>
  )}
  {confidence.checkpointAction && (
    <button type="button" onClick={handleCheckpointAction} className="inline-flex items-center gap-1.5 rounded border border-border px-2 py-1 text-[11px] text-foreground hover:bg-secondary">
      <ShieldCheck className="size-3" />
      创建检查点
    </button>
  )}
</div>
```

- [ ] **Step 5: Run focused test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "delivery shows preview action"
```

Expected: pass.

- [ ] **Step 6: Commit Task 4**

```bash
git add src/lib/delivery-confidence.ts src/components/layout/ProjectStatusCard.tsx e2e/frontend.spec.ts
git commit -m "feat: make delivery status actionable"
```

## Task 5: Turn Closure Summary

**Files:**

- Modify: `src/lib/protocol.ts`
- Modify: `src-tauri/src/protocol/events.rs`
- Modify: `src/store/index.ts`
- Create: `src/components/messages/DeliverySummaryCard.tsx`
- Modify: `src/components/chat/MessageList.tsx`
- Modify: `src/hooks/useSession.ts`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add failing e2e test for completion summary**

Add this test inside `test.describe("First loop v0", ...)`:

```ts
test("shows a delivery summary after sending a first-loop request", async ({ page }) => {
  const sessionId = "delivery-summary-session";
  await setup(page);
  await page.addInitScript((sessionId) => {
    window.localStorage.clear();
    window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
  await page.locator("textarea").press("Enter");

  await expect(page.getByText("本轮交付")).toBeVisible();
  await expect(page.getByText("预览未运行")).toBeVisible();
  await expect(page.getByText("下一步")).toBeVisible();
});
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "shows a delivery summary"
```

Expected: fail because no delivery summary block exists.

- [ ] **Step 3: Add mirrored delivery summary event type**

Modify `src-tauri/src/protocol/events.rs` near the other protocol structs:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeliverySummary {
    pub preview_label: String,
    pub checkpoint_label: String,
    pub next_step: String,
}
```

Add to `StreamEvent`:

```rust
#[serde(rename = "delivery_summary")]
DeliverySummary {
    session_id: String,
    block_id: String,
    summary: DeliverySummary,
},
```

Add `DeliverySummary { session_id, .. }` to the `session_id()` match arm.

Modify `src/lib/protocol.ts`:

```ts
export interface DeliverySummary {
  preview_label: string;
  checkpoint_label: string;
  next_step: string;
}
```

Add to `StreamEvent`:

```ts
| {
    event_type: "delivery_summary";
    session_id: string;
    block_id: string;
    summary: DeliverySummary;
  }
```

The first implementation emits this event from the frontend after `sendInput` completes, but Rust and TypeScript protocol definitions stay mirrored so the backend can emit it later without another protocol cleanup pass.

- [ ] **Step 4: Store delivery summary blocks**

Modify `eventToBlock` in `src/store/index.ts`:

```ts
case "delivery_summary":
  return {
    ...base,
    event_type: "delivery_summary",
    content: "本轮交付",
    metadata: { summary: event.summary },
    isComplete: true,
  };
```

- [ ] **Step 5: Add DeliverySummaryCard**

Create `src/components/messages/DeliverySummaryCard.tsx`:

```tsx
import type { BlockState, DeliverySummary } from "@/lib/protocol";

export function DeliverySummaryCard({ block }: { block: BlockState }) {
  const summary = block.metadata.summary as DeliverySummary | undefined;
  if (!summary) return null;

  return (
    <div className="mb-3">
      <div className="rounded-lg border border-border bg-card px-4 py-3">
        <div className="mb-2 text-xs font-semibold text-foreground">本轮交付</div>
        <div className="space-y-1.5 text-xs">
          <Line label="预览" value={summary.preview_label} />
          <Line label="检查点" value={summary.checkpoint_label} />
          <Line label="下一步" value={summary.next_step} />
        </div>
      </div>
    </div>
  );
}

function Line({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[52px_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-foreground/85">{value}</span>
    </div>
  );
}
```

Modify `src/components/chat/MessageList.tsx`:

```ts
import { DeliverySummaryCard } from "@/components/messages/DeliverySummaryCard";
```

Add to `BlockRenderer`:

```tsx
case "delivery_summary": return <DeliverySummaryCard block={block} />;
```

- [ ] **Step 6: Emit summary after send completes**

Modify imports in `src/hooks/useSession.ts`:

```ts
import { createSession, resumeSession, sendInput, killSession, getProjectRuntimeStatus, getProjectCheckpointStatus } from "../lib/tauri";
```

In `send`, after `await sendInput(sessionId, text);`, add the complete guarded summary block:

```ts
try {
  const [runtime, checkpoint] = await Promise.all([
    getProjectRuntimeStatus(sessionId),
    getProjectCheckpointStatus(sessionId),
  ]);
  dispatchOutputEvent({
    event_type: "delivery_summary",
    session_id: sessionId,
    block_id: crypto.randomUUID(),
    summary: {
      preview_label: runtime.running ? "预览运行中" : "预览未运行",
      checkpoint_label: checkpoint.last_checkpoint
        ? checkpoint.dirty
          ? "有检查点，当前有改动"
          : "检查点已就绪"
        : checkpoint.is_git_repo
          ? "还没有检查点"
          : "检查点不可用",
      next_step: runtime.running
        ? "打开预览检查第一版。"
        : runtime.can_start
          ? "启动预览后检查第一版。"
          : "先补齐项目启动方式，再继续交付。",
    },
  });
} catch (summaryError) {
  console.warn("Failed to create delivery summary:", summaryError);
}
```

- [ ] **Step 7: Run focused test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "shows a delivery summary"
```

Expected: pass.

- [ ] **Step 8: Commit Task 5**

```bash
git add src-tauri/src/protocol/events.rs src/lib/protocol.ts src/store/index.ts src/components/messages/DeliverySummaryCard.tsx src/components/chat/MessageList.tsx src/hooks/useSession.ts e2e/frontend.spec.ts
git commit -m "feat: summarize delivery after meaningful turns"
```

## Task 6: Product Scan And Full Verification

**Files:**

- Modify: `e2e/frontend.spec.ts`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`

- [ ] **Step 1: Add or update acceptance prompt documentation**

Append to `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`:

````markdown
## Safety Delivery Loop

```text
请把首页标题改成 Forge Demo，并启动预览让我检查。
```

Expected:

- Before modifying files, Forge shows `准备修改项目`.
- The card shows workspace, operation, impact, risk, and recovery state.
- If the current workspace is Forge itself, the card shows a stronger warning.
- Delivery compactly shows preview and checkpoint state.
- After the turn, Forge shows what happened and the next step.
```
````

- [ ] **Step 2: Run product language scan**

Run:

```bash
rg -n --glob '!src-tauri/target/**' --glob '!node_modules/**' --glob '!dist/**' "PermissionDecision|ConfirmAsk|Project Status|runtime status|checkpoint internals|tool permission" src e2e src-tauri/src
```

Expected: matches only in internal Rust/TS protocol code, tests, or comments; no user-facing component copy.

- [ ] **Step 3: Run full verification**

Run:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npx playwright test e2e/frontend.spec.ts
```

Expected:

- TypeScript/Vite build exits 0.
- Rust compile check exits 0.
- Rust test suite exits 0.
- Playwright e2e exits 0.

- [ ] **Step 4: Commit docs and final polish**

```bash
git add e2e/frontend.spec.ts "/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md"
git commit -m "test: cover safety delivery loop"
```

If no tracked repo files changed in this task, skip the commit and note that Obsidian was updated outside the repo.

## Final Handoff

When all tasks are complete:

1. Check `git status --short`.
2. Confirm `website/` remains excluded unless the user explicitly asks to remove or commit it.
3. Push the current branch if requested:

```bash
git push
```

4. Report:

- commits created
- verification commands and results
- any deferred items

## Self-Review

Spec coverage:

- write-boundary confirmation: Tasks 1, 2, 3
- compact delivery confidence: Task 4
- turn closure: Task 5
- product language and acceptance: Task 6

Scope check:

- The plan does not implement diff review, git restore UI, document parsing, or a new dashboard.
- The plan reuses `ConfirmAsk`, `ConfirmCard`, `ProjectStatusCard`, runtime/checkpoint IPC, and the existing chat renderer.

Type consistency:

- Rust `WriteBoundary` uses snake-case enum serialization matching TypeScript `WriteBoundaryRisk`.
- `ConfirmAsk.boundary` is optional in both Rust and TypeScript.
- `DeliverySummary` is defined in both Rust and TypeScript, even though v0 emits it from the frontend after `sendInput` completes.
