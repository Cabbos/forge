# First Loop v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge's first small-tool loop feel ready, guided, and recoverable before and after the first meaningful request.

**Architecture:** Add pure view-model helpers for readiness and first-loop progress, then render compact React cards/strips around existing chat surfaces. Reuse existing IPC for API key, runtime, checkpoint, and delivery actions; do not add backend features unless a required signal is missing.

**Tech Stack:** React, TypeScript, Zustand, Tauri IPC wrappers, Playwright e2e, existing shadcn-style dark UI.

---

## File Structure

- Create `src/lib/start-readiness.ts`: pure helper that maps workspace, provider key status, runtime, and checkpoint state to product rows.
- Create `src/components/session/StartReadinessCard.tsx`: compact `准备开始` card for empty/early sessions.
- Modify `src/components/chat/MessageList.tsx`: render readiness card instead of the generic empty chat copy when a session exists and no blocks exist.
- Create `src/lib/first-loop-progress.ts`: pure helper that derives the five first-loop phases from blocks and first-loop draft state.
- Create `src/components/session/FirstLoopProgressStrip.tsx`: one-line phase strip.
- Modify `src/components/session/SessionView.tsx`: render progress strip below the existing task popover.
- Modify `src/components/messages/DeliverySummaryCard.tsx`: add next-action buttons that load clear follow-up prompts.
- Modify `src/components/chat/MessageList.tsx`: pass `sessionId` to `DeliverySummaryCard`.
- Modify `e2e/frontend.spec.ts`: add focused coverage for readiness, progress, and delivery summary actions.
- Modify `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Next Two Weeks.md`: mark First Loop v1 as the current focus.
- Modify `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`: add First Loop v1 acceptance prompt.

## Task 1: Start Readiness Card

**Files:**
- Create: `src/lib/start-readiness.ts`
- Create: `src/components/session/StartReadinessCard.tsx`
- Modify: `src/components/chat/MessageList.tsx`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Write failing e2e test**

Add a test under `First loop v0` or a new `First loop v1` describe block:

```ts
test("empty session shows start readiness", async ({ page }) => {
  const sessionId = "first-loop-readiness";
  await setup(page);
  await page.addInitScript((sessionId) => {
    window.localStorage.clear();
    window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();

  const main = page.getByRole("main");
  await expect(main.getByText("准备开始")).toBeVisible();
  await expect(main.getByText("工作空间")).toBeVisible();
  await expect(main.getByText("模型密钥")).toBeVisible();
  await expect(main.getByText("预览")).toBeVisible();
  await expect(main.getByText("检查点")).toBeVisible();
});
```

- [ ] **Step 2: Run red test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "empty session shows start readiness"
```

Expected: FAIL because `准备开始` is not rendered.

- [ ] **Step 3: Implement readiness helper**

Create `src/lib/start-readiness.ts` with:

```ts
import type { KeyStatus, ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";
import type { Workspace } from "@/lib/workspaces";

export type ReadinessTone = "ready" | "warning" | "blocked" | "muted";
export type ReadinessAction = "open_settings" | "start_preview" | "create_checkpoint" | null;

export interface StartReadinessRow {
  label: string;
  value: string;
  tone: ReadinessTone;
  action: ReadinessAction;
  actionLabel: string | null;
}

export interface StartReadinessView {
  title: string;
  subtitle: string;
  issueCount: number;
  rows: StartReadinessRow[];
}

export function deriveStartReadiness(input: {
  workspace: Workspace | null;
  providerId: string;
  providerLabel: string;
  keyStatuses: KeyStatus[];
  runtime: ProjectRuntimeStatus | null;
  checkpoint: ProjectCheckpointStatus | null;
}): StartReadinessView {
  const keySet = input.keyStatuses.some((item) => item.provider === input.providerId && item.set);
  const rows: StartReadinessRow[] = [
    {
      label: "工作空间",
      value: input.workspace ? `已选择 ${input.workspace.name}` : "还没有选择项目",
      tone: input.workspace ? "ready" : "blocked",
      action: null,
      actionLabel: null,
    },
    {
      label: "模型密钥",
      value: keySet ? `${input.providerLabel} 已配置` : `还没有配置 ${input.providerLabel}`,
      tone: keySet ? "ready" : "blocked",
      action: keySet ? null : "open_settings",
      actionLabel: keySet ? null : "打开设置",
    },
    {
      label: "预览",
      value: input.runtime?.running
        ? "预览运行中"
        : input.runtime?.can_start
          ? "可启动"
          : "没有检测到 dev 脚本",
      tone: input.runtime?.running || input.runtime?.can_start ? "ready" : "warning",
      action: input.runtime?.can_start && !input.runtime.running ? "start_preview" : null,
      actionLabel: input.runtime?.can_start && !input.runtime.running ? "启动预览" : null,
    },
    {
      label: "检查点",
      value: input.checkpoint?.last_checkpoint
        ? "检查点已就绪"
        : input.checkpoint?.is_git_repo
          ? "可创建"
          : "当前不是 Git 项目",
      tone: input.checkpoint?.last_checkpoint || input.checkpoint?.is_git_repo ? "ready" : "warning",
      action: input.checkpoint?.is_git_repo && !input.checkpoint.last_checkpoint ? "create_checkpoint" : null,
      actionLabel: input.checkpoint?.is_git_repo && !input.checkpoint.last_checkpoint ? "创建检查点" : null,
    },
  ];

  const issueCount = rows.filter((row) => row.tone === "blocked" || row.tone === "warning").length;
  return {
    title: "准备开始",
    subtitle: issueCount === 0 ? "可以开始做第一版小工具。" : "开始前有几项可以先确认。",
    issueCount,
    rows,
  };
}
```

- [ ] **Step 4: Implement card and render it in empty sessions**

Create `StartReadinessCard` that calls:

- `getApiKeyStatus()`
- `getProjectRuntimeStatus(sessionId)`
- `getProjectCheckpointStatus(sessionId)`

Render rows from `deriveStartReadiness`. For actions:

- `open_settings`: dispatch `new Event("forge:open-settings")`, which is already handled by `SettingsDialog`.
- `start_preview`: call `startProjectDevServer(sessionId)` then refresh.
- `create_checkpoint`: call `createProjectCheckpoint(sessionId)` then refresh.

Modify `MessageList` empty state:

```tsx
if (blocks.length === 0) {
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-8 py-10">
      <StartReadinessCard sessionId={sessionId} />
    </div>
  );
}
```

- [ ] **Step 5: Run green test and build**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "empty session shows start readiness"
npm run build
```

Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add src/lib/start-readiness.ts src/components/session/StartReadinessCard.tsx src/components/chat/MessageList.tsx e2e/frontend.spec.ts
git commit -m "feat: show first loop readiness"
```

## Task 2: First Loop Progress Strip

**Files:**
- Create: `src/lib/first-loop-progress.ts`
- Create: `src/components/session/FirstLoopProgressStrip.tsx`
- Modify: `src/components/session/SessionView.tsx`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Write failing e2e test**

```ts
test("first loop progress advances after a request", async ({ page }) => {
  const sessionId = "first-loop-progress";
  await setup(page);
  await page.addInitScript((sessionId) => {
    window.localStorage.clear();
    window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();

  await expect(page.getByText("理解目标")).toBeVisible();

  await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
  await page.locator("textarea").press("Enter");

  await expect(page.getByText("正在制作")).toBeVisible();
  await expect(page.getByText("等你验收")).toBeVisible();
});
```

- [ ] **Step 2: Run red test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "first loop progress advances"
```

Expected: FAIL because progress strip is missing.

- [ ] **Step 3: Implement progress helper**

Create `src/lib/first-loop-progress.ts` with five phase labels and a function that marks current phase:

- no draft and no delivery summary: `understand`
- first-loop draft exists: `making`
- delivery summary exists: `review`
- confirm card pending: `prepare`
- runtime running in summary text: `preview`

Keep the helper defensive and pure.

- [ ] **Step 4: Render strip in SessionView**

`FirstLoopProgressStrip` reads active blocks and first-loop draft from store and renders labels:

- 理解目标
- 准备修改
- 正在制作
- 可以预览
- 等你验收

Use compact dots and muted labels. No cards.

- [ ] **Step 5: Run green test and build**

```bash
npx playwright test e2e/frontend.spec.ts -g "first loop progress advances"
npm run build
```

- [ ] **Step 6: Commit**

```bash
git add src/lib/first-loop-progress.ts src/components/session/FirstLoopProgressStrip.tsx src/components/session/SessionView.tsx e2e/frontend.spec.ts
git commit -m "feat: show first loop progress"
```

## Task 3: Delivery Summary Actions

**Files:**
- Modify: `src/components/messages/DeliverySummaryCard.tsx`
- Modify: `src/components/chat/MessageList.tsx`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Write failing e2e test**

```ts
test("delivery summary offers follow-up actions", async ({ page }) => {
  const sessionId = "first-loop-delivery-actions";
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

  await expect(page.getByRole("button", { name: "检查风险" })).toBeVisible();
  await expect(page.getByRole("button", { name: "继续优化" })).toBeVisible();

  await page.getByRole("button", { name: "检查风险" }).click();
  await expect(page.locator("textarea")).toContainText("检查刚才的改动有没有风险");
});
```

- [ ] **Step 2: Run red test**

```bash
npx playwright test e2e/frontend.spec.ts -g "delivery summary offers follow-up actions"
```

Expected: FAIL because buttons are missing.

- [ ] **Step 3: Add action buttons**

Modify `DeliverySummaryCard` to accept `sessionId?: string`, import `useStore`, and add two prompt-loading buttons:

- 检查风险: `请检查刚才的改动有没有风险、遗漏或需要我确认的地方，并按严重程度排序。`
- 继续优化: `请基于当前结果，继续找一个最影响使用体验的问题并直接优化，最后给我验收提示词。`

Modify `MessageList` to pass `sessionId` to `DeliverySummaryCard`.

- [ ] **Step 4: Run green test and build**

```bash
npx playwright test e2e/frontend.spec.ts -g "delivery summary offers follow-up actions"
npm run build
```

- [ ] **Step 5: Commit**

```bash
git add src/components/messages/DeliverySummaryCard.tsx src/components/chat/MessageList.tsx e2e/frontend.spec.ts
git commit -m "feat: add delivery follow-up actions"
```

## Task 4: Product Scan, Docs, And Verification

**Files:**
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Next Two Weeks.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/04 Acceptance/Acceptance Prompts.md`
- Test: `e2e/frontend.spec.ts`

- [ ] **Step 1: Update Obsidian roadmap and acceptance prompts**

Add First Loop v1 as the current focus and add a prompt:

```text
我想做一个番茄钟小工具，可以开始、暂停、重置。请做到第一版可以预览，并在过程中告诉我当前进展。
```

Expected:

- 准备开始 appears before the first request.
- Progress strip shows 理解目标、准备修改、正在制作、可以预览、等你验收.
- 本轮交付 appears after the request.
- 检查风险 and 继续优化 are available.

- [ ] **Step 2: Run product language scan**

```bash
rg -n --glob '!src-tauri/target/**' --glob '!node_modules/**' --glob '!dist/**' "Workflow Router|Task Mode|Living Wiki|Forge Wiki|Context Activation|Project Status|runtime status|checkpoint internals|tool permission" src e2e src-tauri/src
```

Expected: no user-facing component copy matches. Internal Rust protocol/test matches are acceptable if not rendered.

- [ ] **Step 3: Run full verification**

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npx playwright test e2e/frontend.spec.ts
```

Expected: all pass.

- [ ] **Step 4: Commit test/doc polish if repo files changed**

```bash
git add e2e/frontend.spec.ts
git commit -m "test: cover first loop v1"
```

If only Obsidian files changed, skip repo commit and mention the external note update.
