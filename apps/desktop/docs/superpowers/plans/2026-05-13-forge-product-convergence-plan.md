# Forge Product Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge's visible product model converge around 当前任务, 上下文, and 交付 without adding new capabilities.

**Architecture:** Keep all backend behavior intact. Apply a frontend-only information architecture and copy pass over the existing Workflow, Memory, Forge Wiki, Context Activation, and Project Status surfaces.

**Tech Stack:** React, TypeScript, Zustand, Playwright, Vite, existing Tauri IPC.

---

## File Structure

- Modify `src/components/layout/HubPanel.tsx`
  - Rename panel title to `工作台`.
  - Group visible sections into `当前任务`, `上下文`, and `交付`.
  - Keep the existing resources placeholder under `上下文`.

- Modify `src/components/context/WikiSections.tsx`
  - Replace primary user-facing `项目 Wiki` and `上下文记忆` wording.
  - Keep the underlying component and IPC names unchanged.

- Modify `src/components/workflow/WorkflowStatusPill.tsx`
  - Keep mode labels, but make tooltip user-facing.

- Modify `src/components/session/InputBar.tsx`
  - Rename active skills hover label from plugin-oriented wording to capability/support wording.

- Modify `e2e/frontend.spec.ts`
  - Update assertions for the converged product language.
  - Add focused coverage that the right panel exposes the three product layers.

## Task 1: Right Panel Product Layers

- [ ] **Step 1: Add a failing e2e assertion**

In `e2e/frontend.spec.ts`, extend the top-level mode/context test to open the right panel and assert:

```ts
await expect(page.locator("aside").last().getByText("工作台", { exact: true }).first()).toBeVisible();
await expect(page.getByRole("heading", { name: "当前任务" })).toBeVisible();
await expect(page.getByRole("heading", { name: "上下文", exact: true })).toBeVisible();
await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
```

- [ ] **Step 2: Run the focused test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "top-level mode pill"
```

Expected: FAIL because the right panel title is still `上下文` and there is no `交付` heading.

- [ ] **Step 3: Update `HubPanel.tsx`**

Change the panel title to `工作台`. Add a compact `ProductLayerHeader` helper and render:

```tsx
<CurrentTaskCard workflow={workflow} />

<ProductLayerHeader title="上下文" meta={activeContextItems.length > 0 ? `${activeContextItems.length} 条` : null} />
<ActiveContextSection items={activeContextItems} />
<WikiSections sessionId={activeId} projectPath={projectPath} />
<ContextFilesSection files={contextFiles} />

<ProductLayerHeader title="交付" meta="最近状态" />
<ProjectStatusCard sessionId={activeId} />
```

Do not add new IPC or resource behavior.

- [ ] **Step 4: Run the focused test again**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "top-level mode pill"
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/layout/HubPanel.tsx e2e/frontend.spec.ts
git commit -m "feat: converge workbench panel structure"
```

## Task 2: Hide Internal Wiki And Memory Terms

- [ ] **Step 1: Add/update e2e assertions**

In the Living Wiki context panel tests, replace primary copy expectations:

```ts
await expect(projectRecords.getByText("还没有项目记录", { exact: true })).toBeVisible();
await page.getByRole("button", { name: "建立项目记录" }).click();
await expect(page.getByRole("heading", { name: "已保存背景" })).toBeVisible();
```

- [ ] **Step 2: Run the focused test**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "Living Wiki context panel"
```

Expected: FAIL on old `项目 Wiki` / `上下文记忆` copy.

- [ ] **Step 3: Update `WikiSections.tsx`**

Replace only visible text:

- `打开项目后可以建立项目 Wiki` -> `打开项目后可以建立项目记录`
- `还没有项目 Wiki` -> `还没有项目记录`
- `建立项目 Wiki` -> `建立项目记录`
- `上下文记忆` -> `已保存背景`
- `还没有上下文记忆` -> `还没有已保存背景`

Keep component names and backend types unchanged.

- [ ] **Step 4: Run the focused test again**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "Living Wiki context panel"
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/context/WikiSections.tsx e2e/frontend.spec.ts
git commit -m "feat: hide internal context vocabulary"
```

## Task 3: Top-Level Copy Polish

- [ ] **Step 1: Add/update assertions**

Update tests that open the right panel to expect `工作台`. Keep the toolbar button allowed to say `工作台`.

- [ ] **Step 2: Update user-facing copy**

In `WorkflowStatusPill.tsx`, replace the title with:

```tsx
title={workflow.reason || mode.description}
```

In `InputBar.tsx`, replace the active skills tooltip label:

```tsx
<div className="text-[10px] uppercase tracking-wider text-muted-foreground/70 mb-1">已启用能力</div>
```

In `AppShell.tsx`, rename the right toolbar button visible copy from `上下文` to `工作台`.

- [ ] **Step 3: Run task/context tests**

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "Task Mode|Context Activation|Living Wiki context panel|Workflow Router"
```

Expected: PASS after assertion updates.

- [ ] **Step 4: Build**

Run:

```bash
npm run build
```

Expected: PASS. Existing Vite chunk-size warning is acceptable.

- [ ] **Step 5: Commit**

```bash
git add src/components/workflow/WorkflowStatusPill.tsx src/components/session/InputBar.tsx src/components/layout/AppShell.tsx e2e/frontend.spec.ts
git commit -m "feat: polish product-facing copy"
```

## Final Verification

Run:

```bash
npm run build
npx playwright test e2e/frontend.spec.ts -g "Task Mode|Context Activation|Living Wiki context panel|Workflow Router"
git status --short --branch
```

Expected:

- Build exits 0.
- Focused e2e passes.
- Worktree is clean except intentional branch state.
