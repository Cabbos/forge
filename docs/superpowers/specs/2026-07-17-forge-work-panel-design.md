# Forge Work Panel Design

Date: 2026-07-17
Status: pending user review
Scope: Replace the desktop Project Archive side panel with a user-directed, preview-capable work panel

## Goal

Forge should replace the current user-facing Project Archive panel with a resizable work panel for inspecting and operating on the current task's outputs. The panel should help a user preview an artifact, review the current change set, run a temporary verification command, browse a file, or inspect a subtask without exposing internal memory architecture.

The completion target is:

> Opening the work panel presents a calm launcher. The user chooses what to open, each choice becomes a task-scoped dynamic tab, and closing and reopening the panel restores the previous working state.

This design changes the desktop presentation and interaction model. It does not remove continuity, unified memory, Forge Wiki, recall, or other backend context capabilities.

## Product Principles

1. **Show user work, not internal machinery.** Experience distillation, recall, continuity, and memory remain implementation details. Users should feel their work continues accurately without managing those systems from the work panel.
2. **Never take the user's focus.** Forge may signal that new output exists, but it does not automatically create a tab, switch tabs, or open a preview.
3. **Open objects, not categories.** Tabs represent concrete things such as `localhost:1420`, `README.md`, `审阅 · 8 个文件`, a temporary terminal, or a named subtask. The panel has no permanent category tabs.
4. **Keep the latest result primary.** Preview shows the current result. Review shows the current change set. The panel does not add version trees, historical artifact pickers, or stale-comment management.
5. **Stay lighter than an IDE.** The work panel supports inspection and targeted action. It does not become a full code editor, terminal manager, or general window-layout system.

## Selected Direction

The selected direction is a unified, resizable work panel with a launcher and dynamic tabs.

Two alternatives were rejected:

- A fixed five-tab navigation model was rejected because it makes the panel feel like a product settings area and leaves irrelevant categories permanently visible.
- A freely dockable IDE model was rejected because split panes, movable regions, and layout persistence add complexity without improving the core inspect-and-respond workflow.

## Panel Frame

The panel opens from the right and shares the application window with the conversation surface. It does not use a modal backdrop.

- Default width is approximately 45 percent of the application window.
- A drag handle lets the user resize the panel within sensible minimum and maximum bounds.
- A maximize control expands the panel to the available workbench width; invoking it again returns to the previous width.
- A close control hides the panel without destroying its task-scoped tabs.
- The panel header identifies the surface as `工作面板` and shows the active task name when space permits.
- The title-bar entry that currently opens Project Archive becomes `工作面板`.

Panel width may be remembered globally. Open tabs, active tab, object identity, and view state are remembered per task.

## First Open And Launcher

The first time a task opens the work panel, Forge shows a launcher rather than selecting content automatically. The launcher follows the restrained structure of the Codex side-panel reference while retaining Forge's light visual language.

The launcher contains five large action rows in this order:

1. 审阅
2. 终端
3. 预览
4. 文件
5. 子任务

Each row has an icon and short label on the left and an optional keyboard shortcut on the right. It does not contain explanatory copy, project summaries, status cards, memory rows, or experience records.

Selecting an action opens a focused second step when an object is required:

- Review opens the current change set.
- Terminal creates or focuses the task's temporary terminal.
- Preview lets the user choose a discovered artifact, a previewable file, or a local URL.
- Files lets the user search or browse and then choose a file.
- Subtasks lets the user choose an active or retained subtask.

The launcher is searchable. Search spans actions and concrete available objects, including files, artifacts, local preview targets, and subtasks.

## Dynamic Tabs

The tab strip contains only objects the user has chosen to open. A fixed `+` control creates one transient launcher slot. Choosing an object replaces that slot with the resulting content tab; closing or cancelling it removes the slot. If a launcher slot is already active, `+` focuses it instead of creating another.

Tab labels describe their actual content:

- `localhost:1420`
- `README.md`
- `审阅 · 8 个文件`
- `终端`
- `子任务 · 设置诊断`

Rules:

- Opening an object that is already open focuses its existing tab instead of creating a duplicate.
- The first-open launcher is an empty-panel state and does not show a meaningless `启动页` tab label.
- Closing the active tab focuses the nearest remaining tab.
- Closing the final content tab returns the panel to the launcher.
- Closing and reopening the work panel restores the current task's tabs and active tab.
- Switching tasks restores that task's independent tab set.
- A newly discovered artifact, changed diff, or updated subtask may add a subtle unread marker to a related tab or launcher result, but it never changes the active tab.
- If a restored object no longer exists, its tab remains visible with a concise unavailable state and actions to retry or close it.

## Preview

Preview is always user-initiated. Forge does not automatically open a detected web server, image, document, or file.

The preview flow is:

1. The user chooses `预览` from the launcher.
2. Forge presents available artifacts, previewable files, and a local-address input.
3. The user chooses one target.
4. Forge creates a tab named after that target.

The preview uses an adaptive toolbar:

- Web content: address, refresh, desktop/tablet/mobile viewport, external open, and panel maximize.
- Images: fit, actual size, zoom, and save/open externally where supported.
- PDF and document content: page position, zoom, outline where available, and export/open externally.
- Code and text: syntax highlighting, line numbers, copy, and reveal in Files.

Preview renders only the current result. It does not expose artifact version history. If a target has no renderer, Forge offers a safe external-open or Files fallback instead of displaying raw unsupported bytes.

## Review

Review represents the current task's current change set.

- A file list groups added, modified, renamed, and deleted files.
- The diff view collapses unchanged regions and supports per-file navigation.
- Selecting a changed row or range opens a lightweight feedback composer.
- Sending feedback immediately adds the file path, line context, and user note to the active conversation as the next-turn instruction.
- Forge does not maintain a multi-comment draft queue, review versions, approval workflow, or stale-line comment migration.
- When the task changes the worktree, Review updates to the current diff. Previously sent feedback remains part of the conversation, not an anchored review object.
- The first implementation handles code and text diffs. Non-text artifacts receive a concise change summary and an appropriate preview link rather than pixel-level comparison.

## Temporary Terminal

Terminal is a convenience for ad hoc user verification, not a record of Forge's execution history.

- The terminal is created lazily when the user chooses `终端`.
- A single temporary terminal is retained for the current task while the task is active.
- Switching tabs preserves its process and screen state.
- The surface supports ordinary terminal input plus clear, restart, and close.
- It does not show Forge tool logs, identify command ownership, manage multiple named sessions, or provide a separate log browser.
- Closing the task releases the terminal process. An exited process shows a direct restart action and does not restart automatically.

## Files

Files is a read-oriented project browser.

- It supports file-tree navigation, filename search, syntax highlighting, line numbers, copy path, and reveal/open actions.
- At wide panel sizes it may show tree and content together. At narrow sizes it uses a drill-in flow so content remains readable.
- A previewable file can be opened as its own preview tab.
- The first implementation does not embed a general code editor. Manual editing continues in the user's editor or through the conversation with Forge.

## Subtasks

Subtasks exposes user-relevant A2A work without exposing orchestration internals.

- The selection view groups running, waiting for review, completed, and failed subtasks.
- A subtask tab shows its goal, current status, latest progress, output, and relevant process evidence.
- The user can add an instruction or take over when the runtime supports it.
- The panel reuses the existing task authority and review gates. It does not invent a second subtask scheduler or creation model.

## Hidden Internal Context

The following concepts no longer render in the work panel:

- continuity experiences and experience search
- unified memory management
- saved background and memory candidates
- project archive and project-record management
- internal recall confidence or storage-path details

Their backend behavior remains intact. Existing recall, archive, forget, Wiki writeback, and continuity APIs may remain accessible through other dedicated settings or future administrative surfaces when genuinely needed, but they are not part of the work panel's primary user model.

## Visual Direction

The visual thesis is: **a calm, light workbench canvas that reveals only the object the user chose to inspect.**

- Preserve Forge's light palette, typography, design tokens, and restrained motion.
- Use one continuous panel surface instead of a stack of dashboard cards.
- Place the launcher actions near the visual center, slightly below the midpoint, with generous surrounding whitespace.
- Use background levels and fine dividers for structure; avoid thick outlines, ornamental gradients, and decorative status pills.
- Keep the dynamic tab strip compact and allow horizontal overflow without shrinking labels into unreadable fragments.
- Use short fade-and-translate transitions for panel entry, tab creation, and content switching.
- Respect reduced-motion preferences and keep resize behavior immediate and stable.

## State Model

Each open content tab uses a discriminated identity:

```ts
type WorkPanelTab =
  | { kind: "launcher"; id: string }
  | { kind: "review"; id: string; taskId: string }
  | { kind: "terminal"; id: string; taskId: string }
  | { kind: "preview"; id: string; target: PreviewTarget }
  | { kind: "file"; id: string; path: string }
  | { kind: "subtask"; id: string; taskId: string; subtaskId: string };
```

The exact implementation type may differ, but identity must be stable enough to deduplicate an object and restore it. Serializable presentation state is stored per task. Live resources such as terminal processes and webviews are reconstructed or reported unavailable rather than serialized.

## Architecture Boundaries

The implementation should separate frame state from content adapters:

- `WorkPanelHost`: open/close lifecycle, active task binding, and lazy loading.
- `WorkPanelShell`: resize, maximize, header, tab strip, and active content outlet.
- `WorkPanelLauncher`: action rows, search, and object selection.
- A pure tab-state module: create, focus, close, deduplicate, persist, and restore transitions.
- Content adapters for review, terminal, preview, files, and subtasks.

Existing capabilities should be reused through their current authority boundaries:

- project runtime and preview ownership data for preview targets
- current Git/worktree diff data for Review
- workspace file IPC for Files
- task-scoped PTY or shell authority for Terminal
- A2A runtime projection and review gates for Subtasks

The React surface must not infer permission to read, execute, or control a subtask. It requests those actions through the existing backend authority.

## Failure Handling

Failures are isolated to one tab.

- Preview load failure keeps the target visible and offers retry, external open, or close.
- A removed file shows that the path is no longer available and offers a return to Files.
- Review with no current changes shows an empty current-change state, not an old diff.
- Terminal exit shows its exit status and a restart action.
- A missing or expired subtask shows the last safe retained summary and disables unavailable actions.
- Failure in one adapter never closes the panel or resets other tabs.
- Corrupt persisted tab state is discarded for the affected task and falls back to the launcher.

## Accessibility And Shortcuts

- Every launcher row and tab is keyboard reachable.
- The tab strip follows standard roving-focus behavior and exposes active/selected state to assistive technology.
- Resize and maximize have labelled button alternatives; drag resize is not the only way to change panel size.
- Shortcut labels are hints, not required interaction paths.
- Focus returns to the title-bar work-panel trigger when the panel closes.
- Color is never the only signal for unread, running, failed, or unavailable states.

## Verification Strategy

### Pure State Tests

Cover:

- launcher initialization
- object identity and deduplication
- create, focus, close, and nearest-tab selection
- final-tab fallback to launcher
- per-task state separation
- restore of valid tabs and rejection of corrupt state
- unavailable restored targets
- no automatic tab creation on artifact discovery

### Component Tests

Cover:

- launcher action order and search
- user-driven preview selection
- adaptive preview toolbar selection
- review row feedback payload
- temporary terminal lifecycle
- narrow and wide Files layouts
- subtask status and action availability
- keyboard tab navigation and accessible names

### Product-Level Acceptance

Extend the desktop acceptance coverage to prove:

1. Opening the work panel for a task with no panel state shows the launcher.
2. No preview opens without user selection, even when a preview target exists.
3. Choosing an action and object creates a correctly named dynamic tab.
4. Choosing the same object again focuses the existing tab.
5. `+` creates a launcher tab.
6. Closing and reopening restores task-scoped tabs.
7. Width resize and maximize do not obscure required controls.
8. Review feedback enters the active conversation with file and line context.
9. Terminal is user-created and does not expose Forge execution logs.
10. Project experience, continuity, unified memory, and Project Archive labels are absent from the work panel.
11. Existing A2A review, preview ownership, and file-boundary behavior remain intact.

## Documentation Impact

Because this is a user-visible runtime surface, implementation must update:

- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `apps/desktop/e2e/acceptance.spec.ts`
- `scripts/acceptance.sh --dry-run` descriptions when advertised coverage changes

The implementation must also remove or revise documentation that describes Project Archive as the primary side-panel surface, while preserving accurate descriptions of backend memory and continuity behavior.

## Out Of Scope

- Removing continuity, unified memory, Forge Wiki, or recall backend capabilities
- Artifact version history or version comparison
- A full code editor inside Files
- Multiple managed terminal sessions or a Forge command-log viewer
- Freely dockable panes or arbitrary split layouts
- Automatic preview opening or active-tab switching
- Pixel-level image comparison or rich document redlining in Review
- A new subtask creation or scheduling authority

## Acceptance Decision

The design is approved when the user agrees that:

- the first-open experience is the launcher;
- every preview and tool surface is opened explicitly;
- content appears in dynamic object-named tabs;
- task state restores after closing the panel;
- internal experience and memory concepts are removed from this user-facing surface; and
- the work panel remains an inspection and targeted-action surface rather than becoming an IDE.
