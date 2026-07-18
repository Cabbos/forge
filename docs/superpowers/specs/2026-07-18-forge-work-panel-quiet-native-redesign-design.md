# Forge Work Panel Embedded Split Redesign

Date: 2026-07-18
Status: approved in design discussion; pending written-spec review
Scope: Desktop Work Panel structure, layout, and interaction
Supersedes: the panel-frame, launcher, width, responsive, and interaction sections of `2026-07-17-forge-work-panel-design.md`

## Goal

Make the Work Panel feel like a native part of the desktop workspace instead of a large drawer containing another card. The panel must support preview, file inspection, review, temporary terminal verification, and subtask inspection without competing with the conversation or exposing internal memory and experience systems.

This redesign intentionally does not change the theme, palette, typography direction, or other visual tokens already being adjusted elsewhere. It changes structure, hierarchy, spacing, and interaction only.

## User Model

The user opens an adjacent workspace, chooses a concrete object, inspects or acts on it, and returns to the conversation without leaving the task.

The Work Panel follows these rules:

1. It is an embedded workspace column, not a floating Sheet or modal drawer.
2. Tabs represent concrete objects, not categories.
3. Preview, file, and task content opens only after explicit user action.
4. Review shows the latest useful result instead of a version archive.
5. Terminal is a temporary verification tool, not a terminal-history product.
6. Memory, experience, continuity, and recall remain implementation details.

## Selected Direction

The selected direction is **Native Embedded Split**.

Rejected alternatives:

- **Overlay drawer:** preserves conversation width but makes preview and file inspection feel temporary and spatially unstable.
- **Persistent tool dock:** speeds switching but pushes Forge toward an IDE-style interface and keeps utility chrome visible when it is not useful.

## Workspace Structure

At ordinary desktop widths, the workbench has this structure:

`Sidebar | Conversation | Resize Divider | Work Panel`

The Work Panel:

- touches the top, right, and bottom edges of the available workbench;
- has no surrounding margin, large radius, outer shadow, or inset frame;
- shares the same base plane as the conversation;
- is separated from the conversation by one visually quiet divider;
- pushes the conversation aside instead of covering it.

The redesign removes the current nested-surface effect. There must not be a rounded application canvas containing a second rounded Work Panel sheet containing a third content card.

## Width And Resize

- Default opened width: `440px` when the available workbench can support it.
- The user can drag the divider to resize the panel.
- The visible divider may remain thin while its pointer hit area is wider and easy to acquire.
- The last explicit width is remembered per task.
- Resizing is immediate and does not use easing, spring motion, or delayed content reflow.
- In split mode, the panel minimum is `360px` and the maximum is the lesser of `920px` or the width that leaves at least `400px` for the conversation.

When the available workbench is narrower than `840px`, the Work Panel occupies the workbench instead of forcing two unusable columns. It remains flush to the workbench edges and does not revert to a floating card with a large surrounding frame.

## Visible States

### 1. Closed

The conversation uses the available workbench. Closing the Work Panel hides it without destroying the current task's tabs or remembered width.

### 2. Empty Launcher

The first open for a task, or closing the final object tab, shows a titleless launcher.

The launcher:

- contains exactly five actions in this order: `审阅`, `终端`, `预览网页`, `打开文件`, `侧边任务`;
- presents them as one compact vertical command list centered horizontally and positioned `48px` above the bottom edge;
- limits the command list to `360px` or the panel width minus `48px`, whichever is smaller;
- uses `48px` rows with `8px` gaps;
- has no enclosing card, title, description, status summary, search field, or project archive content;
- aligns icon, label, and optional shortcut on a consistent baseline;
- has no default selected or highlighted row;
- shows hover or keyboard focus only after actual user interaction.

The launcher occupies only the space needed by the commands. It does not stretch each action into a large full-panel settings row.

The empty panel has no title bar. Expand and close remain aligned in the shared top control region; they do not float at unrelated coordinates.

### 3. Choosing An Object

Selecting `审阅` or `终端` opens the object directly.

Selecting `预览网页`, `打开文件`, or `侧边任务` replaces the launcher with an in-panel chooser. The launcher does not remain visible as an accordion and the chooser does not open as a second modal card.

After the user chooses a target:

- the chooser exits;
- the object opens in the same panel;
- the object receives or reuses a concrete tab;
- focus moves to the opened object or its first useful control.

Cancelling the chooser returns to the previous object when one exists, otherwise to the empty launcher.

### 4. Object Open

The active object fills all space below the object bar. Preview, file, review, terminal, and subtask adapters must not add another outer card, large inset, or redundant header.

### 5. Unavailable Object

An unavailable preview, file, or task remains local to its tab. It shows a concise reason and relevant `重试`, `返回`, or `关闭` actions in the content area. It does not reset the entire panel or open another dialog.

## Object Bar And Tabs

Opened content uses one compact object bar.

- Tab labels are concrete object names such as `localhost:64059`, `README.md`, or `审阅 · 3 个文件`.
- Tabs do not display product categories.
- Opening an already-open object focuses and updates the existing tab instead of duplicating it.
- Review reuses its tab and shows the latest result; it does not create version tabs.
- `+` opens a chooser containing the same five actions as the empty launcher.
- Closing the final tab returns to the empty launcher.
- Closing the panel hides it while preserving its remaining tabs.
- Reopening the panel restores the last valid active object for that task.

Expand, close, add, overflow, and object-specific actions belong to the same compact top control system. Controls must not float independently in empty space.

## Content Rules

### Review

- Shows the current change set and latest review result only.
- Sends selected feedback back into the active conversation.
- Does not expose review-version history or internal reasoning archives.

### Terminal

- Opens as one temporary task-scoped verification surface.
- Does not expose Forge execution logs or internal terminal history.
- Closing the terminal tab ends the temporary session and removes its transient UI state.

### Preview

- Opens only after the user selects `预览网页` and chooses a target.
- Fills the available content area without a preview card wrapper.
- Reuses an existing tab for the same target.

### Files

- Opens only after the user selects `打开文件` and chooses a file.
- Remains read-oriented and does not become a general code editor.
- Fills the available content area without redundant outer padding.

### Subtasks

- Shows the current useful state of the selected subtask.
- Does not expose orchestration internals or require the user to manage agent architecture.

## Interaction And Motion

- Opening and closing the panel uses a `160ms` ease-out width transition shared with the conversation reflow.
- Object changes may use a `120ms` opacity transition when it does not delay interaction.
- Launcher actions do not scale, bounce, spring, or simulate thick button presses.
- A click on an action begins one clear state transition; it must not cause stacked menus, nested cards, or multiple competing focus targets.
- Loading remains inside the content region and does not flash the whole Work Panel blank.
- Restoring a task keeps the layout stable and does not visibly jump through default and saved widths.
- `prefers-reduced-motion` removes nonessential transitions.

## Keyboard And Accessibility

- The launcher, tabs, add, close, expand, overflow, and object controls are fully keyboard reachable.
- Launcher focus follows logical vertical movement and exposes a visible focus state.
- No launcher row appears selected before pointer or keyboard interaction.
- The tab list exposes the active tab and uses predictable keyboard navigation.
- `Escape` closes only the current transient chooser or menu; it does not unexpectedly close the whole panel.
- Closing the panel returns focus to its trigger.
- The resize divider has an accessible keyboard alternative.
- Visible control targets remain usable in dense desktop chrome.
- Theme contrast remains governed by the existing visual system and is outside this structural redesign.

## State And Data Flow

The existing task-scoped Work Panel state remains the source of truth:

1. The panel trigger reveals the current task's stored panel state.
2. No tabs produces the empty launcher.
3. An action either opens an immediate object or enters an in-panel chooser.
4. A completed choice creates or focuses a concrete object tab.
5. The active tab determines the content adapter.
6. Tab, active object, open/closed state, and explicit width changes persist at task scope.
7. Invalid restored objects remain isolated unavailable tabs.

The implementation should preserve the existing focused component boundaries for layout, shell, launcher, object bar, persistence, and content adapters unless discovery finds a concrete blocker.

## Verification

### Visual Acceptance

Capture and compare the implemented Work Panel at the same viewport and state as the supplied reference and current screenshot.

Verify:

- no surrounding Work Panel card or oversized frame;
- one divider between conversation and Work Panel;
- `440px` default panel width where the workbench supports it;
- compact lower launcher without an enclosing card;
- no default highlighted launcher row;
- object content fills the panel below one compact object bar;
- narrow-window fallback remains flush to the workbench edges;
- existing user-adjusted theme and color treatment is unchanged.

### Interaction Acceptance

1. First open shows the five-action titleless launcher.
2. The Work Panel pushes the conversation aside at ordinary desktop widths.
3. The divider resizes the panel and the task remembers the explicit width.
4. `审阅` and `终端` open directly.
5. `预览网页`, `打开文件`, and `侧边任务` replace the launcher with an in-panel chooser.
6. A completed choice opens or focuses one concrete object tab.
7. The same object does not create duplicate tabs or review versions.
8. Closing the final tab returns to the launcher.
9. Closing and reopening the Work Panel restores remaining task-scoped tabs and width.
10. Loading, unavailable, and retry states remain local to the active content area.
11. Keyboard navigation and focus restoration work without a default launcher highlight.
12. Terminal remains temporary and exposes no internal execution history.

### Regression Coverage

- Extend desktop acceptance coverage for the embedded split, default width, resize persistence, titleless launcher, chooser replacement, and content fill.
- Keep existing tab-state, restoration, adapter, and accessibility tests green.
- Verify the representative normal, expanded, and narrow viewport states.
- Run the desktop build and the repository acceptance dry-run before handoff.

## Documentation Impact

Implementation of this user-visible change must keep these surfaces aligned:

- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `apps/desktop/e2e/acceptance.spec.ts`
- `scripts/acceptance.sh --dry-run` descriptions when advertised coverage changes

## Out Of Scope

- changing the current theme, palette, typography direction, or design tokens;
- backend memory, experience, continuity, Wiki, or recall changes;
- artifact version history or multi-version comparison;
- multiple named terminal sessions;
- a full embedded code editor;
- arbitrary IDE docking and split layouts;
- automatic preview or file opening;
- user-facing project experience or memory management.

## Approved Decisions

- Structure: native embedded split that pushes the conversation aside.
- Outer frame: none; one divider only.
- Default width: `440px`, clamped only when the workbench cannot preserve the documented split bounds.
- Resize: draggable and remembered per task.
- Empty state: titleless compact five-row launcher, `48px` above the bottom edge.
- Action transition: launcher exits and the same panel handles selection and content.
- Tabs: concrete objects, no categories, no duplicate object or review-version tabs.
- Terminal: temporary verification only.
- Responsive fallback: flush to the workbench, never a floating framed card.
- Theme and color treatment: preserved, not part of this redesign.
