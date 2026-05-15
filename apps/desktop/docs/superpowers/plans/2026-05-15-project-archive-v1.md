# Project Archive v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight 项目概览 card so returning users can see project state and continue from the Project Archive panel.

**Architecture:** Derive a pure Project Overview view model from existing workspace, session, first-loop draft, and persisted blocks. Render it as a small top card in the right-side 项目档案 panel, with actions that load continuation prompts into the input box.

**Tech Stack:** React, TypeScript, Zustand, Playwright e2e, existing Tauri IPC wrappers only.

---

## File Structure

- Create `src/lib/project-archive-overview.ts`
  - Pure derivation logic for title, goal, current version, next step, and action prompts.
- Create `src/components/context/ProjectOverviewCard.tsx`
  - UI component for the Project Archive top card.
- Modify `src/components/layout/HubPanel.tsx`
  - Select active blocks/workspace/session and render `ProjectOverviewCard` before `CurrentTaskCard`.
- Modify `e2e/frontend.spec.ts`
  - Add Project Archive v1 e2e coverage for restored session and continuation prompt.
- Modify Obsidian notes after implementation:
  - `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Next Two Weeks.md`
  - `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`

## Task 1: Add Failing E2E For Returning To A Project

**Files:**
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Add the failing test**

Add this test near the First Loop / Project records area:

```ts
test.describe("Project Archive v1", () => {
  test("restored project archive shows overview and continuation actions", async ({ page }) => {
    const sessionId = "project-archive-return-session";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(async ({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: projectPath, name: "forge", path: projectPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(projectPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: projectPath,
          workspaceId: projectPath,
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "return-user-message",
          event_type: "user_message",
          content: "我想做一个番茄钟小工具，可以开始、暂停、重置。",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "return-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: {
            summary: {
              project_path: projectPath,
              preview_label: "预览可打开",
              checkpoint_label: "检查点已就绪",
              next_action: "下一步：继续调整计时器的视觉反馈。",
            },
          },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByTitle("打开项目档案").click();

    const archive = page.locator("aside").last();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("番茄钟小工具")).toBeVisible();
    await expect(archive.getByText("预览可打开 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：继续调整计时器的视觉反馈。")).toBeVisible();
    await expect(archive.getByRole("button", { name: "继续上次任务" })).toBeVisible();

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "restored project archive shows overview"
```

Expected: FAIL because 项目概览 does not exist.

## Task 2: Implement Project Overview Derivation

**Files:**
- Create: `src/lib/project-archive-overview.ts`

- [ ] **Step 1: Create the helper**

Implement:

```ts
import type { BlockState, DeliverySummary, SessionState } from "@/lib/protocol";
import type { FirstLoopDraft } from "@/lib/first-loop";
import { deriveFirstLoopDraft } from "@/lib/first-loop";
import type { Workspace } from "@/lib/workspaces";

export type ProjectOverviewActionId = "continue_last_task" | "check_current_version" | "continue_polish";

export interface ProjectOverviewAction {
  id: ProjectOverviewActionId;
  label: string;
  prompt: string;
}

export interface ProjectArchiveOverview {
  projectName: string;
  projectPath: string;
  goal: string;
  currentVersion: string;
  nextStep: string;
  actions: ProjectOverviewAction[];
}

export function deriveProjectArchiveOverview(input: {
  workspace: Workspace | null;
  session: SessionState | null;
  blocks: BlockState[];
  firstLoopDraft: FirstLoopDraft | null;
}): ProjectArchiveOverview {
  const projectPath = normalizeProjectPath(input.session?.workingDir || input.workspace?.path || "");
  const projectName = input.workspace?.name || nameFromPath(projectPath) || "当前项目";
  const latestUserMessage = latestBlock(input.blocks, "user_message")?.content.trim() ?? "";
  const latestDelivery = parseDeliverySummary(latestBlock(input.blocks, "delivery_summary")?.metadata.summary);
  const derivedDraft = input.firstLoopDraft ?? (latestUserMessage ? deriveFirstLoopDraft(input.session?.id ?? "project", latestUserMessage) : null);

  const goal = derivedDraft?.goal || latestUserMessage || "等待你描述这个项目要做什么。";
  const currentVersion = latestDelivery
    ? `${latestDelivery.preview_label} · ${latestDelivery.checkpoint_label}`
    : derivedDraft?.scope || "还没有形成可验收版本";
  const nextStep = latestDelivery?.next_action || derivedDraft?.nextStep || "描述一个小工具，Forge 会先推进到可预览第一版。";

  return {
    projectName,
    projectPath: projectPath || "暂无项目路径",
    goal,
    currentVersion,
    nextStep,
    actions: [
      {
        id: "continue_last_task",
        label: "继续上次任务",
        prompt: `继续上次任务：${goal}\n\n请先说明当前项目进展，再直接推进下一步。`,
      },
      {
        id: "check_current_version",
        label: "检查当前版本",
        prompt: "请检查当前版本是否可预览、核心动作是否能完成，并列出需要我验收的地方。",
      },
      {
        id: "continue_polish",
        label: "继续优化",
        prompt: `请基于当前版本继续优化：${nextStep}\n\n优先处理最影响使用体验的一点。`,
      },
    ],
  };
}

function latestBlock(blocks: BlockState[], eventType: BlockState["event_type"]) {
  return [...blocks].reverse().find((block) => block.event_type === eventType);
}

function parseDeliverySummary(value: unknown): DeliverySummary | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Partial<Record<keyof DeliverySummary, unknown>>;
  const preview = stringValue(record.preview_label);
  const checkpoint = stringValue(record.checkpoint_label);
  if (!preview || !checkpoint) return null;
  return {
    project_path: stringValue(record.project_path),
    preview_label: preview,
    checkpoint_label: checkpoint,
    next_action: stringValue(record.next_action) ?? "下一步：继续检查交付状态。",
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}

function nameFromPath(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? "";
}
```

## Task 3: Render Project Overview In 项目档案

**Files:**
- Create: `src/components/context/ProjectOverviewCard.tsx`
- Modify: `src/components/layout/HubPanel.tsx`

- [ ] **Step 1: Create the card component**

Render title, path, goal, current version, next step, and action buttons. Use `setPendingInput` from the store. Keep the style compact, dark, and tool-like.

- [ ] **Step 2: Wire into `HubPanel`**

Use `useActiveBlocks()` and render `ProjectOverviewCard` before `CurrentTaskCard`.

- [ ] **Step 3: Run target e2e**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "restored project archive shows overview"
```

Expected: PASS.

- [ ] **Step 4: Run build**

Run:

```bash
npm run build
```

Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add e2e/frontend.spec.ts src/lib/project-archive-overview.ts src/components/context/ProjectOverviewCard.tsx src/components/layout/HubPanel.tsx
git commit -m "feat: add project archive overview"
```

## Task 4: Update Knowledge Base

**Files:**
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Next Two Weeks.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`

- [ ] **Step 1: Update roadmap**

Add Project Archive v1 as the next continuity step:

```md
Project Archive v1 standard:

> A returning user can see what the project is, where the current version stopped, and which continuation action to take.
```

- [ ] **Step 2: Add acceptance prompt**

Add:

```text
打开一个已有番茄钟项目，不要让我重新解释。请告诉我当前版本做到哪，并给我一个继续入口。
```

Expected:

- 项目档案 shows 项目概览.
- 项目概览 includes 当前版本 and 下一步.
- 继续上次任务 loads a prompt into the input box.

## Task 5: Full Verification And Push

- [ ] **Step 1: Run product language scan**

```bash
rg -n --glob '!src-tauri/target/**' --glob '!node_modules/**' --glob '!dist/**' "Workflow Router|Task Mode|Living Wiki|Forge Wiki|Context Activation|Project Status|runtime status|checkpoint internals|tool permission" src e2e src-tauri/src
```

Expected: no output, exit 1.

- [ ] **Step 2: Run full build and tests**

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npx playwright test e2e/frontend.spec.ts
```

Expected: all exit 0.

- [ ] **Step 3: Commit any remaining tracked changes**

Do not stage `website/`.

- [ ] **Step 4: Push**

```bash
git push
```
