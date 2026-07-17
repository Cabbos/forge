# Forge Work Panel Quiet Native Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current launcher-first Work Panel into the approved Quiet Native adaptive sheet, with real light/dark theme consumption, titleless empty state, fused object bar, task-scoped sizing, and a temporary lightweight terminal.

**Architecture:** Keep the existing Work Panel state machine and content adapters, but separate object chrome into a focused `WorkPanelObjectBar` component and move width normalization into a pure dimensions module. Reuse the existing store `theme` value rather than changing the CRITICAL-risk preferences subsystem; wire that value into `AppShell` and `SessionView`, then express the redesign through shared Work Panel tokens and one outer surface.

**Tech Stack:** React 18, TypeScript, Base UI Tabs/Menu/Button, cmdk, react-resizable-panels, GSAP, Playwright, Node test runner, Tauri 2.

---

## Scope And Risk Notes

- Baseline after main sync: merge commit `282ceed3` on `cabbos/work-panel`.
- The current uncommitted terminal compatibility fix must be committed separately before visual work.
- GitNexus reports `AppShell` and `SessionView` as LOW upstream risk.
- GitNexus reports `createPreferencesActions` as CRITICAL: 49 impacted symbols across 19 processes. Do not change the theme store shape or persistence in this plan.
- The feature-branch-only symbols `WorkPanelLayout`, `WorkPanelShell`, and `WorkPanelLauncher` are absent from the current index. Before editing them, record the required fallback impact report: the failed MCP lookup, current index freshness, direct import chain (`AppShell → WorkPanelLayout → WorkPanelShell → WorkPanelLauncher/WorkPanelContent`), tests selected below, affected desktop UI authority, and residual risk.
- Do not use `frontend-skill`. Preserve the approved `design-taste-frontend` dials: variance 5, motion 3, density 5.
- Unless a step explicitly says "from the monorepo root", run `npm`, `node --test`, and Playwright commands from `apps/desktop`.

## File Map

### Create

- `apps/desktop/src/components/workpanel/workPanelDimensions.ts` — width defaults, clamping, task restore normalization, and responsive mode calculation.
- `apps/desktop/src/components/workpanel/workPanelDimensions.test.ts` — pure boundary tests for 40%, 34%–62%, 360px/920px, and overlay behavior.
- `apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx` — object tabs, overflow menu, `+`, maximize/restore, and close controls.

### Modify

- `apps/desktop/src/components/layout/AppShell.tsx` — expose the existing resolved theme on the app root.
- `apps/desktop/src/components/session/SessionView.tsx` — expose the same theme to conversation-scoped light/dark rules.
- `apps/desktop/src/styles/globals.css` — change hard-coded light-workbench selectors to real theme selectors.
- `apps/desktop/scripts/check-conversation-style.mjs` — assert dynamic theme markers instead of hard-coded light attributes.
- `apps/desktop/e2e/messages.spec.ts` and `apps/desktop/e2e/process.spec.ts` — stop mutating obsolete design-version attributes.
- `apps/desktop/src/components/workpanel/workPanelTypes.ts` — persist width with task state.
- `apps/desktop/src/components/workpanel/workPanelState.ts` — preserve width through open/focus/close transitions.
- `apps/desktop/src/components/workpanel/workPanelPersistence.ts` — migrate storage v1 to v2 without losing tabs.
- `apps/desktop/src/components/workpanel/workPanelState.test.ts` — cover width migration and final-tab preservation.
- `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx` — use task width, responsive modes, reset, and keyboard resize alternatives.
- `apps/desktop/src/components/workpanel/WorkPanelShell.tsx` — render titleless utility controls or the object bar according to state.
- `apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx` — remove empty-state search, add explicit new-object mode, retain focused pickers.
- `apps/desktop/src/components/workpanel/workPanelSelectors.ts` and `.test.ts` — use the approved launcher labels.
- `apps/desktop/src/components/workpanel/WorkPanelPreview.tsx`, `WorkPanelFiles.tsx`, `WorkPanelReview.tsx`, `WorkPanelSubtask.tsx`, and `WorkPanelTerminal.tsx` — keep adapter chrome shallow and content-first.
- `apps/desktop/src/styles/tokens.css` and `apps/desktop/src/styles/work-panel.css` — Quiet Native light/dark tokens, one-sheet layout, context bars, responsive overlay, and reduced motion.
- `apps/desktop/e2e/acceptance.spec.ts` — theme, launcher, object bar, persistence, width, responsive, and lightweight terminal acceptance.
- `apps/desktop/scripts/desktop-security-config.test.mjs` — retain the frozen-prototype terminal guard.
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`, and `scripts/acceptance.sh` — describe the shipped behavior accurately and remove xterm references.

## Task 1: Commit The Lightweight Terminal Compatibility Baseline

**Files:**
- Modify: `apps/desktop/e2e/acceptance.spec.ts:223-250`
- Modify: `README.md:83`
- Modify: `apps/desktop/README.md:65`
- Existing modified: `apps/desktop/package.json`
- Existing modified: `apps/desktop/package-lock.json`
- Existing modified: `apps/desktop/scripts/desktop-security-config.test.mjs`
- Existing modified: `apps/desktop/src/components/workpanel/WorkPanelTerminal.tsx`
- Existing modified: `apps/desktop/src/styles/work-panel.css`

- [ ] **Step 1: Record the pre-edit impact report for `WorkPanelTerminal`**

Run GitNexus upstream impact for `WorkPanelTerminal`. Because the branch symbol is currently unindexed, save this fallback in the task notes:

```text
Command: impact({ target: "WorkPanelTerminal", direction: "upstream" })
Result: target not found in current index
Direct caller: WorkPanelContent
Transitive UI chain: WorkPanelContent → WorkPanelShell → WorkPanelLayout → AppShell
Authority domains: task-scoped PTY IPC only; no permission expansion
Selected tests: desktop-security-config.test.mjs, work panel terminal acceptance, desktop build
Residual risk: medium until acceptance proves start/write/close lifecycle
```

- [ ] **Step 2: Update the terminal acceptance test for the lightweight command view**

Replace the xterm-specific interaction with the accessible input and recent-output view:

```ts
const terminal = panel.getByTestId("work-panel-terminal");
await expect(terminal).toBeVisible();
await expect(terminal).toContainText("临时验证终端");

const command = terminal.getByRole("textbox", { name: "临时验证命令" });
await command.fill("printf 'verification passed'");
await terminal.getByRole("button", { name: "运行验证命令" }).click();
await expect(terminal.getByRole("log")).toContainText("verification passed");
```

Add `role="log"` to the existing terminal `<pre>`:

```tsx
<pre aria-live="polite" role="log">
  {recentOutput || "临时环境已连接。输入一条验证命令即可；这里只保留最近输出。"}
</pre>
```

- [ ] **Step 3: Remove stale xterm claims from documentation**

Use these exact dependency descriptions:

```markdown
基于 Base UI Tabs、cmdk、react-resizable-panels 与成熟 diff 组件组织审阅、临时命令验证、显式预览、文件和聚焦子任务。
```

Do not mention `xterm.js`, `@xterm/xterm`, or `@xterm/addon-fit` in `README.md` or `apps/desktop/README.md`.

- [ ] **Step 4: Run the terminal-focused red/green gates**

Run:

```bash
node --test scripts/desktop-security-config.test.mjs
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel terminal"
npm run build
```

Expected: 4 security tests pass; the terminal acceptance passes without `.xterm-*` selectors; the production build succeeds.

- [ ] **Step 5: Detect changes and commit only the terminal baseline**

Run `detect_changes({ scope: "unstaged", worktree: "/Users/cabbos/.config/superpowers/worktrees/forge/work-panel" })`, review the Work Panel terminal path, then commit only these files:

```bash
git add apps/desktop/package.json apps/desktop/package-lock.json \
  apps/desktop/scripts/desktop-security-config.test.mjs \
  apps/desktop/src/components/workpanel/WorkPanelTerminal.tsx \
  apps/desktop/src/styles/work-panel.css \
  apps/desktop/e2e/acceptance.spec.ts README.md apps/desktop/README.md
git commit -m "fix(desktop): keep work panel terminal compatible with frozen prototypes"
```

Leave `.claude/skills/`, root `AGENTS.md`, root `CLAUDE.md`, `docs/forge-sync/feishu-upgrade-log.md`, `.playwright-cli/`, and `output/` unstaged.

## Task 2: Wire The Existing Theme State Into The App Surface

**Files:**
- Modify: `apps/desktop/src/components/layout/AppShell.tsx:18-68`
- Modify: `apps/desktop/src/components/session/SessionView.tsx:1-16`
- Modify: `apps/desktop/src/styles/globals.css:313-370`
- Modify: `apps/desktop/scripts/check-conversation-style.mjs:60-95`
- Modify: `apps/desktop/e2e/messages.spec.ts:1-24`
- Modify: `apps/desktop/e2e/process.spec.ts:1-24`
- Test: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Confirm low-risk theme consumers and avoid the CRITICAL store path**

Run upstream impact for `AppShell` and `SessionView`; expected risk is LOW. Do not modify `createPreferencesActions`, `AppStore.theme`, hydration, or the persisted `tui-theme` key.

- [ ] **Step 2: Add a failing system-theme acceptance test**

Add this case near the Work Panel acceptance group:

```ts
test("app and conversation consume the selected light and dark themes", async ({ page }) => {
  const shell = page.getByTestId("operating-surface");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  const conversation = page.locator(".forge-session-operating-surface");

  await expect(shell).toHaveAttribute("data-theme", /light|dark/);
  await page.keyboard.press("Meta+k");
  await page.getByRole("option", { name: /切换主题/ }).click();
  const nextTheme = await shell.getAttribute("data-theme");
  await expect(conversation).toHaveAttribute("data-conversation-theme", nextTheme ?? "light");
});
```

- [ ] **Step 3: Run the test to verify the hard-coded light shell fails**

Run:

```bash
npm run test:e2e -- e2e/acceptance.spec.ts --grep "consume the selected light and dark themes"
```

Expected: FAIL because `AppShell` has `data-design-version="v3-light-workbench"` and no `data-theme`.

- [ ] **Step 4: Bind `AppShell` and `SessionView` to the existing theme value**

In `AppShell`:

```tsx
const theme = useStore((state) => state.theme);

return (
  <div
    data-testid="operating-surface"
    data-design-version="v4-quiet-native"
    data-theme={theme}
    className="forge-app-shell h-screen grid bg-background"
  >
```

In `SessionView`:

```tsx
import { useStore } from "@/store";

export function SessionView({ sessionId }: SessionViewProps) {
  const theme = useStore((state) => state.theme);
  return (
    <div
      data-conversation-theme={theme}
      className="forge-session-operating-surface flex-1 min-h-0 flex flex-col bg-background"
    >
      <ChatView />
      <InputBar sessionId={sessionId} />
    </div>
  );
}
```

- [ ] **Step 5: Replace the obsolete global light selector**

Use the actual theme attribute while keeping the existing conversation selector:

```css
body:has(.forge-app-shell[data-theme="light"]),
.forge-app-shell[data-theme="light"],
.forge-session-operating-surface[data-conversation-theme="light"] {
  /* retain the complete existing light token block */
}
```

Update `check-conversation-style.mjs` to require `data-theme={theme}` and `data-conversation-theme={theme}`, and update message/process E2E helpers to toggle `data-theme` instead of `data-design-version`.

- [ ] **Step 6: Run theme gates and commit**

Run:

```bash
npm run check:conversation-style
npm run test:e2e -- e2e/acceptance.spec.ts --grep "consume the selected light and dark themes"
npm run test:e2e -- e2e/messages.spec.ts e2e/process.spec.ts
npm run build
```

Expected: all commands pass in light and dark theme paths.

Run `detect_changes({ scope: "unstaged" })`, then:

```bash
git add apps/desktop/src/components/layout/AppShell.tsx \
  apps/desktop/src/components/session/SessionView.tsx \
  apps/desktop/src/styles/globals.css \
  apps/desktop/scripts/check-conversation-style.mjs \
  apps/desktop/e2e/messages.spec.ts apps/desktop/e2e/process.spec.ts \
  apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): apply selected theme across the workbench"
```

## Task 3: Persist Task-Scoped Width And Responsive Mode

**Files:**
- Create: `apps/desktop/src/components/workpanel/workPanelDimensions.ts`
- Create: `apps/desktop/src/components/workpanel/workPanelDimensions.test.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelTypes.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelState.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelPersistence.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelState.test.ts`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`

- [ ] **Step 1: Record the fallback impact chain for Work Panel state and layout**

Record the unindexed-symbol fallback with direct imports:

```text
workPanelDimensions → WorkPanelLayout/workPanelState/workPanelPersistence
workPanelState → WorkPanelLayout and node tests
WorkPanelLayout → AppShell
Authority change: local presentation persistence only
Residual risk: panel restoration and resize behavior
```

- [ ] **Step 2: Write failing pure dimension and persistence tests**

Create `workPanelDimensions.test.ts` with exact boundaries:

```ts
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  DEFAULT_WORK_PANEL_WIDTH,
  normalizeWorkPanelWidth,
  workPanelBoundsForWidth,
  workPanelModeForViewport,
} from "./workPanelDimensions.ts";

describe("work panel dimensions", () => {
  it("defaults to 40 percent and clamps split widths", () => {
    assert.equal(DEFAULT_WORK_PANEL_WIDTH, 40);
    assert.equal(normalizeWorkPanelWidth(undefined), 40);
    assert.equal(normalizeWorkPanelWidth(12), 34);
    assert.equal(normalizeWorkPanelWidth(90), 62);
  });

  it("uses fixed and overlay fallbacks for narrow windows", () => {
    assert.equal(workPanelModeForViewport(1100), "split");
    assert.equal(workPanelModeForViewport(899), "fixed");
    assert.equal(workPanelModeForViewport(719), "overlay");
  });

  it("enforces the 360px minimum and 920px maximum inside percentage bounds", () => {
    assert.deepEqual(workPanelBoundsForWidth(1000), { minPercent: 36, maxPercent: 62 });
    assert.deepEqual(workPanelBoundsForWidth(2000), { minPercent: 34, maxPercent: 46 });
  });
});
```

Extend persistence tests so storage version 1 migrates to `widthPercent: 40`, and closing the final tab preserves a custom `widthPercent: 52`.

- [ ] **Step 3: Run tests to verify missing dimensions and width state fail**

Run:

```bash
node --test src/components/workpanel/workPanelDimensions.test.ts src/components/workpanel/workPanelState.test.ts
```

Expected: FAIL because the dimensions module and `widthPercent` do not exist.

- [ ] **Step 4: Implement the pure dimensions contract**

Create:

```ts
export const DEFAULT_WORK_PANEL_WIDTH = 40;
export const MIN_WORK_PANEL_WIDTH = 34;
export const MAX_WORK_PANEL_WIDTH = 62;
export const MIN_WORK_PANEL_PIXELS = 360;
export const MAX_WORK_PANEL_PIXELS = 920;

export type WorkPanelViewportMode = "split" | "fixed" | "overlay";

export function normalizeWorkPanelWidth(value: unknown): number {
  const width = typeof value === "number" ? value : Number.NaN;
  if (!Number.isFinite(width)) return DEFAULT_WORK_PANEL_WIDTH;
  return Math.min(MAX_WORK_PANEL_WIDTH, Math.max(MIN_WORK_PANEL_WIDTH, width));
}

export function workPanelModeForViewport(viewportWidth: number): WorkPanelViewportMode {
  if (viewportWidth < 720) return "overlay";
  if (viewportWidth < 900) return "fixed";
  return "split";
}

export function workPanelBoundsForWidth(workbenchWidth: number) {
  const safeWidth = Math.max(1, workbenchWidth);
  const minPercent = Math.max(
    MIN_WORK_PANEL_WIDTH,
    Math.min(MAX_WORK_PANEL_WIDTH, (MIN_WORK_PANEL_PIXELS / safeWidth) * 100),
  );
  const maxPercent = Math.max(
    minPercent,
    Math.min(MAX_WORK_PANEL_WIDTH, (MAX_WORK_PANEL_PIXELS / safeWidth) * 100),
  );
  return {
    minPercent: Math.round(minPercent * 100) / 100,
    maxPercent: Math.round(maxPercent * 100) / 100,
  };
}
```

- [ ] **Step 5: Migrate task state to storage version 2**

Add `widthPercent: number` to `WorkPanelTaskState`. Make `restoreTaskPanelState` normalize it. When closing the final tab, return an empty launcher while retaining width:

```ts
if (tabs.length === 0) {
  return {
    tabs: [],
    activeTabId: null,
    launcherOpen: true,
    widthPercent: normalizeWorkPanelWidth(state.widthPercent),
  };
}
```

Write storage as version 2 and accept both v1 and v2 on read:

```ts
export interface WorkPanelStorage {
  version: 2;
  tasks: Record<string, WorkPanelTaskState>;
}

const widthPercent = parsed.version === 2
  ? normalizeWorkPanelWidth(value.widthPercent)
  : DEFAULT_WORK_PANEL_WIDTH;
```

- [ ] **Step 6: Update layout sizing and controls**

Use `state.widthPercent` as the default. Save resize changes through `updateState`, reset to 40% on separator double-click, and expose keyboard resize buttons with labels `缩小工作面板` and `扩大工作面板`.

Track the viewport explicitly inside `WorkPanelLayout`:

```tsx
const [viewportWidth, setViewportWidth] = useState(() =>
  typeof window === "undefined" ? 1200 : window.innerWidth,
);

useEffect(() => {
  const updateViewportWidth = () => setViewportWidth(window.innerWidth);
  window.addEventListener("resize", updateViewportWidth);
  return () => window.removeEventListener("resize", updateViewportWidth);
}, []);

const viewportMode = workPanelModeForViewport(viewportWidth);
const workbenchWidth = Math.max(360, viewportWidth - 284);
const bounds = workPanelBoundsForWidth(workbenchWidth);
```

Apply the computed bounds:

```tsx
<Panel
  id="work-panel"
  defaultSize={`${state.widthPercent}%`}
  minSize={viewportMode === "fixed" ? "360px" : `${bounds.minPercent}%`}
  maxSize={viewportMode === "fixed" ? "360px" : `${bounds.maxPercent}%`}
  onResize={(size) => {
    if (!maximized && viewportMode === "split") {
      updateState((current) => ({
        ...current,
        widthPercent: normalizeWorkPanelWidth(size.asPercentage),
      }));
    }
  }}
>
```

For `overlay`, render the panel with `data-viewport-mode="overlay"` and do not render the conversation separator. Pass `viewportMode` and `state.widthPercent` into `WorkPanelShell`, and expose them on the `<aside>` as `data-viewport-mode` and `data-width-percent` so acceptance can verify the applied mode.

- [ ] **Step 7: Run state tests, build, and commit**

Run:

```bash
node --test src/components/workpanel/workPanelDimensions.test.ts src/components/workpanel/workPanelState.test.ts
npm run build
```

Expected: all pure tests and the build pass.

Run `detect_changes({ scope: "unstaged" })`, then:

```bash
git add apps/desktop/src/components/workpanel/workPanelDimensions.ts \
  apps/desktop/src/components/workpanel/workPanelDimensions.test.ts \
  apps/desktop/src/components/workpanel/workPanelTypes.ts \
  apps/desktop/src/components/workpanel/workPanelState.ts \
  apps/desktop/src/components/workpanel/workPanelPersistence.ts \
  apps/desktop/src/components/workpanel/workPanelState.test.ts \
  apps/desktop/src/components/workpanel/WorkPanelLayout.tsx
git commit -m "feat(desktop): persist adaptive work panel dimensions"
```

## Task 4: Build The Titleless Shell And Fused Object Bar

**Files:**
- Create: `apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Test: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Add failing shell acceptance assertions**

In the first-open test assert:

```ts
await expect(panel.getByText("工作面板", { exact: true })).toHaveCount(0);
await expect(panel.getByRole("tablist")).toHaveCount(0);
await expect(panel.getByRole("button", { name: "关闭工作面板" })).toBeVisible();
```

After opening a preview assert:

```ts
await expect(panel.getByRole("tablist", { name: "已打开的工作内容" })).toBeVisible();
await expect(panel.getByRole("button", { name: "更多已打开内容" })).toBeVisible();
await expect(panel.getByRole("button", { name: "新建工作面板标签" })).toBeVisible();
```

- [ ] **Step 2: Run the focused tests to verify the fixed header fails**

Run:

```bash
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens on a launcher|preview and file adapters"
```

Expected: FAIL because the fixed `工作面板` header and empty tab rail are visible.

- [ ] **Step 3: Create `WorkPanelObjectBar`**

The component contract must remain presentation-only:

```tsx
interface WorkPanelObjectBarProps {
  activeTabId: string | null;
  maximized: boolean;
  tabs: WorkPanelTab[];
  onClose: () => void;
  onCloseTab: (tabId: string) => void;
  onFocusTab: (tabId: string) => void;
  onOpenLauncher: () => void;
  onToggleMaximize: () => void;
}
```

Render Base UI tabs, an always-available overflow menu, `+`, maximize/restore, and close. Overflow items must focus an existing tab:

```tsx
<DropdownMenu>
  <DropdownMenuTrigger asChild>
    <ForgeIconButton aria-label="更多已打开内容" title="更多已打开内容">
      <ChevronDown className="size-3.5" />
    </ForgeIconButton>
  </DropdownMenuTrigger>
  <DropdownMenuContent align="end" className="forge-work-panel-overflow-menu">
    {tabs.map((tab) => (
      <DropdownMenuItem key={tab.id} onClick={() => onFocusTab(tab.id)}>
        <span>{tab.label}</span>
        {tab.id === activeTabId ? <Check className="ml-auto size-3.5" /> : null}
      </DropdownMenuItem>
    ))}
  </DropdownMenuContent>
</DropdownMenu>
```

- [ ] **Step 4: Make shell chrome state-dependent**

When `state.launcherOpen` is true, render only top-right utility controls over the launcher. When an object is active, render `WorkPanelObjectBar`. Remove the fixed title, task label, and fixed header entirely:

```tsx
{state.launcherOpen ? (
  <div className="forge-work-panel-launcher-controls">
    <ForgeIconButton
      aria-label={maximized ? "恢复工作面板宽度" : "最大化工作面板"}
      title={maximized ? "恢复工作面板宽度" : "最大化工作面板"}
      onClick={onToggleMaximize}
    >
      {maximized ? <Minimize2 className="size-4" /> : <Maximize2 className="size-4" />}
    </ForgeIconButton>
    <ForgeIconButton aria-label="关闭工作面板" title="关闭工作面板" onClick={onClose}>
      <X className="size-4" />
    </ForgeIconButton>
  </div>
) : (
  <WorkPanelObjectBar
    activeTabId={state.activeTabId}
    maximized={maximized}
    tabs={state.tabs}
    onClose={onClose}
    onCloseTab={onCloseTab}
    onFocusTab={onFocusTab}
    onOpenLauncher={onOpenLauncher}
    onToggleMaximize={onToggleMaximize}
  />
)}
```

- [ ] **Step 5: Run shell acceptance, accessibility, and build gates**

Run:

```bash
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens on a launcher|preview and file adapters|restore keeps tabs"
npm run build
```

Expected: titleless launcher, visible object bar after selection, and keyboard tab navigation all pass.

- [ ] **Step 6: Detect and commit the shell boundary**

Run `detect_changes({ scope: "unstaged" })`, then:

```bash
git add apps/desktop/src/components/workpanel/WorkPanelObjectBar.tsx \
  apps/desktop/src/components/workpanel/WorkPanelShell.tsx \
  apps/desktop/src/styles/work-panel.css \
  apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add quiet native work panel object chrome"
```

## Task 5: Redesign The Empty Launcher And New-Object Chooser

**Files:**
- Modify: `apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx`
- Modify: `apps/desktop/src/components/workpanel/workPanelSelectors.ts`
- Modify: `apps/desktop/src/components/workpanel/workPanelSelectors.test.ts`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Test: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Update selector tests to the approved labels**

The expected list is:

```ts
[
  ["review", "审阅"],
  ["terminal", "终端"],
  ["preview", "预览网页"],
  ["files", "打开文件"],
  ["subtasks", "侧边任务"],
]
```

- [ ] **Step 2: Add failing launcher acceptance**

Assert that empty state has no search input or explanatory heading, and that `+` focuses a chooser labelled `打开新的…`:

```ts
await expect(panel.getByPlaceholder("搜索工作面板")).toHaveCount(0);
await expect(panel.getByTestId("work-panel-launcher")).toHaveAttribute("data-mode", "empty");

await panel.getByRole("option", { name: /^预览网页/ }).click();
// open one object, then:
await panel.getByRole("button", { name: "新建工作面板标签" }).click();
await expect(panel.getByTestId("work-panel-launcher")).toHaveAttribute("data-mode", "new");
await expect(panel.getByText("打开新的…", { exact: true })).toBeVisible();
```

- [ ] **Step 3: Run selector and acceptance tests to verify they fail**

Run:

```bash
node --test src/components/workpanel/workPanelSelectors.test.ts
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens on a launcher|preview and file adapters"
```

Expected: FAIL on old labels, root search input, and absent launcher mode.

- [ ] **Step 4: Implement `empty` and `new` launcher modes**

Add:

```ts
interface WorkPanelLauncherProps {
  mode: "empty" | "new";
  taskKey: string;
  onOpenTab: (tab: WorkPanelTab) => void;
}
```

Keep `Command` for keyboard roving but remove `CommandInput` from the root action list:

```tsx
<div className="forge-work-panel-launcher" data-mode={mode} data-testid="work-panel-launcher">
  {mode === "new" ? <div className="forge-work-panel-launcher-label">打开新的…</div> : null}
  <Command className="forge-work-panel-command forge-work-panel-launcher-command">
    <CommandList className="forge-work-panel-launcher-actions">
      <CommandGroup>
        {WORK_PANEL_LAUNCHER_ACTIONS.map((action) => {
          const Icon = actionIcons[action.id];
          return (
            <CommandItem
              key={action.id}
              role="button"
              value={action.label}
              className="forge-work-panel-launcher-action"
              onSelect={() => handleAction(action.id)}
            >
              <Icon className="size-4" />
              <span>{action.label}</span>
              {action.shortcut ? <CommandShortcut>{action.shortcut}</CommandShortcut> : null}
            </CommandItem>
          );
        })}
      </CommandGroup>
    </CommandList>
  </Command>
</div>
```

Retain focused search inputs only inside preview, file, and subtask pickers. Pass `mode={state.tabs.length === 0 ? "empty" : "new"}` from the shell.

- [ ] **Step 5: Apply the tonal-row interaction contract**

Use these exact launcher properties in `work-panel.css`:

```css
.forge-work-panel-launcher-action {
  min-height: 46px;
  border: 0;
  border-radius: 8px;
  background: var(--forge-work-panel-row);
  transition: background-color 140ms ease, color 140ms ease;
}

.forge-work-panel-launcher-action:hover,
.forge-work-panel-launcher-action[data-selected="true"] {
  background: var(--forge-work-panel-row-active);
  transform: none;
}

.forge-work-panel-launcher-action kbd {
  border-radius: 6px;
  background: transparent;
}
```

- [ ] **Step 6: Run launcher gates and commit**

Run:

```bash
node --test src/components/workpanel/workPanelSelectors.test.ts
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens on a launcher|preview and file adapters|restore keeps tabs"
npm run build
```

Expected: all pass; the launcher has five tonal rows and no root search field.

Run `detect_changes({ scope: "unstaged" })`, then:

```bash
git add apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx \
  apps/desktop/src/components/workpanel/WorkPanelShell.tsx \
  apps/desktop/src/components/workpanel/workPanelSelectors.ts \
  apps/desktop/src/components/workpanel/workPanelSelectors.test.ts \
  apps/desktop/src/styles/work-panel.css \
  apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add titleless quiet native work launcher"
```

## Task 6: Apply The Adaptive Sheet And Content-First Adapter Styling

**Files:**
- Modify: `apps/desktop/src/styles/tokens.css:140-150`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelPreview.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelFiles.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelReview.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelSubtask.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelTerminal.tsx`
- Test: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Add semantic Work Panel theme tokens**

Define dark defaults in `tokens.css`:

```css
--forge-work-panel-canvas: #1D1F1C;
--forge-work-panel-sheet: #292B28;
--forge-work-panel-row: #323531;
--forge-work-panel-row-active: #3A3D38;
--forge-work-panel-context: #262825;
--forge-work-panel-border: rgba(241, 242, 238, 0.09);
--forge-work-panel-shadow: 0 18px 42px rgba(0, 0, 0, 0.32);
--forge-work-panel-object-bar-height: 42px;
```

Override them inside the existing light theme block:

```css
--forge-work-panel-canvas: #E5E6E1;
--forge-work-panel-sheet: #FAF9F6;
--forge-work-panel-row: #EFEFEA;
--forge-work-panel-row-active: #E7E8E2;
--forge-work-panel-context: #F5F4F0;
--forge-work-panel-border: rgba(42, 46, 38, 0.10);
--forge-work-panel-shadow: 0 16px 38px rgba(46, 48, 43, 0.13);
```

- [ ] **Step 2: Make the panel one adaptive outer sheet**

Use one outer radius and shadow:

```css
.forge-work-panel-layout {
  background: var(--forge-work-panel-canvas);
}

.forge-work-panel {
  margin: 10px 10px 10px 0;
  border: 1px solid var(--forge-work-panel-border);
  border-radius: 12px;
  background: var(--forge-work-panel-sheet);
  box-shadow: var(--forge-work-panel-shadow);
}

.forge-work-panel-tab-content,
.forge-work-panel-file-view,
.forge-work-panel-web-preview,
.forge-work-panel-review {
  background: var(--forge-work-panel-sheet);
}
```

- [ ] **Step 3: Collapse adapter headers to shallow context rows**

Keep adapter semantics and actions, but make every `.forge-work-panel-content-toolbar` a 30–32px context row. Remove duplicate large titles, borders around ordinary buttons, and adapter outer shadows. For file and preview adapters, keep path/URL as muted monospace context.

Use this shared contract:

```css
.forge-work-panel-content-toolbar {
  min-height: 32px;
  border-bottom: 1px solid var(--forge-work-panel-border);
  padding: 0 10px;
  background: var(--forge-work-panel-context);
}

.forge-work-panel-content-title {
  font-size: 12px;
  font-weight: 500;
}
```

- [ ] **Step 4: Remove the nested preview card**

The web preview stage and viewport must use the sheet interior directly:

```css
.forge-work-panel-preview-stage {
  padding: 0;
  background: var(--forge-work-panel-sheet);
}

.forge-work-panel-preview-viewport {
  border: 0;
  border-radius: 0;
  box-shadow: none;
}
```

Tablet/mobile width controls may constrain iframe width, but must not add a second outer card shadow.

- [ ] **Step 5: Add container and viewport fallbacks**

Use a single-column review below 460px panel width and the approved overlay below 720px viewport width:

```css
@container (max-width: 460px) {
  .forge-work-panel-review-body {
    grid-template-columns: 1fr;
  }

  .forge-work-panel-review-files {
    max-height: 132px;
    border-right: 0;
    border-bottom: 1px solid var(--forge-work-panel-border);
  }
}

@media (max-width: 719px) {
  .forge-work-panel[data-viewport-mode="overlay"] {
    position: fixed;
    inset: var(--forge-titlebar-height) 0 0 0;
    z-index: 40;
    margin: 0;
    border-radius: 0;
  }
}
```

- [ ] **Step 6: Enforce restrained motion**

Keep only 140–180ms opacity/translation transitions. Remove launcher translate-on-hover and preview card shadow transitions. In reduced motion, remove panel/content translations and leave resizing immediate.

- [ ] **Step 7: Run adapter acceptance and build gates**

Run:

```bash
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel"
npm run check:conversation-style
npm run build
```

Expected: all Work Panel cases pass; no adapter gains a second outer boundary.

- [ ] **Step 8: Detect and commit visual integration**

Run `detect_changes({ scope: "unstaged" })`, then:

```bash
git add apps/desktop/src/styles/tokens.css apps/desktop/src/styles/work-panel.css \
  apps/desktop/src/components/workpanel/WorkPanelPreview.tsx \
  apps/desktop/src/components/workpanel/WorkPanelFiles.tsx \
  apps/desktop/src/components/workpanel/WorkPanelReview.tsx \
  apps/desktop/src/components/workpanel/WorkPanelSubtask.tsx \
  apps/desktop/src/components/workpanel/WorkPanelTerminal.tsx \
  apps/desktop/e2e/acceptance.spec.ts
git commit -m "style(desktop): apply quiet native work panel surfaces"
```

## Task 7: Complete Product Acceptance, Documentation, And Visual Proof

**Files:**
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`

- [ ] **Step 1: Add final responsive and restoration acceptance**

Add one test that proves per-task width survives closing and the overlay activates on narrow viewports:

```ts
test("work panel restores task width and uses a narrow overlay", async ({ page }) => {
  await page.getByRole("button", { name: "打开工作面板" }).click();
  const panel = page.getByRole("complementary", { name: "工作面板" });
  await page.getByRole("separator", { name: "调整工作面板宽度" }).dblclick();
  await expect(panel).toHaveAttribute("data-width-percent", "40");

  await panel.getByRole("button", { name: "关闭工作面板" }).click();
  await page.getByRole("button", { name: "打开工作面板" }).click();
  await expect(panel).toHaveAttribute("data-width-percent", "40");

  await page.setViewportSize({ width: 700, height: 900 });
  await expect(panel).toHaveAttribute("data-viewport-mode", "overlay");
  await expect(page.getByTestId("main-workbench")).toBeVisible();
});
```

- [ ] **Step 2: Assert the anti-slop visual contract in acceptance**

Use computed styles to prove the structural rules rather than pixel snapshots:

```ts
const launcherRow = panel.getByRole("option", { name: /^审阅/ });
await expect(launcherRow).toHaveCSS("border-top-width", "0px");
await expect(launcherRow).toHaveCSS("transform", "none");

// After opening a web preview:
const viewport = panel.locator(".forge-work-panel-preview-viewport");
await expect(viewport).toHaveCSS("border-top-width", "0px");
await expect(viewport).toHaveCSS("box-shadow", "none");
```

- [ ] **Step 3: Update user-visible documentation**

Document these exact outcomes:

- titleless first-open launcher;
- five explicit user-chosen objects;
- fused object bar and deduplicated tabs;
- default 40%, task-scoped width, 34%–62% split range, and narrow overlay;
- complete light/dark consumption through the existing theme state;
- temporary recent-output terminal rather than an embedded terminal manager;
- experience and memory remain hidden implementation details.

Update `scripts/acceptance.sh` descriptions only if the advertised Work Panel coverage text changes.

- [ ] **Step 4: Run the full required verification matrix**

Run:

```bash
npm run build:desktop
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
npm --prefix apps/desktop run check:frontend-architecture
npm --prefix apps/desktop run check:security-config
npm --prefix apps/desktop run check:backend
scripts/acceptance.sh --dry-run
```

Expected: all commands pass. If a workspace command is unavailable from the monorepo root, run the equivalent documented command from `apps/desktop` and record both the failed wrapper and passing direct command.

- [ ] **Step 5: Capture manual visual proof in both themes**

Launch the merged worktree desktop app and inspect these states at 360px, default 40%, and 62% widths:

```text
Light: empty launcher, web preview, file, review, terminal, new-object chooser
Dark: empty launcher, web preview, file, review, terminal, new-object chooser
Narrow: 700px viewport overlay with visible return/close control
```

Reject the result if any state shows a fixed `工作面板` title, bordered card wall, solid-black selected launcher row, nested preview card, gradient decoration, exposed experience/memory label, or more than one outer sheet shadow.

- [ ] **Step 6: Run final GitNexus change detection**

Run:

```text
detect_changes({ scope: "compare", base_ref: "main", worktree: "/Users/cabbos/.config/superpowers/worktrees/forge/work-panel" })
```

Expected: the report will remain HIGH/CRITICAL because the feature branch changes `AppShell` and replaces the old Project Archive flow. Review that every affected flow is covered by the build, acceptance, security, and backend gates above; do not dismiss the risk based only on visual success.

- [ ] **Step 7: Commit documentation and acceptance evidence**

```bash
git add README.md apps/desktop/README.md CHANGELOG.md \
  apps/desktop/e2e/acceptance.spec.ts scripts/acceptance.sh
git commit -m "docs(desktop): document quiet native work panel behavior"
```

Run `git status --short` and verify only the pre-existing user-owned files remain unstaged.

## Completion Criteria

- First open is a titleless launcher with five tonal full-width rows.
- No preview, file, or subtask opens without user choice.
- `+` asks what to open and never asks for a tab category.
- Open content uses one fused object bar and one outer sheet.
- Web preview has no nested card boundary.
- Width defaults to 40%, restores per task, stays within 34%–62% on ordinary desktops, and uses the narrow overlay below 720px.
- Existing theme selection visibly drives both the whole workbench and the conversation; first-run default still comes from the system preference.
- Terminal uses lightweight command input and recent output, with no xterm dependency or Forge tool-log exposure.
- Light/dark, reduced-motion, keyboard focus, and responsive acceptance all pass.
- `Project Archive`, `经验`, and internal memory-management UI remain absent from the Work Panel.
