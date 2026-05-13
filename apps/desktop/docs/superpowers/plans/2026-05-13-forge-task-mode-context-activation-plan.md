# Forge Task Mode and Context Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge visibly show the current working mode and the exact context used for the current turn, so Workflow Router, Living Wiki, and Forge Wiki feel like one coherent product loop.

**Architecture:** Keep the backend models intact and add frontend derivation helpers over existing `WorkflowState`, `SelectedContextMemory`, and `SelectedForgeWikiPage`. Update the right-side Context panel to prioritize current task, active context, and update inbox, then add a compact top-level indicator and mode-aware input copy. Add one focused backend memory suppression guard for "do not remember" user intent.

**Tech Stack:** React, TypeScript, Zustand, Tauri IPC wrappers, Rust memory extraction, Playwright e2e, Cargo tests, Vite build.

---

## 中文摘要

这次不是继续加新底层，而是把已经进 `main` 的 Workflow Router、Living Wiki、Forge Wiki 连接成用户能感知的工作流。

完成后，用户应该能直接看到：

- Forge 当前是在梳理想法、确认方案、拆步骤、制作、排查还是检查结果。
- 本轮到底带入了哪些背景、Wiki 页面、未来资料。
- 哪些记忆或 Wiki 更新只是建议，用户可以接受、编辑、忽略。
- 如果 Forge 判断错了，用户可以手动切换成直接回答、先拆方案、排查问题、检查结果。

本阶段不做 PDF/Word/PPT/Excel 解析，不做向量库，不做完整新手 Builder Wizard。

## File Structure

### Create

- `src/lib/task-mode.ts`
  - Derives product-facing Task Mode labels, helper copy, gate labels, override labels, and input placeholders from existing `WorkflowState`.
- `src/lib/context-activation.ts`
  - Normalizes selected Living Wiki memories and Forge Wiki pages into a shared active context item list.
- `src/components/context/ActiveContextSection.tsx`
  - Renders **本轮上下文** using normalized active context items.

### Modify

- `src/components/workflow/CurrentTaskCard.tsx`
  - Use `deriveTaskModeView()`, expose compact override actions, keep developer details collapsed.
- `src/components/workflow/WorkflowStatusPill.tsx`
  - Use the same Task Mode derivation and show active context count when provided.
- `src/components/layout/HubPanel.tsx`
  - Derive active context items from Zustand and render **本轮上下文** directly after **当前任务**.
  - Add `open-hub` event support in addition to existing `toggle-hub`.
- `src/components/context/WikiSections.tsx`
  - Remove duplicate "本轮带入" rendering after `ActiveContextSection` exists.
  - Rename candidate/proposal area to **建议更新记录**.
- `src/components/session/InputBar.tsx`
  - Use mode-aware placeholder and show `本轮已带入 N 条背景` from both memory and Forge Wiki context.
- `src/components/layout/AppShell.tsx`
  - Pass active context count to `WorkflowStatusPill`.
- `src-tauri/src/memory/extraction.rs`
  - Suppress durable memory candidates when the user explicitly says not to remember the turn.
- `e2e/frontend.spec.ts`
  - Add focused tests for Task Mode, Active Context, top-level indicator, Memory Inbox, and suppression behavior.
- `e2e/mock-ipc.ts`
  - Ensure mock IPC and stream events support the new e2e cases.

## Task 1: Task Mode Display Model

**Files:**
- Create: `src/lib/task-mode.ts`
- Modify: `src/components/workflow/CurrentTaskCard.tsx`
- Modify: `src/components/workflow/WorkflowStatusPill.tsx`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add the failing e2e test for stable Task Mode labels**

Append this test under a new `test.describe("Task Mode", ...)` block in `e2e/frontend.spec.ts`:

```ts
test.describe("Task Mode", () => {
  test("shows stable mode copy and manual override actions", async ({ page }) => {
    const sessionId = "task-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "workflow_updated",
        session_id: sessionId,
        state: {
          session_id: sessionId,
          route: "workflow",
          phase: "planning",
          beginner_label: "router raw label should not be the final UI label",
          developer_label: "workflow/planning",
          matched_signals: ["new feature", "multi component"],
          reason: "用户正在规划一个会影响多个部分的新能力。",
          gate: "soft",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        },
      },
    ], 5);

    await page.getByTitle("打开上下文").click();
    const currentTask = page.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });

    await expect(currentTask.getByText("拆成步骤")).toBeVisible();
    await expect(currentTask.getByText("正在拆成可执行步骤")).toBeVisible();
    await expect(currentTask.getByText("这个需求会影响多个部分")).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "直接回答" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "先拆方案" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "排查问题" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "检查结果" })).toBeVisible();
  });
});
```

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "stable mode copy"
```

Expected: FAIL because the UI still renders raw `workflow.beginner_label` and no inline override buttons.

- [ ] **Step 2: Create the Task Mode helper**

Create `src/lib/task-mode.ts`:

```ts
import type { WorkflowGate, WorkflowOverrideAction, WorkflowPhase, WorkflowRoute, WorkflowState } from "@/lib/protocol";

export type TaskModeId =
  | "ready"
  | "clarify"
  | "spec"
  | "plan"
  | "build"
  | "debug"
  | "verify"
  | "wrap";

export interface TaskModeView {
  id: TaskModeId;
  label: string;
  title: string;
  description: string;
  tone: "neutral" | "accent" | "warning" | "danger";
}

const MODE_COPY: Record<TaskModeId, TaskModeView> = {
  ready: {
    id: "ready",
    label: "准备判断",
    title: "正在判断工作方式",
    description: "Forge 会根据你的请求选择直接回答、规划、执行或排查。",
    tone: "neutral",
  },
  clarify: {
    id: "clarify",
    label: "梳理想法",
    title: "正在把想法整理清楚",
    description: "适合新功能、产品方向、需求还不完整的任务。",
    tone: "accent",
  },
  spec: {
    id: "spec",
    label: "确认方案",
    title: "先确认方案再继续",
    description: "这个任务可能影响多个部分，建议先看方案。",
    tone: "warning",
  },
  plan: {
    id: "plan",
    label: "拆成步骤",
    title: "正在拆成可执行步骤",
    description: "Forge 会把方案变成小步任务，便于执行和验证。",
    tone: "accent",
  },
  build: {
    id: "build",
    label: "开始制作",
    title: "正在处理项目",
    description: "Forge 可能会读写文件、运行命令或更新界面。",
    tone: "accent",
  },
  debug: {
    id: "debug",
    label: "排查问题",
    title: "正在定位问题",
    description: "Forge 会先收集症状，再做有依据的修复。",
    tone: "danger",
  },
  verify: {
    id: "verify",
    label: "检查结果",
    title: "正在检查结果",
    description: "Forge 会跑构建、测试或查看关键状态。",
    tone: "neutral",
  },
  wrap: {
    id: "wrap",
    label: "整理结果",
    title: "正在整理完成情况",
    description: "Forge 会说明改了什么、验证了什么、还剩什么。",
    tone: "neutral",
  },
};

export function deriveTaskModeView(workflow: WorkflowState | null): TaskModeView {
  if (!workflow) return MODE_COPY.ready;
  return MODE_COPY[deriveTaskModeId(workflow.route, workflow.phase, workflow.gate)];
}

function deriveTaskModeId(route: WorkflowRoute, phase: WorkflowPhase, gate: WorkflowGate): TaskModeId {
  if (gate === "approval_required") return "spec";
  if (route === "recovery" || phase === "debugging" || phase === "blocked") return "debug";
  if (route === "verification" || phase === "verifying") return "verify";
  if (phase === "done") return "wrap";
  if (phase === "planning") return "plan";
  if (phase === "spec" || phase === "designing") return "spec";
  if (phase === "clarifying" || route === "workflow" || route === "strict_workflow") return "clarify";
  if (phase === "executing" || route === "light") return "build";
  return "ready";
}

export function taskGateLabel(gate: WorkflowGate): string {
  if (gate === "approval_required") return "需确认";
  if (gate === "soft") return "建议";
  return "直接";
}

export function taskGateCopy(gate: WorkflowGate): string | null {
  if (gate === "approval_required") return "这个请求风险较高，建议先确认方案和步骤。";
  if (gate === "soft") return "这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。";
  return null;
}

export function workflowOverrideLabel(action: WorkflowOverrideAction): string {
  switch (action) {
    case "direct":
      return "直接回答";
    case "plan_first":
      return "先拆方案";
    case "debug":
      return "排查问题";
    case "verify":
      return "检查结果";
  }
}

export function modeAwarePlaceholder(workflow: WorkflowState | null, isRunning: boolean): string {
  if (!isRunning) return "这个会话已停止，可以继续后再发送";
  const mode = deriveTaskModeView(workflow).id;
  switch (mode) {
    case "clarify":
      return "描述目标、使用者、输入和输出。";
    case "spec":
      return "看完方案后，可以说“开始做”或指出要改哪里。";
    case "plan":
      return "可以补充约束，或说“按这个计划执行”。";
    case "build":
      return "继续描述修改，Forge 会处理项目。";
    case "debug":
      return "粘贴报错、失败现象或复现步骤。";
    case "verify":
      return "说要检查什么，或让 Forge 跑构建/测试。";
    case "wrap":
      return "可以继续追问结果，或指定下一步。";
    case "ready":
      return "说说你想做什么，Forge 会判断下一步。";
  }
}
```

- [ ] **Step 3: Update `CurrentTaskCard` to use derived copy and override actions**

Modify `src/components/workflow/CurrentTaskCard.tsx`:

```ts
import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { WorkflowOverrideAction, WorkflowState } from "@/lib/protocol";
import { overrideWorkflowRoute } from "@/lib/tauri";
import { useStore } from "@/store";
import { deriveTaskModeView, taskGateCopy, taskGateLabel, workflowOverrideLabel } from "@/lib/task-mode";

export function CurrentTaskCard({ workflow }: { workflow: WorkflowState | null }) {
  const [expanded, setExpanded] = useState(false);
  const [busyAction, setBusyAction] = useState<WorkflowOverrideAction | null>(null);
  const setWorkflowState = useStore((s) => s.setWorkflowState);
  const mode = deriveTaskModeView(workflow);
  const gateCopy = workflow ? taskGateCopy(workflow.gate) : null;

  const handleOverride = async (action: WorkflowOverrideAction) => {
    if (!workflow || busyAction) return;
    setBusyAction(action);
    try {
      const next = await overrideWorkflowRoute(workflow.session_id, action);
      setWorkflowState(workflow.session_id, next);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">当前任务</h3>
        <span className="text-[10px] text-muted-foreground/70">自动判断</span>
      </div>
      <div className="rounded-md border border-border bg-card px-3 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{mode.label}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{mode.title}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground/80">{workflow?.reason || mode.description}</div>
          </div>
          <span className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {workflow ? taskGateLabel(workflow.gate) : "等待"}
          </span>
        </div>

        {gateCopy && (
          <div className="mt-2 rounded border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-200/90">
            {gateCopy}
          </div>
        )}

        {workflow && workflow.override_actions.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {workflow.override_actions.map((action) => (
              <button
                key={action}
                type="button"
                disabled={busyAction !== null}
                onClick={() => handleOverride(action)}
                className="rounded border border-border px-2 py-1 text-[10px] text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-default disabled:opacity-60"
              >
                {busyAction === action ? "切换中" : workflowOverrideLabel(action)}
              </button>
            ))}
          </div>
        )}

        {/* keep the existing developer details block below this point */}
      </div>
    </section>
  );
}
```

Keep the existing `Row` component and developer detail contents. Replace old `gateLabel()` with `taskGateLabel()`.

- [ ] **Step 4: Update `WorkflowStatusPill` to use the same derivation**

Modify `src/components/workflow/WorkflowStatusPill.tsx` signature and content:

```ts
export function WorkflowStatusPill({
  workflow,
  activeContextCount = 0,
  onOpenContext,
}: {
  workflow: WorkflowState | null;
  activeContextCount?: number;
  onOpenContext?: () => void;
}) {
  if (!workflow) return null;

  const mode = deriveTaskModeView(workflow);
  const strict = workflow.gate === "approval_required";
  const label = activeContextCount > 0 ? `${mode.label} · 已带入 ${activeContextCount}` : mode.label;

  return (
    <button
      type="button"
      data-testid="workflow-status-pill"
      onClick={onOpenContext}
      className={cn(
        "inline-flex min-w-0 max-w-[220px] shrink items-center gap-1 rounded-md border px-2 py-0.5 text-[10px] transition-colors",
        strict ? "border-amber-500/30 text-amber-300" : "border-border text-muted-foreground",
        onOpenContext && "hover:bg-secondary hover:text-foreground",
      )}
      title={`${workflow.developer_label}: ${workflow.reason}`}
    >
      {strict ? <ShieldAlert className="size-3" /> : <Compass className="size-3" />}
      <span className="truncate">{label}</span>
    </button>
  );
}
```

Add imports:

```ts
import { deriveTaskModeView } from "@/lib/task-mode";
```

- [ ] **Step 5: Run the focused e2e and build**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "Task Mode"
npm run build
```

Expected:

- Playwright test PASS.
- `npm run build` exits 0.

- [ ] **Step 6: Commit**

```bash
git add src/lib/task-mode.ts src/components/workflow/CurrentTaskCard.tsx src/components/workflow/WorkflowStatusPill.tsx e2e/frontend.spec.ts
git commit -m "feat: show task mode state"
```

## Task 2: Active Context Bundle

**Files:**
- Create: `src/lib/context-activation.ts`
- Create: `src/components/context/ActiveContextSection.tsx`
- Modify: `src/components/layout/HubPanel.tsx`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add the failing e2e test for active context**

Append this test under `test.describe("Context Activation", ...)` in `e2e/frontend.spec.ts`:

```ts
test.describe("Context Activation", () => {
  test("shows active memories and Forge Wiki pages for the current turn", async ({ page }) => {
    const sessionId = "context-activation-session";
    const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedMemory = {
      memory_id: "memory-1",
      title: "中文优先",
      body: "用户偏好中文沟通，英文能力稍弱。",
      category: "preference" as const,
      scope: "user_profile" as const,
      score: 0.93,
      reason: "这是你固定的偏好",
      injected: true,
    };
    const selectedPage = {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks" as const,
      summary: "当前正在做 Task Mode 和 Context Activation。",
      score: 0.91,
      reason: "这页项目记录与本轮请求相关",
      injected: true,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("tui-to-gui-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_selection", session_id: sessionId, selected: [selectedMemory] },
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await page.getByTitle("打开上下文").click();
    const activeContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮上下文" }) });

    await expect(activeContext.getByText("已带入 2 条背景")).toBeVisible();
    await expect(activeContext.getByText("中文优先")).toBeVisible();
    await expect(activeContext.getByText("当前任务")).toBeVisible();
    await expect(activeContext.getByText("这是你固定的偏好")).toBeVisible();
    await expect(activeContext.getByText("这页项目记录与本轮请求相关")).toBeVisible();
  });
});
```

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "active memories"
```

Expected: FAIL because there is no normalized **本轮上下文** section yet.

- [ ] **Step 2: Create the context activation helper**

Create `src/lib/context-activation.ts`:

```ts
import type { SelectedContextMemory, SelectedForgeWikiPage } from "@/lib/protocol";

export type ActiveContextKind = "memory" | "forge_wiki_page";

export interface ActiveContextItem {
  id: string;
  kind: ActiveContextKind;
  title: string;
  summary: string;
  reason: string;
  injected: boolean;
  score?: number;
  sourceLabel: string;
  sourcePath?: string;
}

export function getActiveContextItems(
  memories: SelectedContextMemory[],
  pages: SelectedForgeWikiPage[],
): ActiveContextItem[] {
  const memoryItems = memories.map((memory): ActiveContextItem => ({
    id: memory.memory_id,
    kind: "memory",
    title: memory.title,
    summary: memory.body,
    reason: memory.reason,
    injected: memory.injected,
    score: memory.score,
    sourceLabel: memoryCategoryLabel(memory.category),
  }));

  const pageItems = pages.map((page): ActiveContextItem => ({
    id: page.page_id,
    kind: "forge_wiki_page",
    title: page.title,
    summary: page.summary,
    reason: page.reason,
    injected: page.injected,
    score: page.score,
    sourceLabel: "项目记录",
    sourcePath: page.path,
  }));

  return [...memoryItems, ...pageItems].sort((a, b) => Number(b.injected) - Number(a.injected) || (b.score ?? 0) - (a.score ?? 0));
}

export function countInjectedContext(items: ActiveContextItem[]): number {
  return items.filter((item) => item.injected).length;
}

export function activeContextSummary(items: ActiveContextItem[]): string {
  const injected = countInjectedContext(items);
  if (items.length === 0) return "本轮没有带入额外背景";
  if (injected === 0) return `找到 ${items.length} 条相关背景`;
  return `已带入 ${injected} 条背景`;
}

function memoryCategoryLabel(category: SelectedContextMemory["category"]): string {
  switch (category) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目信息";
    case "decision":
      return "已定方案";
    case "task_state":
      return "当前进度";
  }
}
```

- [ ] **Step 3: Create `ActiveContextSection`**

Create `src/components/context/ActiveContextSection.tsx`:

```tsx
import { BookOpen, Database, MinusCircle } from "lucide-react";
import type { ActiveContextItem } from "@/lib/context-activation";
import { activeContextSummary } from "@/lib/context-activation";
import { cn } from "@/lib/utils";

export function ActiveContextSection({ items }: { items: ActiveContextItem[] }) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">本轮上下文</h3>
        <span className="text-[10px] text-muted-foreground/70">{activeContextSummary(items)}</span>
      </div>

      {items.length === 0 ? (
        <div className="rounded-md border border-border bg-card px-3 py-4 text-center text-xs text-muted-foreground">
          本轮没有带入额外背景
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((item) => (
            <ActiveContextRow key={`${item.kind}:${item.id}`} item={item} />
          ))}
        </div>
      )}
    </section>
  );
}

function ActiveContextRow({ item }: { item: ActiveContextItem }) {
  const Icon = item.kind === "forge_wiki_page" ? BookOpen : Database;

  return (
    <article className="rounded-md border border-border bg-card px-3 py-2.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-1.5">
            <Icon className="size-3 shrink-0 text-muted-foreground" />
            <span className="truncate text-xs font-medium text-foreground">{item.title}</span>
          </div>
          <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{item.summary}</p>
          <p className="mt-1 text-[10px] leading-relaxed text-muted-foreground/75">{item.reason}</p>
        </div>
        <span
          className={cn(
            "shrink-0 rounded border px-1.5 py-0.5 text-[10px]",
            item.injected ? "border-primary/30 text-primary" : "border-border text-muted-foreground",
          )}
        >
          {item.injected ? "已带入" : "未使用"}
        </span>
      </div>
      <div className="mt-2 flex items-center justify-between gap-2">
        <span className="truncate text-[10px] text-muted-foreground/70">
          {item.sourceLabel}{item.sourcePath ? ` · ${item.sourcePath}` : ""}
        </span>
        <button
          type="button"
          disabled
          title="后续支持从本轮移除"
          className="inline-flex items-center gap-1 text-[10px] text-muted-foreground/50"
        >
          <MinusCircle className="size-3" />
          本轮移除
        </button>
      </div>
    </article>
  );
}
```

The remove action is visibly reserved but disabled in this slice. It becomes interactive after a backend exclusion ledger exists.

- [ ] **Step 4: Render active context in `HubPanel`**

Modify `src/components/layout/HubPanel.tsx`:

```ts
import { ActiveContextSection } from "@/components/context/ActiveContextSection";
import { getActiveContextItems } from "@/lib/context-activation";
```

Add selectors:

```ts
const selectedMemories = useStore((s) => activeId ? s.selectedContextBySession.get(activeId) ?? [] : []);
const selectedWikiPages = useStore((s) => activeId ? s.forgeWikiContextBySession.get(activeId) ?? [] : []);
const activeContextItems = getActiveContextItems(selectedMemories, selectedWikiPages);
```

Render after `CurrentTaskCard`:

```tsx
<CurrentTaskCard workflow={workflow} />
<ActiveContextSection items={activeContextItems} />
<WikiSections sessionId={activeId} projectPath={projectPath} />
```

- [ ] **Step 5: Run focused test and build**

```bash
npx playwright test e2e/frontend.spec.ts -g "active memories"
npm run build
```

Expected:

- Playwright test PASS.
- `npm run build` exits 0.

- [ ] **Step 6: Commit**

```bash
git add src/lib/context-activation.ts src/components/context/ActiveContextSection.tsx src/components/layout/HubPanel.tsx e2e/frontend.spec.ts
git commit -m "feat: show active context bundle"
```

## Task 3: Top-Level Indicator And Mode-Aware Input

**Files:**
- Modify: `src/components/layout/AppShell.tsx`
- Modify: `src/components/layout/HubPanel.tsx`
- Modify: `src/components/workflow/WorkflowStatusPill.tsx`
- Modify: `src/components/session/InputBar.tsx`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add the failing e2e test for top-level mode and context count**

Append to `test.describe("Task Mode", ...)`:

```ts
test("top-level mode pill opens the Context panel and shows context count", async ({ page }) => {
  const sessionId = "top-level-mode-session";
  await setup(page);
  await page.addInitScript((sessionId) => {
    window.localStorage.clear();
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话" }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    {
      event_type: "workflow_updated",
      session_id: sessionId,
      state: {
        session_id: sessionId,
        route: "workflow",
        phase: "clarifying",
        beginner_label: "raw",
        developer_label: "workflow/clarifying",
        matched_signals: ["idea"],
        reason: "用户正在描述一个新工具。",
        gate: "soft",
        override_actions: ["direct", "plan_first", "debug", "verify"],
        spec_path: null,
        plan_path: null,
        checkpoint_id: null,
        updated_at: Date.now(),
      },
    },
    {
      event_type: "forge_wiki_context_selected",
      session_id: sessionId,
      selected: [{
        page_id: "tasks",
        title: "当前任务",
        path: "tasks.md",
        kind: "tasks",
        summary: "正在做上下文激活。",
        score: 0.9,
        reason: "这页项目记录与本轮请求相关",
        injected: true,
      }],
    },
  ], 5);

  const pill = page.getByTestId("workflow-status-pill");
  await expect(pill).toContainText("梳理想法");
  await expect(pill).toContainText("已带入 1");
  await pill.click();

  await expect(page.getByRole("complementary").getByText("上下文")).toBeVisible();
  await expect(page.getByRole("heading", { name: "本轮上下文" })).toBeVisible();
});
```

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "top-level mode pill"
```

Expected: FAIL because `WorkflowStatusPill` does not yet receive context count or open the panel.

- [ ] **Step 2: Support an explicit `open-hub` event**

Modify the event effect in `src/components/layout/HubPanel.tsx`:

```ts
useEffect(() => {
  const toggleHandler = () => setOpen((value) => !value);
  const openHandler = () => setOpen(true);
  window.addEventListener("toggle-hub", toggleHandler);
  window.addEventListener("open-hub", openHandler);
  return () => {
    window.removeEventListener("toggle-hub", toggleHandler);
    window.removeEventListener("open-hub", openHandler);
  };
}, []);
```

- [ ] **Step 3: Pass active context count from `AppShell`**

Modify `src/components/layout/AppShell.tsx`:

```ts
const selectedMemoryCount = useStore((s) => activeSessionId ? s.selectedContextBySession.get(activeSessionId)?.filter((item) => item.injected).length ?? 0 : 0);
const selectedWikiPageCount = useStore((s) => activeSessionId ? s.forgeWikiContextBySession.get(activeSessionId)?.filter((item) => item.injected).length ?? 0 : 0);
const activeContextCount = selectedMemoryCount + selectedWikiPageCount;
```

Update the pill:

```tsx
<WorkflowStatusPill
  workflow={workflow}
  activeContextCount={activeContextCount}
  onOpenContext={() => window.dispatchEvent(new Event("open-hub"))}
/>
```

- [ ] **Step 4: Make input copy mode-aware**

Modify `src/components/session/InputBar.tsx` imports:

```ts
import { modeAwarePlaceholder } from "@/lib/task-mode";
```

Replace the selected context count:

```ts
const selectedMemoryContextCount = useStore((s) => s.selectedContextBySession.get(sessionId)?.filter((item) => item.injected).length ?? 0);
const selectedWikiContextCount = useStore((s) => s.forgeWikiContextBySession.get(sessionId)?.filter((item) => item.injected).length ?? 0);
const selectedContextCount = selectedMemoryContextCount + selectedWikiContextCount;
```

Replace:

```tsx
上轮带入 {selectedContextCount} 条相关背景
```

with:

```tsx
本轮已带入 {selectedContextCount} 条背景
```

Replace the textarea placeholder:

```tsx
placeholder={modeAwarePlaceholder(workflow, isRunning)}
```

- [ ] **Step 5: Run focused tests and build**

```bash
npx playwright test e2e/frontend.spec.ts -g "top-level mode pill|InputBar"
npm run build
```

Expected:

- Playwright tests PASS.
- `npm run build` exits 0.

- [ ] **Step 6: Commit**

```bash
git add src/components/layout/AppShell.tsx src/components/layout/HubPanel.tsx src/components/workflow/WorkflowStatusPill.tsx src/components/session/InputBar.tsx e2e/frontend.spec.ts
git commit -m "feat: add task mode indicator"
```

## Task 4: Memory Inbox And Right Panel Polish

**Files:**
- Modify: `src/components/context/WikiSections.tsx`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add the failing e2e test for the unified inbox**

Append to `test.describe("Living Wiki context panel", ...)`:

```ts
test("groups memory candidates and Wiki proposals as update suggestions", async ({ page }) => {
  const sessionId = "memory-inbox-session";
  const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
  const now = "2026-05-13T00:00:00.000Z";
  const candidateMemory = {
    id: "candidate-1",
    category: "decision" as const,
    scope: "project" as const,
    status: "candidate" as const,
    title: "项目已定方案：上下文优先",
    body: "右侧面板优先展示当前任务和本轮上下文。",
    project_path: projectPath,
    source_session_id: sessionId,
    source_message_ids: [],
    confidence: 0.76,
    created_at: now,
    updated_at: now,
    last_used_at: null,
    use_count: 0,
    tags: ["decision"],
  };
  const proposal = {
    id: "proposal-1",
    project_path: projectPath,
    session_id: sessionId,
    target_pages: ["tasks.md"],
    title: "记录上下文激活计划",
    summary: "补充 Task Mode 和 Context Activation 的下一步。",
    patch_preview: "追加任务记录。",
    status: "pending" as const,
    created_at: now,
  };

  await setup(page);
  await page.addInitScript(({ sessionId, projectPath, candidateMemory }) => {
    window.localStorage.clear();
    window.localStorage.setItem("tui-to-gui-working-dir", projectPath);
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string) => {
      if (cmd === "create_session") return { session_id: sessionId };
      if (cmd === "get_default_working_dir") return projectPath;
      if (cmd === "get_project_runtime_status") return {
        working_dir: projectPath,
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
      if (cmd === "list_memories") return [candidateMemory];
      if (cmd === "get_forge_wiki_state") return { project_path: projectPath, exists: true, wiki_dir: `${projectPath}/.forge/wiki`, pages: [], message: "ready" };
      return undefined;
    };
  }, { sessionId, projectPath, candidateMemory });

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话" }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
  ], 5);

  await page.getByTitle("打开上下文").click();
  const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

  await expect(inbox.getByText(candidateMemory.body)).toBeVisible();
  await expect(inbox.getByText(proposal.summary)).toBeVisible();
  await expect(inbox.getByRole("button", { name: "接受" }).first()).toBeVisible();
  await expect(inbox.getByRole("button", { name: /忽略|丢弃/ }).first()).toBeVisible();
});
```

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "unified inbox"
```

Expected: FAIL because the current heading is split between candidate/proposal copy and "建议更新项目记录".

- [ ] **Step 2: Remove duplicate selected context from `WikiSections`**

In `src/components/context/WikiSections.tsx`, remove the section whose title is currently `"本轮带入"`. `ActiveContextSection` now owns active context.

Keep all state selectors because proposal/candidate/project memory rendering still needs them.

- [ ] **Step 3: Rename the proposal/candidate area to `建议更新记录`**

In `WikiSections`, update the section title:

```tsx
<Section title="建议更新记录" meta={pendingForgeWikiProposals.length + candidateMemories.length > 0 ? `${pendingForgeWikiProposals.length + candidateMemories.length} 条` : undefined}>
```

Use empty state:

```tsx
<EmptyState label="没有待确认的记录更新" />
```

Render candidate memories and Forge Wiki proposals inside this one section. Candidate memory cards should show:

- category label from the existing memory label helper
- title
- body
- actions: `接受`, `编辑`, `忘记`

Forge Wiki proposal cards should show:

- title
- summary
- target pages
- actions: `接受`, `丢弃`

- [ ] **Step 4: Update existing e2e selectors that referenced old headings**

In `e2e/frontend.spec.ts`, replace old heading expectations:

```ts
page.getByRole("heading", { name: "建议更新项目记录" })
```

with:

```ts
page.getByRole("heading", { name: "建议更新记录" })
```

Remove expectations for the old `"本轮带入"` section and point active-context assertions at `"本轮上下文"`.

- [ ] **Step 5: Run context panel tests and build**

```bash
npx playwright test e2e/frontend.spec.ts -g "Living Wiki context panel|Context Activation"
npm run build
```

Expected:

- Playwright tests PASS.
- `npm run build` exits 0.

- [ ] **Step 6: Commit**

```bash
git add src/components/context/WikiSections.tsx e2e/frontend.spec.ts
git commit -m "feat: polish memory update inbox"
```

## Task 5: Memory Suppression And Acceptance Coverage

**Files:**
- Modify: `src-tauri/src/memory/extraction.rs`
- Modify: `e2e/frontend.spec.ts`
- Modify: `e2e/mock-ipc.ts` only if a shared mock helper needs a default memory candidate.

- [ ] **Step 1: Add failing Rust tests for "do not remember" wording**

Add these tests to `src-tauri/src/memory/extraction.rs` inside the existing `#[cfg(test)]` module:

```rust
#[test]
fn suppresses_memory_when_user_says_do_not_remember() {
    let candidates = extract_candidates_from_user_message(
        "session-1",
        Some("/tmp/project"),
        "不要记住这个，只是临时测试：以后这个项目默认用亮色主题。",
    );

    assert!(candidates.is_empty());
}

#[test]
fn suppresses_memory_when_user_says_session_only() {
    let candidates = extract_candidates_from_user_message(
        "session-1",
        Some("/tmp/project"),
        "这条不要作为长期偏好，只在本轮里用：以后回答都短一点。",
    );

    assert!(candidates.is_empty());
}
```

Run:

```bash
cargo test memory::extraction --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL because suppression is not implemented yet.

- [ ] **Step 2: Implement suppression helper in extraction**

Modify `src-tauri/src/memory/extraction.rs`:

```rust
pub fn extract_candidates_from_user_message(
    session_id: &str,
    project_path: Option<&str>,
    text: &str,
) -> Vec<WikiMemory> {
    let body = collapse_whitespace(text);
    if body.chars().count() < 8 || should_suppress_persistent_memory(&body) || should_reject_persistent_memory(&body) {
        return Vec::new();
    }

    // existing candidate extraction stays unchanged
}

fn should_suppress_persistent_memory(text: &str) -> bool {
    contains_any(
        text,
        &[
            "不要记住",
            "别记住",
            "不要保存",
            "别保存",
            "不要作为长期偏好",
            "不是长期偏好",
            "只是临时",
            "临时测试",
            "只在本轮",
            "只在这次",
            "do not remember",
            "don't remember",
            "do not save",
            "temporary only",
            "just for this turn",
        ],
    )
}
```

Keep `should_suppress_persistent_memory` private unless another module needs it later.

- [ ] **Step 3: Add e2e acceptance coverage for no durable candidate**

Append to `test.describe("Context Activation", ...)`:

```ts
test("does not show a memory candidate when user says not to remember", async ({ page }) => {
  const sessionId = "no-memory-session";
  await setup(page);
  await page.addInitScript((sessionId) => {
    window.localStorage.clear();
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话" }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await page.locator("textarea").fill("不要记住这个，只是临时测试：以后默认用亮色主题。");
  await page.locator("textarea").press("Enter");
  await page.getByTitle("打开上下文").click();

  const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });
  await expect(inbox.getByText("以后默认用亮色主题")).not.toBeVisible();
});
```

This e2e test guards the UI from showing a durable candidate in the no-remember scenario. The Rust test is the authoritative backend behavior.

- [ ] **Step 4: Run acceptance checks**

Run:

```bash
cargo test memory::extraction --manifest-path src-tauri/Cargo.toml
npx playwright test e2e/frontend.spec.ts -g "not to remember|Context Activation|Task Mode|Living Wiki context panel"
npm run build
```

Expected:

- Cargo tests PASS.
- Playwright tests PASS.
- `npm run build` exits 0.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/memory/extraction.rs e2e/frontend.spec.ts e2e/mock-ipc.ts
git commit -m "fix: respect no-memory requests"
```

## Final Verification

After all tasks are complete, run the full verification set:

```bash
cargo test memory --manifest-path src-tauri/Cargo.toml
npm run build
npx playwright test e2e/frontend.spec.ts -g "Task Mode|Context Activation|Living Wiki context panel"
```

Expected:

- Cargo memory tests pass.
- TypeScript and Vite build pass.
- Focused Playwright coverage passes.

Then inspect the final diff:

```bash
git status --short
git log --oneline -5
git diff --stat main...HEAD
```

Expected:

- Worktree clean after commits.
- Five focused commits.
- Diff touches only the files listed in this plan.

## Manual Acceptance Prompts

Use these prompts in the app after implementation:

1. `我想做一个记账小工具，但我完全不会写代码`
   - Expect: mode `梳理想法`; right panel explains current task and active context.

2. `继续上次那个 Forge Wiki 方向，把它做得更像产品`
   - Expect: mode `梳理想法` or `拆成步骤`; active context shows Forge Wiki / product direction records.

3. `不要记住这个，只是临时测试：sk-1234567890abcdefghijkl`
   - Expect: no durable memory candidate containing the key.

4. `直接回答，不要改文件：auto compact 是什么？`
   - Expect: direct or ready mode; no implementation gate.

5. `跑一下 npm run build，确认 main 没问题`
   - Expect: mode `检查结果`; response includes verification evidence.

6. `这个预览打不开，帮我看下`
   - Expect: mode `排查问题`; debugging path and context details are visible.

7. `以后这个项目的 UI 都尽量深色、克制、紧凑`
   - Expect: visible memory candidate in **建议更新记录**.

8. `这条不要作为长期偏好`
   - Expect: no persistent memory candidate.

## Rollback Notes

If UI changes become too noisy, revert in reverse order:

1. Revert Task 4 to restore old `WikiSections` layout.
2. Revert Task 3 to remove top-level pill behavior and input copy changes.
3. Revert Task 2 to remove `ActiveContextSection`.
4. Keep Task 5 if tests pass; respecting "do not remember" is a safety improvement independent of the UI.
5. Revert Task 1 only if derived Task Mode labels are causing incorrect product promises.
