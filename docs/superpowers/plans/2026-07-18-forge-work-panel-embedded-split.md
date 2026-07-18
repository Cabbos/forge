# Forge Work Panel Embedded Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the framed, percentage-sized Work Panel sheet with a flush `440px` native split whose compact launcher has no default selection and whose selected objects fill the panel directly.

**Architecture:** Keep the existing task-scoped tab and adapter model, but persist panel width in pixels, collapse the responsive modes to `split` and flush `full`, and let `WorkPanelLayout` own all workspace sizing. Split the root five-action keyboard list out of `WorkPanelLauncher`; keep cmdk only for the preview/file/subtask choosers, and preserve the current theme tokens while changing structural CSS.

**Tech Stack:** React 18, TypeScript, Base UI, cmdk, `react-resizable-panels`, GSAP with reduced-motion support, Playwright, Node test runner.

**Design spec:** `docs/superpowers/specs/2026-07-18-forge-work-panel-quiet-native-redesign-design.md`

---

## Execution Safety

The current main worktree contains user-owned theme and design edits, including changes in `apps/desktop/src/styles/work-panel.css`, the desktop E2E specs, and user-facing docs. Do not discard, overwrite, or silently include those edits.

Before Task 1:

1. Run `git status --short` and record the dirty paths.
2. Execute from a clean implementation worktree only after the user's current theme baseline exists on a commit or branch that the worktree can include.
3. If implementation must remain in the dirty worktree, stage only reviewed hunks and prove with `git diff --cached --name-only` plus `git diff --cached` that no unrelated theme work entered the task commit.
4. Before editing each indexed function or component, run GitNexus upstream impact analysis for the symbols named in that task. Report direct callers, affected processes, and risk. Stop and warn the user before any HIGH or CRITICAL edit.
5. Before every commit, stage only that task's files and run GitNexus `detect_changes({scope: "staged", repo: "forge"})`.

## File Map

### Create

- `apps/desktop/src/components/workpanel/WorkPanelActionList.tsx`: the five root actions, keyboard navigation, and initial unselected state.

### Modify

- `apps/desktop/src/components/workpanel/workPanelDimensions.ts`: pixel width, split/full breakpoint, and conversion helpers.
- `apps/desktop/src/components/workpanel/workPanelDimensions.test.ts`: pure dimension contract.
- `apps/desktop/src/components/workpanel/workPanelTypes.ts`: persisted `widthPx` task state.
- `apps/desktop/src/components/workpanel/workPanelState.ts`: preserve normalized pixel width across tab transitions.
- `apps/desktop/src/components/workpanel/workPanelState.test.ts`: v3 persistence and width preservation.
- `apps/desktop/src/components/workpanel/workPanelPersistence.ts`: migrate legacy v1/v2 records to v3 `widthPx`.
- `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`: flush split/full composition and pixel persistence.
- `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`: remove sheet-entry translation and expose pixel state.
- `apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx`: delegate the root action list and retain focused choosers.
- `apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx`: keep all object controls in one aligned top band.
- `apps/desktop/src/styles/work-panel.css`: remove the outer frame and apply the approved launcher geometry without changing theme tokens.
- `apps/desktop/e2e/acceptance.spec.ts`: embedded split, width, launcher, chooser, restore, content-fill, and narrow-window acceptance.
- `apps/desktop/e2e/chrome.spec.ts`: narrow full-workbench geometry.
- `apps/desktop/e2e/messages.spec.ts`: replace obsolete Work Panel header assertions with launcher rhythm assertions.
- `apps/desktop/e2e/guardrails.spec.ts`: recognize the focused action-list component and preserve mature primitives.
- `README.md`: replace the old percentage/overlay product description.
- `apps/desktop/README.md`: document the flush split and updated acceptance surface.
- `CHANGELOG.md`: describe the user-visible redesign.

No backend, memory, continuity, terminal authority, preview authority, or theme-token file changes belong to this plan.

### Task 1: Define the pixel dimension contract

**Files:**
- Modify: `apps/desktop/src/components/workpanel/workPanelDimensions.test.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelDimensions.ts`

- [ ] **Step 1: Run pre-change impact analysis**

Run GitNexus upstream impact for `normalizeWorkPanelWidthPercent`, `getWorkPanelViewportMode`, `getWorkPanelBounds`, and `clampWorkPanelWidthPercent` in `apps/desktop/src/components/workpanel/workPanelDimensions.ts`. Include tests in the report.

- [ ] **Step 2: Replace the percentage tests with the failing pixel contract**

```ts
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  DEFAULT_WORK_PANEL_WIDTH_PX,
  clampWorkPanelWidthPx,
  getWorkbenchWidth,
  getWorkPanelBounds,
  getWorkPanelViewportMode,
  normalizeWorkPanelWidthPx,
  workPanelPercentToPx,
  workPanelPxToPercent,
} from "./workPanelDimensions.ts";

describe("work panel dimensions", () => {
  it("normalizes persisted pixel widths", () => {
    assert.equal(DEFAULT_WORK_PANEL_WIDTH_PX, 440);
    assert.equal(normalizeWorkPanelWidthPx(undefined), 440);
    assert.equal(normalizeWorkPanelWidthPx(120), 360);
    assert.equal(normalizeWorkPanelWidthPx(2_000), 920);
  });

  it("uses a flush full-workbench mode below the split threshold", () => {
    assert.equal(getWorkPanelViewportMode(840), "split");
    assert.equal(getWorkPanelViewportMode(839), "full");
    assert.equal(getWorkPanelViewportMode(Number.NaN), "full");
  });

  it("leaves at least 400px for the conversation", () => {
    assert.deepEqual(getWorkPanelBounds(1_200), { min: 360, max: 800 });
    assert.deepEqual(getWorkPanelBounds(2_000), { min: 360, max: 920 });
    assert.equal(clampWorkPanelWidthPx(900, { min: 360, max: 800 }), 800);
  });

  it("converts between the panel library percentage and persisted pixels", () => {
    assert.equal(workPanelPxToPercent(440, 1_200), 36.67);
    assert.equal(workPanelPercentToPx(36.67, 1_200), 440);
    assert.equal(getWorkbenchWidth(1_440), 1_156);
  });
});
```

- [ ] **Step 3: Run the focused test and verify it fails**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelDimensions.test.ts`

Expected: FAIL because the pixel constants and helpers do not exist.

- [ ] **Step 4: Implement the pixel helpers**

Replace `workPanelDimensions.ts` with:

```ts
export const DEFAULT_WORK_PANEL_WIDTH_PX = 440;
export const MIN_WORK_PANEL_WIDTH_PX = 360;
export const MAX_WORK_PANEL_WIDTH_PX = 920;
export const MIN_CONVERSATION_WIDTH_PX = 400;
export const FULL_WORK_PANEL_BREAKPOINT_PX = 840;

export type WorkPanelViewportMode = "split" | "full";

export function normalizeWorkPanelWidthPx(widthPx: number | undefined): number {
  if (typeof widthPx !== "number" || !Number.isFinite(widthPx)) return DEFAULT_WORK_PANEL_WIDTH_PX;
  return Math.min(MAX_WORK_PANEL_WIDTH_PX, Math.max(MIN_WORK_PANEL_WIDTH_PX, Math.round(widthPx)));
}

export function getWorkPanelViewportMode(workbenchWidth: number): WorkPanelViewportMode {
  return Number.isFinite(workbenchWidth) && workbenchWidth >= FULL_WORK_PANEL_BREAKPOINT_PX ? "split" : "full";
}

export function getWorkbenchWidth(viewportWidth: number): number {
  const safeViewportWidth = Number.isFinite(viewportWidth) ? viewportWidth : 0;
  return Math.max(MIN_WORK_PANEL_WIDTH_PX, safeViewportWidth - 284);
}

export function getWorkPanelBounds(workbenchWidth: number): { min: number; max: number } {
  const safeWidth = Number.isFinite(workbenchWidth) ? Math.max(MIN_WORK_PANEL_WIDTH_PX, workbenchWidth) : MIN_WORK_PANEL_WIDTH_PX;
  const max = Math.max(MIN_WORK_PANEL_WIDTH_PX, Math.min(MAX_WORK_PANEL_WIDTH_PX, safeWidth - MIN_CONVERSATION_WIDTH_PX));
  return { min: Math.min(MIN_WORK_PANEL_WIDTH_PX, max), max };
}

export function clampWorkPanelWidthPx(widthPx: number, bounds: { min: number; max: number }): number {
  return Math.min(bounds.max, Math.max(bounds.min, normalizeWorkPanelWidthPx(widthPx)));
}

export function workPanelPxToPercent(widthPx: number, workbenchWidth: number): number {
  if (!Number.isFinite(workbenchWidth) || workbenchWidth <= 0) return 100;
  return round(widthPx / workbenchWidth * 100);
}

export function workPanelPercentToPx(widthPercent: number, workbenchWidth: number): number {
  if (!Number.isFinite(widthPercent) || !Number.isFinite(workbenchWidth)) return DEFAULT_WORK_PANEL_WIDTH_PX;
  return Math.round(workbenchWidth * widthPercent / 100);
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
```

- [ ] **Step 5: Run the focused test and verify it passes**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelDimensions.test.ts`

Expected: PASS with four dimension tests.

- [ ] **Step 6: Review affected scope and commit**

Stage the two dimension files, run GitNexus staged `detect_changes`, then commit:

```bash
git add apps/desktop/src/components/workpanel/workPanelDimensions.ts apps/desktop/src/components/workpanel/workPanelDimensions.test.ts
git commit -m "refactor(desktop): define pixel work panel bounds"
```

### Task 2: Migrate task state from percentage to pixels

**Files:**
- Modify: `apps/desktop/src/components/workpanel/workPanelTypes.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelState.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelState.test.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelPersistence.ts`

- [ ] **Step 1: Run pre-change impact analysis**

Run GitNexus upstream impact for `restoreTaskPanelState`, `openWorkPanelTab`, `closeWorkPanelTab`, `loadWorkPanelTasks`, and `saveWorkPanelTask`. Report all direct callers before editing.

- [ ] **Step 2: Write failing state and migration assertions**

Change the state expectations from `widthPercent` to `widthPx` and add:

```ts
it("starts at the approved pixel width", () => {
  assert.equal(restoreTaskPanelState(null).widthPx, 440);
});

it("preserves explicit pixel width after the final tab closes", () => {
  const opened = openWorkPanelTab(restoreTaskPanelState(null), {
    kind: "file", id: "file:a.ts", label: "a.ts", path: "a.ts",
  });
  assert.equal(closeWorkPanelTab({ ...opened, widthPx: 612 }, "file:a.ts").widthPx, 612);
});

it("migrates v1 and v2 percentage records to the new default", () => {
  const storage = memoryStorage({
    [WORK_PANEL_STORAGE_KEY]: JSON.stringify({
      version: 2,
      tasks: { "task-1": { tabs: [], activeTabId: null, launcherOpen: true, widthPercent: 62 } },
    }),
  });
  assert.equal(loadWorkPanelTasks(storage)["task-1"]?.widthPx, 440);
});

it("normalizes v3 pixel records", () => {
  const storage = memoryStorage({
    [WORK_PANEL_STORAGE_KEY]: JSON.stringify({
      version: 3,
      tasks: { "task-1": { tabs: [], activeTabId: null, launcherOpen: true, widthPx: 2_000 } },
    }),
  });
  assert.equal(loadWorkPanelTasks(storage)["task-1"]?.widthPx, 920);
});
```

- [ ] **Step 3: Run the focused state test and verify it fails**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelState.test.ts`

Expected: FAIL because `WorkPanelTaskState` still exposes `widthPercent` and storage only understands v1/v2.

- [ ] **Step 4: Change the serializable state contract**

Use this state shape:

```ts
export interface WorkPanelTaskState {
  tabs: WorkPanelTab[];
  activeTabId: string | null;
  launcherOpen: boolean;
  widthPx: number;
}
```

In `workPanelState.ts`, import `normalizeWorkPanelWidthPx`, set the empty width to `440`, and preserve `widthPx` in every transition:

```ts
const EMPTY_WORK_PANEL_STATE: WorkPanelTaskState = {
  tabs: [],
  activeTabId: null,
  launcherOpen: true,
  widthPx: 440,
};

// restoreTaskPanelState
widthPx: normalizeWorkPanelWidthPx(state?.widthPx),

// open/close transitions
widthPx: state.widthPx,
```

- [ ] **Step 5: Add v3 persistence with deterministic legacy migration**

Update the storage contract and width parsing:

```ts
export interface WorkPanelStorage {
  version: 3;
  tasks: Record<string, WorkPanelTaskState>;
}

if (!isRecord(parsed) || ![1, 2, 3].includes(Number(parsed.version)) || !isRecord(parsed.tasks)) return {};

const widthPx = parsed.version === 3 && typeof value.widthPx === "number"
  ? value.widthPx
  : 440;
tasks[key] = restoreTaskPanelState({ tabs, activeTabId: requestedActiveId, launcherOpen, widthPx });

storage.setItem(WORK_PANEL_STORAGE_KEY, JSON.stringify({ version: 3, tasks } satisfies WorkPanelStorage));
```

Legacy v1/v2 records intentionally migrate to `440px`; do not infer pixels from an old percentage without a recorded workbench width.

- [ ] **Step 6: Run state and dimension tests**

Run:

```bash
cd apps/desktop
node --test src/components/workpanel/workPanelState.test.ts src/components/workpanel/workPanelDimensions.test.ts
```

Expected: PASS.

- [ ] **Step 7: Review affected scope and commit**

Stage only the four task-state files, run staged `detect_changes`, then commit:

```bash
git add apps/desktop/src/components/workpanel/workPanelTypes.ts apps/desktop/src/components/workpanel/workPanelState.ts apps/desktop/src/components/workpanel/workPanelState.test.ts apps/desktop/src/components/workpanel/workPanelPersistence.ts
git commit -m "refactor(desktop): persist work panel width in pixels"
```

### Task 3: Replace the framed sheet with a native split

**Files:**
- Modify: `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Run pre-change impact analysis**

Run GitNexus upstream impact for `WorkPanelLayout` and `WorkPanelShell`. The report must cover `AppShell`, global panel events, persistence, and the acceptance flow before edits begin.

- [ ] **Step 2: Replace the old breathing-room and overlay tests with a failing embedded-split test**

Add this acceptance case and remove the assertions that require outer spacing, `12px` radius, shadow, `40%`, or `overlay`:

```ts
test("work panel is a flush 440px embedded split", async ({ page }) => {
  await page.setViewportSize({ width: 1_440, height: 900 });
  await page.getByRole("button", { name: "打开工作面板" }).click();
  const panel = page.getByRole("complementary", { name: "工作面板" });
  await expect(panel).toHaveAttribute("data-viewport-mode", "split");
  await expect(panel).toHaveAttribute("data-width-px", "440");

  const metrics = await panel.evaluate((element) => {
    const workbench = document.querySelector<HTMLElement>("[data-testid='main-workbench']")!;
    const panelRect = element.getBoundingClientRect();
    const workbenchRect = workbench.getBoundingClientRect();
    const style = getComputedStyle(element);
    return {
      topGap: Math.round(panelRect.top - workbenchRect.top),
      rightGap: Math.round(workbenchRect.right - panelRect.right),
      bottomGap: Math.round(workbenchRect.bottom - panelRect.bottom),
      width: Math.round(panelRect.width),
      radius: style.borderRadius,
      shadow: style.boxShadow,
    };
  });

  expect(metrics).toEqual({ topGap: 0, rightGap: 0, bottomGap: 0, width: 440, radius: "0px", shadow: "none" });
});

test("work panel uses a flush full-workbench fallback", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 720 });
  await page.getByRole("button", { name: "打开工作面板" }).click();
  const panel = page.getByRole("complementary", { name: "工作面板" });
  await expect(panel).toHaveAttribute("data-viewport-mode", "full");
  await expect(page.getByRole("separator", { name: "调整工作面板宽度" })).toHaveCount(0);
});
```

- [ ] **Step 3: Run the new acceptance cases and verify they fail**

Run: `cd apps/desktop && npx playwright test e2e/acceptance.spec.ts --grep "flush 440px|flush full-workbench"`

Expected: FAIL on the old percentage data attribute, outer gaps, radius/shadow, and overlay mode.

- [ ] **Step 4: Convert `WorkPanelLayout` to pixel sizing and two viewport modes**

Use the dimension helpers as follows:

```ts
const workbenchWidth = getWorkbenchWidth(viewportWidth);
const mode = getWorkPanelViewportMode(workbenchWidth);
const bounds = getWorkPanelBounds(workbenchWidth);
const renderedWidthPx = clampWorkPanelWidthPx(state.widthPx, bounds);
const isSplit = mode === "split";

const setWidthPx = useCallback((widthPx: number) => {
  const target = clampWorkPanelWidthPx(widthPx, bounds);
  updateState((current) => ({ ...current, widthPx: target }), false);
  if (isSplit) panelRef.current?.resize(`${target}px`);
}, [bounds, isSplit, panelRef, updateState]);
```

Remove the fixed/overlay restore-frame state. Configure the panels with:

```tsx
<Panel id="conversation" minSize={mode === "full" || maximized ? "0%" : "400px"}>
  {children}
</Panel>
{open && isSplit && !maximized ? (
  <Separator
    id="work-panel-separator"
    className="forge-work-panel-separator"
    aria-label="调整工作面板宽度"
    onDoubleClick={() => setWidthPx(DEFAULT_WORK_PANEL_WIDTH_PX)}
  />
) : null}
<Panel
  id="work-panel"
  panelRef={panelRef}
  defaultSize={mode === "full" || maximized ? "100%" : `${renderedWidthPx}px`}
  minSize={mode === "full" || maximized ? "100%" : `${bounds.min}px`}
  maxSize={mode === "full" || maximized ? "100%" : `${bounds.max}px`}
>
  {/* WorkPanelShell */}
</Panel>
```

Convert `onLayoutChanged` percentages back to pixels with `workPanelPercentToPx`, clamp them, and persist only in split mode. Maximize must restore `state.widthPx`.

Remove the unused `onDecreaseWidth` and `onIncreaseWidth` props from `WorkPanelShellProps` and their call sites. Keyboard resizing remains owned by the accessible `Separator`, not hidden shell buttons.

- [ ] **Step 5: Expose the pixel state and remove sheet-entry translation**

In `WorkPanelShell`, replace `data-width-percent` with:

```tsx
data-width-px={state.widthPx}
```

Delete the first GSAP effect that animates the entire panel from `{ x: 12 }`. Keep one content effect, but animate opacity only:

```ts
gsap.fromTo(content, { autoAlpha: 0 }, {
  autoAlpha: 1,
  duration: 0.12,
  ease: forgeMotion.evidence.ease,
  clearProps: "opacity,visibility",
});
```

- [ ] **Step 6: Remove only structural sheet CSS**

Preserve all current theme variables and color declarations. Replace the outer geometry with:

```css
.forge-work-panel {
  container-type: inline-size;
  display: flex;
  height: 100%;
  min-width: 0;
  flex-direction: column;
  margin: 0;
  overflow: hidden;
  border: 0;
  border-radius: 0;
  background: var(--forge-work-panel-sheet);
  color: var(--forge-text-primary);
  box-shadow: none;
}

.forge-work-panel-separator {
  position: relative;
  z-index: 2;
  width: 1px;
  background: var(--forge-border-subtle);
  outline: none;
}

.forge-work-panel-separator::before {
  position: absolute;
  inset: 0 -5px;
  content: "";
}
```

Remove the old `@media` rule that fixes an overlay aside to the viewport. The `full` mode remains inside the workbench layout.

- [ ] **Step 7: Run the focused acceptance cases and build**

Run:

```bash
cd apps/desktop
npx playwright test e2e/acceptance.spec.ts --grep "flush 440px|flush full-workbench"
npm run build
```

Expected: both acceptance cases PASS and the TypeScript/Vite build succeeds.

- [ ] **Step 8: Review affected scope and commit**

Stage only the four files, inspect the staged CSS to prove theme tokens were not changed, run staged `detect_changes`, then commit:

```bash
git add apps/desktop/src/components/workpanel/WorkPanelLayout.tsx apps/desktop/src/components/workpanel/WorkPanelShell.tsx apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): embed the work panel as a native split"
```

### Task 4: Remove the launcher's default selection and cheap menu behavior

**Files:**
- Create: `apps/desktop/src/components/workpanel/WorkPanelActionList.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Run pre-change impact analysis**

Run GitNexus upstream impact for `WorkPanelLauncher` and `handleKeyDown` in `WorkPanelLauncher.tsx`. Include the shell, selectors, and acceptance processes.

- [ ] **Step 2: Write failing launcher-state and geometry assertions**

Update the launcher acceptance case:

```ts
const options = panel.getByRole("option");
await expect(options).toHaveCount(5);
for (const option of await options.all()) {
  await expect(option).toHaveAttribute("aria-selected", "false");
  await expect(option).toHaveAttribute("data-selected", "false");
}

const launcherMetrics = await panel.getByTestId("work-panel-launcher").evaluate((launcher) => {
  const panel = launcher.closest<HTMLElement>("aside")!;
  const list = launcher.querySelector<HTMLElement>("[data-testid='work-panel-action-list']")!;
  const actions = Array.from(list.querySelectorAll<HTMLElement>("[role='option']"));
  const panelRect = panel.getBoundingClientRect();
  const listRect = list.getBoundingClientRect();
  return {
    bottomGap: Math.round(panelRect.bottom - listRect.bottom),
    width: Math.round(listRect.width),
    heights: actions.map((action) => Math.round(action.getBoundingClientRect().height)),
    gap: Number.parseFloat(getComputedStyle(list).rowGap),
  };
});
expect(launcherMetrics).toEqual({ bottomGap: 48, width: 360, heights: [48, 48, 48, 48, 48], gap: 8 });

await panel.getByTestId("work-panel-action-list").focus();
await page.keyboard.press("ArrowDown");
await expect(panel.getByRole("option", { name: /^审阅/ })).toHaveAttribute("aria-selected", "true");
await page.keyboard.press("Enter");
await expect(panel.getByRole("tab", { name: /^审阅/ })).toBeVisible();
```

- [ ] **Step 3: Run the launcher case and verify it fails**

Run: `cd apps/desktop && npx playwright test e2e/acceptance.spec.ts --grep "launcher without project archive"`

Expected: FAIL because the first cmdk item is selected by default and the launcher is too wide/high in the wrong position.

- [ ] **Step 4: Create the focused root action list**

Create `WorkPanelActionList.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { FileDiff, FolderOpen, Globe2, ListTree, TerminalSquare, type LucideIcon } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { WORK_PANEL_LAUNCHER_ACTIONS } from "./workPanelSelectors";
import type { WorkPanelLauncherAction } from "./workPanelTypes";

const actionIcons: Record<WorkPanelLauncherAction, LucideIcon> = {
  review: FileDiff,
  terminal: TerminalSquare,
  preview: Globe2,
  files: FolderOpen,
  subtasks: ListTree,
};

export function WorkPanelActionList({ onAction }: { onAction: (action: WorkPanelLauncherAction) => void }) {
  const listRef = useRef<HTMLDivElement>(null);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);

  useEffect(() => {
    const frame = requestAnimationFrame(() => listRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const move = (direction: 1 | -1) => {
    setActiveIndex((current) => {
      if (current === null) return direction === 1 ? 0 : WORK_PANEL_LAUNCHER_ACTIONS.length - 1;
      return (current + direction + WORK_PANEL_LAUNCHER_ACTIONS.length) % WORK_PANEL_LAUNCHER_ACTIONS.length;
    });
  };

  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="选择工作内容"
      aria-activedescendant={activeIndex === null ? undefined : `work-panel-action-${WORK_PANEL_LAUNCHER_ACTIONS[activeIndex].id}`}
      tabIndex={0}
      className="forge-work-panel-action-list"
      data-testid="work-panel-action-list"
      onKeyDown={(event) => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          move(event.key === "ArrowDown" ? 1 : -1);
        } else if (event.key === "Home") {
          event.preventDefault();
          setActiveIndex(0);
        } else if (event.key === "End") {
          event.preventDefault();
          setActiveIndex(WORK_PANEL_LAUNCHER_ACTIONS.length - 1);
        } else if (event.key === "Enter" && activeIndex !== null) {
          event.preventDefault();
          onAction(WORK_PANEL_LAUNCHER_ACTIONS[activeIndex].id);
        }
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setActiveIndex(null);
      }}
    >
      {WORK_PANEL_LAUNCHER_ACTIONS.map((action, index) => {
        const Icon = actionIcons[action.id];
        const selected = activeIndex === index;
        return (
          <ButtonPrimitive
            key={action.id}
            id={`work-panel-action-${action.id}`}
            type="button"
            role="option"
            aria-selected={selected}
            data-selected={selected}
            className="forge-work-panel-launcher-action"
            onClick={() => onAction(action.id)}
          >
            <Icon className="size-5" />
            <span>{action.label}</span>
            {action.shortcut ? <kbd>{action.shortcut}</kbd> : null}
          </ButtonPrimitive>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 5: Let `WorkPanelLauncher` own only actions and chooser state**

Delete `selectedActionId`, `selectedAction`, the root cmdk ref, root keyboard selection logic, root focus effect, and `actionIcons`. Import `WorkPanelActionList` and render:

```tsx
return (
  <div className="forge-work-panel-launcher" data-testid="work-panel-launcher" data-mode={mode}>
    <WorkPanelActionList onAction={handleAction} />
  </div>
);
```

Keep `Command`, `CommandInput`, `CommandList`, and `CommandItem` only inside the preview/file/subtask picker. Its `Escape` handler calls `returnToRoot`.

- [ ] **Step 6: Apply the approved launcher geometry without changing theme colors**

```css
.forge-work-panel-launcher {
  display: flex;
  min-height: 0;
  flex: 1;
  align-items: flex-end;
  justify-content: center;
  overflow: auto;
  padding: 0 24px 48px;
}

.forge-work-panel-action-list {
  display: grid;
  width: min(360px, 100%);
  gap: 8px;
  outline: none;
}

.forge-work-panel-launcher-action {
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto;
  width: 100%;
  height: 48px;
  min-height: 48px;
  align-items: center;
  gap: 12px;
  border: 0;
  padding: 0 16px;
  text-align: left;
  transition: background-color 120ms ease, color 120ms ease;
}
```

Keep the current token-backed background, text, hover, and focus colors. Do not alter `tokens.css`.

- [ ] **Step 7: Run launcher, picker, tab, and keyboard acceptance**

Run:

```bash
cd apps/desktop
npx playwright test e2e/acceptance.spec.ts --grep "launcher without project archive|preview and file adapters|restore keeps tabs"
```

Expected: PASS, including no initial selected option and immediate chooser replacement.

- [ ] **Step 8: Review affected scope and commit**

Stage the new action list plus the launcher, CSS, and focused acceptance hunks. Run staged `detect_changes`, then commit:

```bash
git add apps/desktop/src/components/workpanel/WorkPanelActionList.tsx apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): make the work panel launcher intentional"
```

### Task 5: Fuse object controls and guarantee direct content fill

**Files:**
- Modify: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Run pre-change impact analysis**

Run GitNexus upstream impact for `WorkPanelShell` and `WorkPanelObjectBar`. Report tab, close, maximize, add-object, and adapter consumers.

- [ ] **Step 2: Add failing object-bar and content-fill assertions**

After opening `http://localhost:1420`, assert:

```ts
const objectBar = panel.locator(".forge-work-panel-object-bar");
const tabContent = panel.locator(".forge-work-panel-tab-content:not([hidden])");
const viewport = panel.locator(".forge-work-panel-preview-viewport");
await expect(objectBar).toBeVisible();
await expect(panel.getByRole("button", { name: "新建工作面板标签" })).toBeVisible();
await expect(viewport).toHaveCSS("border-radius", "0px");
await expect(viewport).toHaveCSS("box-shadow", "none");

const fill = await panel.evaluate((element) => {
  const content = element.querySelector<HTMLElement>(".forge-work-panel-tab-content:not([hidden])")!;
  const viewport = element.querySelector<HTMLElement>(".forge-work-panel-preview-viewport")!;
  return {
    panelBottom: Math.round(element.getBoundingClientRect().bottom),
    contentBottom: Math.round(content.getBoundingClientRect().bottom),
    viewportBottom: Math.round(viewport.getBoundingClientRect().bottom),
  };
});
expect(fill.contentBottom).toBe(fill.panelBottom);
expect(fill.viewportBottom).toBe(fill.panelBottom);
```

Click `+` and assert the active content is replaced by the five-action launcher, with no persistent `打开新的…` heading:

```ts
await panel.getByRole("button", { name: "新建工作面板标签" }).click();
await expect(panel.getByTestId("work-panel-launcher")).toBeVisible();
await expect(panel.getByText("打开新的…", { exact: true })).toHaveCount(0);
await expect(viewport).toHaveCount(0);
```

- [ ] **Step 3: Run the focused test and verify the old chrome assertions fail**

Run: `cd apps/desktop && npx playwright test e2e/acceptance.spec.ts --grep "preview and file adapters"`

Expected: FAIL until obsolete radius/shadow expectations and transient-label behavior are removed.

- [ ] **Step 4: Keep a single top control band**

Preserve the existing Base UI Tabs and dropdown behavior. In `WorkPanelObjectBar`, keep this order:

```tsx
<TabsList>{/* concrete object tabs */}</TabsList>
<DropdownMenu>{/* overflow */}</DropdownMenu>
<ForgeIconButton aria-label="新建工作面板标签">{/* Plus */}</ForgeIconButton>
<ForgeIconButton aria-label={maximized ? "恢复工作面板宽度" : "最大化工作面板"}>{/* expand */}</ForgeIconButton>
<ForgeIconButton aria-label="关闭工作面板">{/* close */}</ForgeIconButton>
```

Do not add a separate panel title, category label, secondary global toolbar, or floating controls. Ensure `.forge-work-panel-tabs` and `.forge-work-panel-tab-content` both have `min-height: 0` and the active tab content uses `flex: 1`.

- [ ] **Step 5: Keep empty-state controls aligned to the same system**

The empty launcher retains only the expand and close icon buttons in `.forge-work-panel-launcher-utilities`. Match its height and right padding to the object bar; do not absolutely position either button.

- [ ] **Step 6: Run object, deduplication, restore, and unavailable-state acceptance**

Run:

```bash
cd apps/desktop
npx playwright test e2e/acceptance.spec.ts --grep "preview and file adapters|restore keeps tabs|unavailable file|terminal is temporary"
```

Expected: PASS.

- [ ] **Step 7: Review affected scope and commit**

Stage only the four files, run staged `detect_changes`, then commit:

```bash
git add apps/desktop/src/components/workpanel/WorkPanelShell.tsx apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts
git commit -m "fix(desktop): unify work panel object chrome"
```

### Task 6: Align responsive, accessibility, and structural guardrails

**Files:**
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `apps/desktop/e2e/chrome.spec.ts`
- Modify: `apps/desktop/e2e/messages.spec.ts`
- Modify: `apps/desktop/e2e/guardrails.spec.ts`
- Modify: `apps/desktop/src/styles/work-panel.css`

- [ ] **Step 1: Replace obsolete narrow and header assumptions**

In `chrome.spec.ts`, change the Work Panel test to assert that a `900×720` viewport uses `data-viewport-mode="full"`, the panel is flush with the main workbench, all action rows are exactly `48px`, and the action list width is no greater than `360px`.

In `messages.spec.ts`, replace the obsolete `.forge-work-panel-header`, `.forge-work-panel-title`, and `.forge-work-panel-task` queries with:

```ts
const utilities = panel?.querySelector<HTMLElement>(".forge-work-panel-launcher-utilities");
const actionList = panel?.querySelector<HTMLElement>(".forge-work-panel-action-list");
if (!panel || !utilities || !actionList || actions.length !== 5 || !composer) return null;
```

Assert `48px` action height, `8px` gap, `360px` maximum list width, and no overlap between the conversation and split panel.

In `guardrails.spec.ts`, read `WorkPanelActionList.tsx` and assert that Base UI owns the clickable options while cmdk remains in `WorkPanelLauncher.tsx` only for chooser search.

- [ ] **Step 2: Add keyboard and reduced-motion coverage**

Add acceptance assertions that:

```ts
await actionList.focus();
await page.keyboard.press("End");
await expect(panel.getByRole("option", { name: /^侧边任务/ })).toHaveAttribute("aria-selected", "true");
await page.keyboard.press("Escape");
// Escape at the root does not close the panel.
await expect(panel).toBeVisible();
```

For the preview chooser, `Escape` must return to the root launcher. Under reduced motion, panel and content transforms must compute to `none`.

Verify keyboard resizing and task-scoped restoration:

```ts
const separator = page.getByRole("separator", { name: "调整工作面板宽度" });
await separator.focus();
await page.keyboard.press("ArrowLeft");
await expect.poll(async () => Number(await panel.getAttribute("data-width-px"))).toBeLessThan(440);
const resizedWidth = Number(await panel.getAttribute("data-width-px"));
await panel.getByRole("button", { name: "关闭工作面板" }).click();
await page.getByRole("button", { name: "打开工作面板" }).click();
await expect(panel).toHaveAttribute("data-width-px", String(resizedWidth));
```

- [ ] **Step 3: Run the four focused suites**

Run:

```bash
cd apps/desktop
npx playwright test e2e/acceptance.spec.ts e2e/chrome.spec.ts e2e/messages.spec.ts e2e/guardrails.spec.ts --grep "work panel"
```

Expected: all Work Panel cases PASS.

- [ ] **Step 4: Review affected scope and commit**

Stage the five files, run staged `detect_changes`, then commit:

```bash
git add apps/desktop/e2e/acceptance.spec.ts apps/desktop/e2e/chrome.spec.ts apps/desktop/e2e/messages.spec.ts apps/desktop/e2e/guardrails.spec.ts apps/desktop/src/styles/work-panel.css
git commit -m "test(desktop): lock embedded work panel behavior"
```

### Task 7: Update product documentation and complete visual verification

**Files:**
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Replace the old product wording**

Use this factual description in the README surfaces, adapted only for surrounding grammar:

```md
工作面板是贴合工作区的原生分栏，默认宽度 440px，可拖动并按任务恢复；空间不足时无边框地占据工作区。首次打开显示无标题的紧凑五行入口，用户明确选择审阅、临时终端、预览、文件或侧边任务后，具体对象在去重 Tab 中直接接管面板。主题与颜色沿用当前工作台设计；记忆和 continuity 仍是隐藏的底层能力。
```

Remove claims that the default is `40%`, resizing is `34–62%`, or narrow windows use an overlay.

- [ ] **Step 2: Add the changelog entry**

Replace the old Work Panel bullet with:

```md
- Reworked the desktop Work Panel as a flush native split instead of a framed sheet. It opens at 440px, resizes and restores per task, falls back to a borderless full-workbench surface when narrow, starts with an unselected compact five-action launcher, and lets selected preview, file, review, temporary terminal, or subtask content directly occupy the panel. Existing theme treatment remains unchanged, and memory/continuity stay hidden implementation context.
```

- [ ] **Step 3: Run document and repository dry-run checks**

Run:

```bash
rg -n "40%|34.?62%|overlay" README.md apps/desktop/README.md CHANGELOG.md
scripts/acceptance.sh --dry-run
```

Expected: the first command returns no stale Work Panel width/overlay claims; the dry-run exits successfully and still advertises `e2e/acceptance.spec.ts` through the desktop smoke gate.

- [ ] **Step 4: Run the complete implementation verification**

Run:

```bash
cd apps/desktop
node --test src/components/workpanel/workPanelDimensions.test.ts src/components/workpanel/workPanelState.test.ts src/components/workpanel/workPanelSelectors.test.ts
npm run build
npx playwright test e2e/acceptance.spec.ts e2e/chrome.spec.ts e2e/messages.spec.ts e2e/guardrails.spec.ts --grep "work panel"
cd ../..
scripts/acceptance.sh --dry-run
```

Expected: all focused unit tests pass, the production build succeeds, all Work Panel Playwright cases pass, and the acceptance dry-run succeeds.

- [ ] **Step 5: Perform same-state visual comparison**

Start the desktop frontend, open a normal task at `1440×900`, and capture:

1. first-open empty launcher;
2. preview chooser;
3. `http://localhost:1420` preview;
4. read-only file tab;
5. `900×720` full-workbench fallback.

Inspect each capture and reject it if it is loading, cropped, or in the wrong state. Compare the empty state to the approved Codex reference and the user's rejected framed screenshot. Confirm one divider, zero outer margin/radius/shadow, `440px` default width, `360px` action list, `48px` rows, `8px` gaps, `48px` bottom offset, no default row highlight, and direct content fill. Do not change theme colors during this pass.

- [ ] **Step 6: Run final GitNexus regression review and commit docs**

Stage only the three documentation files, run `detect_changes({scope: "compare", base_ref: "main", repo: "forge"})`, review every affected process, then commit:

```bash
git add README.md apps/desktop/README.md CHANGELOG.md
git commit -m "docs(desktop): describe the embedded work panel split"
```

- [ ] **Step 7: Final handoff evidence**

Report:

- commits created by Tasks 1–7;
- focused unit, build, E2E, and dry-run results;
- GitNexus final risk and affected processes;
- paths to the accepted visual captures;
- confirmation that current theme tokens and unrelated dirty files were not changed by this plan.
