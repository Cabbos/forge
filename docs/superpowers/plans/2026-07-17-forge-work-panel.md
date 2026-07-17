# Forge Work Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the desktop Project Archive with a resizable, launcher-first work panel whose user-selected preview, review, terminal, file, and subtask objects open as restorable dynamic tabs.

**Architecture:** Put the conversation and work panel inside `react-resizable-panels`, keep tab transitions in a pure task-scoped state module, and render each tab through a narrow adapter. Reuse the existing Base UI/shadcn tabs and command primitives, workspace file queries, preview ownership query, diff presentation, and A2A projection; add only the missing bounded Git-diff and temporary-PTY IPC authorities.

**Tech Stack:** React 18, TypeScript, Base UI/shadcn, cmdk, `react-resizable-panels`, `react-diff-viewer-continued`, Shiki, React Query, Zustand, Tauri 2, Rust, `portable-pty`, `@xterm/xterm`, Playwright.

**Design spec:** `docs/superpowers/specs/2026-07-17-forge-work-panel-design.md`

**Open-source reference decisions:** Use Base UI Tabs for keyboard/ARIA behavior, cmdk through the existing shadcn Command wrapper for launcher search, `react-resizable-panels` Group/Panel/Separator for the split layout, `react-diff-viewer-continued` for review line rendering, and xterm.js plus FitAddon for the temporary terminal. Do not build replacement tab focus management, resize math, diff layout, command filtering, or terminal emulation.

---

## File Map

Create the focused frontend feature directory `apps/desktop/src/components/workpanel/`:

- `workPanelTypes.ts`: serializable tab identities and launcher action types.
- `workPanelState.ts`: pure open/focus/close/deduplicate/task-switch reducer.
- `workPanelPersistence.ts`: versioned localStorage parsing and task-scoped persistence.
- `workPanelState.test.ts`: pure reducer and persistence coverage.
- `WorkPanelLayout.tsx`: resizable conversation/panel composition and global open events.
- `WorkPanelShell.tsx`: header, dynamic tab strip, maximize/restore, close, and active outlet.
- `WorkPanelLauncher.tsx`: launcher rows, search, and target selection.
- `WorkPanelContent.tsx`: discriminated adapter routing.
- `WorkPanelPreview.tsx`: loopback web preview and previewable-file selection.
- `WorkPanelFiles.tsx`: workspace search/list and read-only file view.
- `WorkPanelReview.tsx`: current worktree review and row feedback.
- `WorkPanelSubtask.tsx`: selected A2A subtask detail and actions.
- `WorkPanelTerminal.tsx`: xterm.js lifecycle bound to the task PTY.
- `workPanelSelectors.ts`: launcher result derivation and labels.
- `workPanelSelectors.test.ts`: labels, target identity, URL validation, and launcher ordering.

Create the missing backend authority:

- `apps/desktop/src-tauri/src/ipc/workspace_review.rs`: bounded current-worktree diff command.
- `apps/desktop/src-tauri/src/ipc/workspace_terminal.rs`: one task-scoped PTY with start/write/resize/restart/close.

Modify integration surfaces only where required:

- `apps/desktop/src/components/layout/AppShell.tsx`
- `apps/desktop/src/components/layout/AppTitlebar.tsx`
- `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- `apps/desktop/src/lib/ipc/types.ts`
- `apps/desktop/src/lib/ipc/files.ts`
- `apps/desktop/src/lib/ipc/project.ts`
- `apps/desktop/src/lib/tauri.ts`
- `apps/desktop/src-tauri/src/ipc/mod.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/state.rs`
- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/src/styles/globals.css`
- `apps/desktop/src/styles/tokens.css`
- `apps/desktop/src/styles/work-panel.css`
- `apps/desktop/package.json`
- `apps/desktop/package-lock.json`
- `apps/desktop/e2e/fixtures/app.ts`
- `apps/desktop/e2e/acceptance.spec.ts`
- `apps/desktop/scripts/desktop-security-config.test.mjs`
- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `scripts/acceptance.sh`

Delete the old Hub/Archive frontend files only after `rg` proves they have no remaining imports. Do not delete continuity, memory, Wiki, or backend record code.

### Task 1: Lock the dynamic-tab contract

**Files:**
- Create: `apps/desktop/src/components/workpanel/workPanelTypes.ts`
- Create: `apps/desktop/src/components/workpanel/workPanelState.ts`
- Create: `apps/desktop/src/components/workpanel/workPanelPersistence.ts`
- Create: `apps/desktop/src/components/workpanel/workPanelState.test.ts`

- [ ] **Step 1: Write reducer tests for launcher-first behavior and deduplication**

```ts
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { closeWorkPanelTab, openWorkPanelTab, restoreTaskPanelState } from "./workPanelState.ts";

describe("work panel state", () => {
  it("starts on the launcher without fabricating a tab", () => {
    assert.deepEqual(restoreTaskPanelState(null), { tabs: [], activeTabId: null, launcherOpen: true });
  });

  it("focuses an existing object instead of opening a duplicate", () => {
    const first = openWorkPanelTab(restoreTaskPanelState(null), {
      kind: "file", id: "file:README.md", label: "README.md", path: "README.md",
    });
    const second = openWorkPanelTab(first, {
      kind: "file", id: "file:README.md", label: "README.md", path: "README.md",
    });
    assert.equal(second.tabs.length, 1);
    assert.equal(second.activeTabId, "file:README.md");
  });

  it("returns to the launcher after the final tab closes", () => {
    const opened = openWorkPanelTab(restoreTaskPanelState(null), {
      kind: "terminal", id: "terminal:task-1", label: "终端", taskId: "task-1",
    });
    assert.equal(closeWorkPanelTab(opened, "terminal:task-1").launcherOpen, true);
  });
});
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelState.test.ts`

Expected: FAIL because the state modules do not exist.

- [ ] **Step 3: Implement the discriminated tab types and pure transitions**

```ts
export type WorkPanelTab =
  | { kind: "review"; id: string; label: string; taskId: string }
  | { kind: "terminal"; id: string; label: string; taskId: string }
  | { kind: "preview"; id: string; label: string; target: PreviewTarget }
  | { kind: "file"; id: string; label: string; path: string }
  | { kind: "subtask"; id: string; label: string; taskId: string; subtaskId: string };

export interface WorkPanelTaskState {
  tabs: WorkPanelTab[];
  activeTabId: string | null;
  launcherOpen: boolean;
}
```

Implement `openWorkPanelTab`, `closeWorkPanelTab`, `focusWorkPanelTab`, `openWorkPanelLauncher`, and `restoreTaskPanelState` as immutable pure functions. An open tab closes the launcher; `+` sets `launcherOpen`; selecting a target replaces the launcher; duplicate IDs focus the existing tab.

- [ ] **Step 4: Implement versioned task-scoped persistence**

Use key `forge-work-panel-v1` with shape `{ version: 1, tasks: Record<string, WorkPanelTaskState> }`. Parse unknown JSON defensively; retain only recognized kinds and non-empty IDs; never serialize live terminal handles or DOM state.

- [ ] **Step 5: Run the state tests**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelState.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit the state contract**

```bash
git add apps/desktop/src/components/workpanel/workPanelTypes.ts apps/desktop/src/components/workpanel/workPanelState.ts apps/desktop/src/components/workpanel/workPanelPersistence.ts apps/desktop/src/components/workpanel/workPanelState.test.ts
git commit -m "feat(desktop): define work panel tab state"
```

### Task 2: Replace the archive overlay with the resizable work-panel skeleton

**Files:**
- Create: `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`
- Create: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Create: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`
- Modify: `apps/desktop/src/components/layout/AppShell.tsx`
- Modify: `apps/desktop/src/components/layout/AppTitlebar.tsx`
- Modify: `apps/desktop/src/styles/globals.css`
- Modify: `apps/desktop/src/styles/tokens.css`
- Create: `apps/desktop/src/styles/work-panel.css`

- [ ] **Step 1: Extend acceptance with the first-open launcher and new title-bar name**

```ts
test("work panel opens on the launcher without project archive content", async ({ page }) => {
  await page.getByRole("button", { name: "打开工作面板" }).click();
  const panel = page.getByRole("complementary", { name: "工作面板" });
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "审阅" })).toBeVisible();
  await expect(panel.getByRole("button", { name: "预览" })).toBeVisible();
  await expect(panel.getByText("经验回忆")).toHaveCount(0);
  await expect(panel.getByText("项目档案", { exact: true })).toHaveCount(0);
});
```

- [ ] **Step 2: Run the scenario and verify it fails**

Run: `cd apps/desktop && npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens"`

Expected: FAIL because the title-bar control and panel still use Project Archive.

- [ ] **Step 3: Add `WorkPanelLayout` using the installed resize library**

Use `Group`, `Panel`, `Separator`, and `usePanelRef` from `react-resizable-panels`:

```tsx
<Group orientation="horizontal" className="forge-work-panel-layout">
  <Panel id="conversation" minSize="35%" preserveRelativeSize>{children}</Panel>
  {open && <Separator id="work-panel-separator" className="forge-work-panel-separator" />}
  {open && <Panel id="work-panel" panelRef={panelRef} defaultSize="45%" minSize="30%" maxSize="70%">
    <WorkPanelShell {...panelProps} />
  </Panel>}
</Group>
```

Listen for `toggle-work-panel`, `open-work-panel`, and the legacy `toggle-hub`/`open-hub` events during migration. Bind state to `activeSessionId ?? activeWorkspace.path` and restore persisted tabs for that owner key.

Persist the last non-maximized work-panel percentage under `forge-work-panel-width-v1`. The maximize button calls `panelRef.resize("100%")`; restore returns to the saved percentage. `onResize` updates the saved percentage only while the panel is not maximized.

- [ ] **Step 4: Add the shell and dynamic tab strip with Base UI**

Use the existing `@/components/ui/tabs` wrapper. The shell must render header controls, horizontally scrollable concrete tabs, a fixed `+` button, and the active content outlet. Closing the panel preserves tab state; closing the final tab shows the launcher.

- [ ] **Step 5: Replace `HubPanelHost` in `AppShell` and rename the title-bar control**

Wrap the existing `<main>` in `WorkPanelLayout`, remove `<HubPanelHost />`, change `onOpenHub` to `onOpenWorkPanel`, and change accessible labels from `打开项目档案` to `打开工作面板`.

- [ ] **Step 6: Add the calm base styling**

Import `work-panel.css` from `globals.css`. Remove the old `data-project-archive-open` padding rule from the active layout path. Use design tokens for a continuous light surface, a 10px accessible separator hit target, a compact 36px tab rail, launcher whitespace, and responsive minimum widths.

- [ ] **Step 7: Run focused acceptance and build**

Run:

```bash
cd apps/desktop
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel opens"
npm run build
```

Expected: both commands PASS.

- [ ] **Step 8: Commit the overall skeleton**

```bash
git add apps/desktop/src/components/workpanel apps/desktop/src/components/layout/AppShell.tsx apps/desktop/src/components/layout/AppTitlebar.tsx apps/desktop/src/styles/globals.css apps/desktop/src/styles/tokens.css apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add resizable work panel shell"
```

### Task 3: Build the searchable launcher and real object selection

**Files:**
- Create: `apps/desktop/src/components/workpanel/WorkPanelLauncher.tsx`
- Create: `apps/desktop/src/components/workpanel/workPanelSelectors.ts`
- Create: `apps/desktop/src/components/workpanel/workPanelSelectors.test.ts`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`

- [ ] **Step 1: Test launcher order, labels, target IDs, and loopback URL validation**

Assert the exact order `review`, `terminal`, `preview`, `files`, `subtasks`; assert `file:README.md` and `preview:http://localhost:1420`; reject non-HTTP schemes and non-loopback hosts for embedded web preview.

- [ ] **Step 2: Implement the launcher with the existing shadcn/cmdk primitives**

Use `ForgeCommand`, `ForgeCommandInput`, `ForgeCommandList`, `ForgeCommandGroup`, `ForgeCommandItem`, and `ForgeCommandShortcut`. Render the five large action rows without descriptions. Query `useSearchWorkspaceFilesQuery` only when the user types, and derive preview/subtask results from existing runtime/store projections.

- [ ] **Step 3: Implement two-step action selection**

`审阅` and `终端` open their singleton task objects immediately. `预览`, `文件`, and `子任务` switch the command list into a target-selection phase. Escape returns to the root launcher; choosing a target calls `onOpenTab(tab)`.

- [ ] **Step 4: Verify the launcher interactions**

Run: `cd apps/desktop && node --test src/components/workpanel/workPanelSelectors.test.ts && npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel launcher"`

Expected: PASS.

- [ ] **Step 5: Commit the launcher**

```bash
git add apps/desktop/src/components/workpanel apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add work panel launcher"
```

### Task 4: Add preview and read-only Files adapters

**Files:**
- Create: `apps/desktop/src/components/workpanel/WorkPanelPreview.tsx`
- Create: `apps/desktop/src/components/workpanel/WorkPanelFiles.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`
- Modify: `apps/desktop/src/lib/ipc/files.ts`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Modify: `apps/desktop/scripts/desktop-security-config.test.mjs`

- [ ] **Step 1: Add failing acceptance for explicit preview choice and file reuse**

Prove that an available `http://localhost:1420` target does not open by itself, selecting it creates `localhost:1420`, selecting `README.md` creates one `README.md` tab, and selecting it again focuses rather than duplicates.

- [ ] **Step 2: Implement file content by reusing existing query and renderers**

Use `usePreviewFileQuery`, `deriveFilePreviewView`, `FilePreviewBody`, and `FilePreviewActions` inside the tab instead of opening `FilePreviewSheet`. Use `useSearchWorkspaceFilesQuery` for tree/search results and provide `在预览中打开` through the shared tab opener.

- [ ] **Step 3: Implement loopback web preview with a bounded toolbar**

Render an iframe only after `validatePreviewUrl` confirms `http:` or `https:` and hostname `localhost`, `127.0.0.1`, or `[::1]`. Add refresh, external open, and desktop/tablet/mobile width controls. Never accept `file:`, `javascript:`, LAN hosts, or arbitrary remote origins.

- [ ] **Step 4: Update CSP narrowly and test it**

Add `frame-src http://localhost:* http://127.0.0.1:*` to production and dev CSP. Extend `desktop-security-config.test.mjs` to assert that `default-src 'self'`, `object-src 'none'`, and `frame-ancestors 'none'` remain and that frame sources are loopback-only.

- [ ] **Step 5: Run security, frontend, and acceptance checks**

Run:

```bash
cd apps/desktop
npm run check:security-config
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel preview|work panel file"
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit preview and Files**

```bash
git add apps/desktop/src/components/workpanel apps/desktop/src/lib/ipc/files.ts apps/desktop/src-tauri/tauri.conf.json apps/desktop/scripts/desktop-security-config.test.mjs apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add work panel preview and files"
```

### Task 5: Add current-change Review and one-shot row feedback

**Files:**
- Create: `apps/desktop/src-tauri/src/ipc/workspace_review.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/lib/ipc/types.ts`
- Create: `apps/desktop/src/lib/ipc/review.ts`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Create: `apps/desktop/src/hooks/queries/useWorkspaceReviewQuery.ts`
- Modify: `apps/desktop/src/hooks/queries/queryKeys.ts`
- Create: `apps/desktop/src/components/workpanel/WorkPanelReview.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`

- [ ] **Step 1: Add backend tests for workspace binding and bounded diff output**

Create a temporary Git repository, commit a baseline, modify one file, and assert the response includes the bound relative path, additions/deletions, and patch. Assert a mismatched explicit working directory cannot escape the session workspace. Cap the response at 2 MiB and return `truncated: true` when exceeded.

- [ ] **Step 2: Implement `get_workspace_review`**

Resolve the working directory through `resolve_bound_working_dir`. Run Git without a shell using arguments equivalent to `git diff --no-ext-diff --no-color --find-renames HEAD --`. Parse file headers and stats into:

```ts
interface WorkspaceReview {
  working_dir: string;
  patch: string;
  files: Array<{ path: string; status: "added" | "modified" | "renamed" | "deleted"; additions: number; deletions: number }>;
  truncated: boolean;
}
```

- [ ] **Step 3: Add the typed IPC query**

Create `getWorkspaceReview(sessionId, workingDir)` and a React Query hook with a task-scoped key. Refetch when Review gains focus and after a session finishes streaming.

- [ ] **Step 4: Render with the installed diff component**

Use `react-diff-viewer-continued` for unified line layout and line-number clicks. Keep the file list outside the diff component. A click opens one feedback input containing the selected path and line; submission calls `useStore.getState().setPendingInput(...)`, dispatches focus to the composer, and closes the inline input.

- [ ] **Step 5: Verify review feedback**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml workspace_review
cd apps/desktop && npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel review"
```

Expected: PASS.

- [ ] **Step 6: Commit Review**

```bash
git add apps/desktop/src-tauri/src/ipc/workspace_review.rs apps/desktop/src-tauri/src/ipc/mod.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src/lib/ipc apps/desktop/src/lib/tauri.ts apps/desktop/src/hooks/queries apps/desktop/src/components/workpanel apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add work panel review"
```

### Task 6: Add focused subtask tabs

**Files:**
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Create: `apps/desktop/src/components/workpanel/WorkPanelSubtask.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`

- [ ] **Step 1: Add acceptance for explicit subtask selection**

Mock two A2A tasks. Prove the launcher lists both, selecting `设置诊断` creates `子任务 · 设置诊断`, and the tab shows only the chosen task's goal, state, latest progress, output, and available review/takeover action.

- [ ] **Step 2: Extract a focused public task detail from the existing A2A surface**

Export a presentation component that accepts one `AgentA2ATask` plus the existing runtime facts and review callbacks. Keep review authority in `reviewAgentA2ATasks`; do not duplicate task-state derivation.

- [ ] **Step 3: Route subtask actions through existing authority**

Append-instruction uses the existing session input path when available. Takeover/review buttons remain disabled with a reason when the projection does not expose an allowed action.

- [ ] **Step 4: Run A2A and work-panel acceptance**

Run: `cd apps/desktop && npm run test:e2e -- e2e/acceptance.spec.ts --grep "A2A|work panel subtask"`

Expected: PASS.

- [ ] **Step 5: Commit subtask tabs**

```bash
git add apps/desktop/src/components/messages/AgentA2ATimeline.tsx apps/desktop/src/components/workpanel apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add focused subtask tabs"
```

### Task 7: Add the one-task temporary terminal using xterm.js and portable-pty

**Files:**
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/package-lock.json`
- Create: `apps/desktop/src-tauri/src/ipc/workspace_terminal.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src/lib/ipc/types.ts`
- Create: `apps/desktop/src/lib/ipc/terminal.ts`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Create: `apps/desktop/src/components/workpanel/WorkPanelTerminal.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`

- [ ] **Step 1: Install the mature terminal renderer**

Run: `cd apps/desktop && npm install @xterm/xterm @xterm/addon-fit`

Expected: `package.json` and `package-lock.json` contain both packages.

- [ ] **Step 2: Add backend lifecycle tests**

Test that one terminal is keyed to one bound task/workspace, input reaches the PTY, resize is clamped to safe rows/columns, close removes the handle, and another session cannot write to it. Use a short `printf`/exit command through the platform shell and a timeout-bounded output channel.

- [ ] **Step 3: Implement terminal commands and output event**

Add `start_workspace_terminal`, `write_workspace_terminal`, `resize_workspace_terminal`, and `close_workspace_terminal`. Spawn the user's shell through `portable-pty` with the resolved workspace as cwd. Emit `{ terminal_id, task_id, chunk }` on `work-panel-terminal-output`. Keep one live handle per task; restart closes the old handle before starting a new one.

- [ ] **Step 4: Implement the xterm adapter**

Create one `Terminal`, load `FitAddon`, listen to `onData`, and bridge typed IPC. Resize via `ResizeObserver`. Dispose listeners and the renderer on unmount without closing the backend PTY when merely switching tabs; close only on explicit tab close or task release.

- [ ] **Step 5: Verify terminal lifecycle**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml workspace_terminal
cd apps/desktop
npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel terminal"
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit the temporary terminal**

```bash
git add apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/src-tauri/src/ipc/workspace_terminal.rs apps/desktop/src-tauri/src/ipc/mod.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/state.rs apps/desktop/src/lib/ipc apps/desktop/src/lib/tauri.ts apps/desktop/src/components/workpanel apps/desktop/e2e/acceptance.spec.ts
git commit -m "feat(desktop): add temporary work panel terminal"
```

### Task 8: Finish restoration, accessibility, motion, and unavailable states

**Files:**
- Modify: `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- Modify: `apps/desktop/src/components/workpanel/WorkPanelContent.tsx`
- Modify: `apps/desktop/src/styles/work-panel.css`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Add acceptance for restore, task separation, keyboard use, and failure isolation**

Cover close/reopen restore, separate tab sets for two sessions, `+` launcher focus, arrow-key tab focus, labelled resize separator/maximize controls, missing file tab, failed iframe tab, and terminal exit without affecting sibling tabs.

- [ ] **Step 2: Implement stable restoration and unavailable views**

Validate persisted objects when their tabs activate. Preserve the tab identity and show retry/close when an object is gone. Corrupt task state falls back to the launcher without clearing other tasks.

- [ ] **Step 3: Add restrained motion and reduced-motion handling**

Reuse `forgeMotion`/GSAP only for panel entry and content fade. Do not animate drag resize. Under `prefers-reduced-motion`, render final positions immediately.

- [ ] **Step 4: Run focused accessibility and restoration acceptance**

Run: `cd apps/desktop && npm run test:e2e -- e2e/acceptance.spec.ts --grep "work panel restore|work panel keyboard|work panel unavailable"`

Expected: PASS.

- [ ] **Step 5: Commit state hardening**

```bash
git add apps/desktop/src/components/workpanel apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts
git commit -m "fix(desktop): harden work panel restoration"
```

### Task 9: Remove the user-facing Project Archive path and update product documentation

**Files:**
- Delete only unreferenced files under: `apps/desktop/src/components/layout/HubPanel*.tsx`
- Delete only unreferenced archive presentation files under: `apps/desktop/src/components/layout/archive/`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Prove old components are unreferenced before deletion**

Run: `rg -n "HubPanel|Project Archive|项目档案|ContinuityExperiencesSection" apps/desktop/src apps/desktop/e2e README.md apps/desktop/README.md CHANGELOG.md`

Expected: remaining hits are backend/internal memory descriptions or migration tests, not active work-panel UI imports.

- [ ] **Step 2: Delete only dead Project Archive presentation code**

Remove the unused Hub shell/content/archive summary components. Keep continuity, Wiki, unified memory, settings, queries, IPC, and tests that prove backend context behavior.

- [ ] **Step 3: Update user-facing docs and acceptance descriptions**

Describe `工作面板`, launcher-first dynamic tabs, explicit preview, Review feedback, temporary terminal, Files, and subtask tabs. State that continuity and memory are background context systems rather than work-panel navigation.

- [ ] **Step 4: Align the dry-run acceptance matrix**

Update the advertised desktop UI gate label and grep coverage so `scripts/acceptance.sh --dry-run` names Work Panel rather than Project Archive where the behavior changed.

- [ ] **Step 5: Run docs and boundary checks**

Run:

```bash
cd apps/desktop
npm run check:desktop-boundary
npm run check:conversation-style
cd ../..
scripts/acceptance.sh --dry-run
```

Expected: PASS, and dry-run output advertises Work Panel coverage.

- [ ] **Step 6: Commit cleanup and docs**

```bash
git add README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh apps/desktop/src apps/desktop/e2e/acceptance.spec.ts
git commit -m "docs(desktop): replace project archive with work panel"
```

### Task 10: Full verification and change-impact audit

**Files:**
- Modify only if verification reveals an in-scope defect.

- [ ] **Step 1: Run frontend unit and architecture checks**

```bash
cd apps/desktop
node --test src/components/workpanel/*.test.ts
npm run check:frontend-architecture
npm run check:protocol
npm run check:security-config
npm run build
```

Expected: every command PASS.

- [ ] **Step 2: Run backend focused and full checks**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml workspace_review workspace_terminal
npm --prefix apps/desktop run check:backend
```

Expected: every command PASS.

- [ ] **Step 3: Run product acceptance**

```bash
cd apps/desktop
npm run test:e2e -- e2e/acceptance.spec.ts
cd ../..
scripts/acceptance.sh --dry-run
```

Expected: acceptance PASS and dry-run exits 0.

- [ ] **Step 4: Inspect the rendered surface at 1200×800 and 1440×900**

Verify launcher whitespace, 45 percent default width, drag resize, maximize/restore, tab overflow, explicit preview selection, and no Project Archive or experience UI. Capture screenshots as test artifacts; do not commit generated runtime noise.

- [ ] **Step 5: Run GitNexus change detection before the final commit**

Run `detect_changes({ scope: "compare", base_ref: "main" })`. Review every affected process and run `context` on any high-risk symbol. If the index cannot analyze a changed symbol, record the required fallback impact report before proceeding.

- [ ] **Step 6: Confirm worktree hygiene**

Run: `git status --short` and `git diff --check`.

Expected: only intentional Work Panel changes remain; pre-existing unrelated user changes are neither staged nor modified by this plan.

- [ ] **Step 7: Commit final verification fixes if any**

```bash
git add apps/desktop/src/components/workpanel apps/desktop/src/components/layout/AppShell.tsx apps/desktop/src/components/layout/AppTitlebar.tsx apps/desktop/src/components/messages/AgentA2ATimeline.tsx apps/desktop/src/lib/ipc apps/desktop/src/lib/tauri.ts apps/desktop/src/hooks/queries apps/desktop/src-tauri/src/ipc apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/state.rs apps/desktop/src/styles/work-panel.css apps/desktop/e2e/acceptance.spec.ts README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh
git commit -m "test(desktop): verify work panel"
```

Skip this commit when verification required no code changes.
