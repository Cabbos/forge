# Forge Work Panel Quiet Native Redesign

Date: 2026-07-18
Status: approved in design review
Scope: Visual and interaction redesign of the desktop Work Panel
Supersedes: the panel-frame, launcher, tab-chrome, and visual-direction sections of `2026-07-17-forge-work-panel-design.md`

## Goal

Redesign the existing Work Panel so it feels like a mature native desktop work surface rather than a collection of product cards. The panel should borrow the restraint of Codex without copying its dark theme: one adaptive surface, clear object tabs, quiet system colors, and no user-facing exposure of memory or experience internals.

The functional model from the original Work Panel specification remains intact:

- previews, files, review, terminal, and subtasks open only after explicit user action;
- tabs represent concrete objects rather than product categories;
- the panel restores the current task's latest working state;
- terminal remains a temporary verification convenience;
- internal experience, memory, and recall systems remain invisible here.

## Design Read

The product is a high-frequency AI desktop tool for users who need to inspect current work without leaving the conversation. The desired character is native, calm, precise, and content-first.

Design dials:

- Design variance: 5/10
- Motion intensity: 3/10
- Visual density: 5/10

Selected visual direction: **Quiet Native**.

Rejected directions:

- **Warm Forge** was too close to a content product and would preserve the current beige, card-like character.
- **Precision Tool** was too hard and developer-tool-specific, and would split the Work Panel visually from the rest of Forge.

## Product Principles

1. **One surface, one object.** The Work Panel has one outer boundary. Opened content fills that surface instead of being nested inside another preview card.
2. **Reveal chrome only when it is useful.** The empty launcher has no `工作面板` title bar. The object bar appears only when content is open.
3. **State comes from tone, not weight.** Hover and active states move through adjacent surface colors. They do not use a solid black selection block, accent gradient, or heavy shadow.
4. **Preserve the conversation.** The panel is a resizable adjacent workspace, not a modal overlay on ordinary desktop widths.
5. **Open objects, not classifications.** The `+` action asks what the user wants to open. It never asks the user to choose a tab category first.
6. **System theme is a complete design input.** Light and dark themes are each intentionally composed; dark mode is not a color inversion.

## State Flow

The Work Panel has four visible states.

### 1. Closed

The conversation uses the full workbench. The existing Work Panel trigger remains available in the window chrome. Closing the panel does not destroy its task-scoped tabs.

### 2. Empty Launcher

The first open for a task, or closing the final content tab, shows the launcher.

The launcher:

- has no title, description, search field, project summary, status card, or memory row;
- places utility controls in the top-right corner;
- centers five full-width action rows in this order: `审阅`, `终端`, `预览网页`, `打开文件`, `侧边任务`;
- uses low-contrast filled rows with no border and no individual card shadow;
- may show a keyboard hint on the right when a stable shortcut exists.

The launcher is an empty state, not a persistent `启动页` tab.

### 3. Object Open

Selecting a launcher result replaces the launcher with the selected object. Review, web previews, files, terminal, and subtasks all use the same object-tab model.

The content fills the Adaptive Sheet interior. A web preview does not sit inside an additional rounded preview card. Object-specific controls are integrated into the object bar or a minimal context row.

### 4. New-Object Chooser

Clicking `+` opens a temporary chooser titled `打开新的…`. The chooser presents available object types and relevant recent/discovered targets. Selecting a target creates or focuses its object tab. Cancelling returns to the previously active object.

It does not ask for a tab classification, create an empty permanent tab, or automatically select a newly discovered artifact.

## Return And Restore Rules

- Closing the final object tab returns to the empty launcher.
- Closing the panel returns focus to the conversation and preserves existing tabs.
- Reopening the panel restores the last active tab when valid; otherwise it shows the launcher.
- Switching tasks restores that task's independent tab state and last width.
- An invalid restored object remains a concise unavailable tab with retry or close actions; it does not reset the whole panel.

## Spatial Model

The panel opens from the right as a floating sheet within the application workbench. It is visually elevated from the application canvas but participates in layout rather than covering the conversation at ordinary desktop widths.

### Width

- Default: 40% of the available workbench.
- Normal resize range: 34% to 62%.
- Absolute minimum: 360px.
- Absolute maximum: 920px.
- The last explicit width is remembered per task.
- Double-clicking the resize boundary returns to 40%.
- The resize handle receives a keyboard-accessible alternative.

### Responsive Behavior

- At widths below 900px, the panel holds at 360px instead of continuing to scale proportionally.
- Below 720px, the panel becomes a full-height temporary overlay with an explicit return control.
- The responsive overlay is a fallback for insufficient space, not the normal desktop presentation.

### Outer Shape

- Outer sheet radius: 12px.
- Launcher row and selected-tab radius: 8px.
- A single restrained shadow separates the sheet from the workbench.
- No internal content adapter adds a second outer shadow or rounded container.

## Object Bar

The selected design uses a fused object bar instead of a tab row plus a full secondary toolbar.

- Tabs, close affordances, `+`, overflow, and primary object actions share one compact horizontal band.
- The active tab uses a neighboring surface tone, not an underline plus filled pill plus bold type.
- A URL or file path may appear in one shallow context row when it materially helps orientation.
- Web actions such as refresh and external open remain at the right edge.
- File and review adapters substitute only their relevant actions; they do not introduce a new global header.
- Tab names remain object names such as `localhost:1420`, `README.md`, or `审阅 · 8 个文件`.

### Overflow

- Tabs scroll horizontally when needed.
- The right side always preserves an overflow menu and `+` action.
- Labels truncate only after a useful readable width and never collapse into unidentified icons.
- Reopening an already-open object focuses its existing tab.

## Visual System

### Light Theme

- Workbench canvas: cool neutral gray, visually behind the panel.
- Sheet: warm near-white rather than pure white.
- Launcher rows and inactive controls: low-contrast neutral fill.
- Hover and active: one and two tonal steps stronger than rest.
- Primary text: near-black charcoal.
- Secondary text and icons: middle neutral gray with accessible contrast.

### Dark Theme

- Workbench canvas: deep charcoal, not pure black.
- Sheet: a distinct elevated charcoal layer.
- Launcher rows: a quiet lighter charcoal.
- Hover and active: tonal lightening rather than a white outline or saturated accent.
- Primary text: soft off-white.
- Secondary text and icons: restrained warm gray.

### Typography

- Use the desktop system UI font for labels and navigation.
- Base interface text: 13px.
- Secondary context and shortcut text: 12px where space allows.
- Monospace is reserved for commands, terminal output, paths, URLs, and code.
- Selection does not rely on large weight changes.

### Iconography

- Use the existing icon family consistently at a shared optical size.
- Icons are functional markers, not illustrated badges.
- Do not mix emoji, boxed icons, filled badges, and line icons in the same launcher.

### Prohibited Treatments

- no gradient decoration;
- no glassmorphism or backdrop-blur panel styling;
- no bordered card wall;
- no solid black selected launcher row;
- no large title or explanatory subtitle inside the launcher;
- no decorative status pills where plain text is sufficient;
- no user-facing `经验`, `记忆`, or internal recall labels.

## Interaction And Motion

- Panel enter/exit: 160–180ms opacity and horizontal translation.
- Hover and active tone changes: 120–140ms.
- Tab content change: short cross-fade only when it does not delay interaction.
- Resizing: immediate, with no easing or spring behavior.
- No scale-on-hover, bouncing controls, or ambient animation.
- `prefers-reduced-motion` removes translation and nonessential fades.

Keyboard behavior:

- launcher actions and object tabs are reachable in a logical sequence;
- the tab strip uses standard roving focus and exposes selected state;
- `Escape` closes only the temporary new-object chooser or another local transient surface;
- closing the panel returns focus to the Work Panel trigger;
- focus can move explicitly between the conversation and the panel without cycling through hidden controls.

## Content Adapter Constraints

The redesign changes presentation, not authority or scope.

- **Review:** shows the current change set only and sends selected feedback into the active conversation.
- **Terminal:** remains one temporary task-scoped command surface for ad hoc verification. It does not show Forge's internal terminal history.
- **Preview:** opens only after explicit selection and shows the latest result rather than artifact versions.
- **Files:** remains read-oriented and does not become a general editor.
- **Subtasks:** shows the selected subtask's current useful state without exposing orchestration internals.

Each adapter must render against the shared sheet and object-bar contract. Adapter failures stay local to their tab.

## Accessibility

- All interactive targets have accessible names and visible focus treatment.
- Target sizes remain at least 32px in dense chrome and 40px for launcher rows.
- Color is not the only indicator for selected, unavailable, running, or failed states.
- Light and dark theme text and controls meet WCAG AA contrast.
- Resize, maximize, close, overflow, and external-open controls are keyboard operable.

## Implementation Boundaries

The existing Work Panel state and adapter architecture should remain unless implementation discovery finds a concrete blocker. The redesign should be expressed primarily through:

- the panel shell and resize layout;
- the empty launcher;
- the fused object bar and overflow behavior;
- shared Work Panel design tokens;
- adapter chrome cleanup;
- responsive and theme styles.

The implementation must not reintroduce Project Archive UI or expose experience/memory management. It must preserve current backend authority checks and task-scoped restoration.

Before editing any indexed symbol, run GitNexus upstream impact analysis and report the risk. Update user-visible documentation and desktop acceptance coverage as required by the repository instructions.

## Verification

### Visual Acceptance

Verify in both system themes at representative panel widths:

- 360px minimum;
- 40% default on a standard desktop window;
- 62% expanded;
- responsive overlay below 720px.

Capture the empty launcher, one web preview, one file, review, temporary terminal, and new-object chooser. Confirm that there is never more than one outer content boundary.

### Interaction Acceptance

1. First open shows the titleless launcher.
2. No preview or file opens without explicit user selection.
3. `+` asks what to open and does not ask for a tab category.
4. Selecting an existing object focuses its tab instead of duplicating it.
5. Closing the final tab returns to the launcher.
6. Closing and reopening restores remaining task-scoped tabs and width.
7. Resize, overflow, keyboard focus, and responsive fallback work at their boundaries.
8. Theme changes update the complete visual system without losing contrast.
9. Terminal remains a temporary command view and never exposes Forge execution logs.
10. `Project Archive`, `经验`, and internal memory-management UI remain absent.

### Regression Coverage

- pure tab-state tests remain green;
- component and accessibility tests cover the titleless launcher and object bar;
- desktop acceptance covers explicit preview opening, restoration, and responsive layout;
- desktop build, TypeScript checks, Rust tests, clippy, and acceptance dry-run remain green.

## Documentation Impact

Because this is a user-visible runtime change, implementation must keep these surfaces aligned:

- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `apps/desktop/e2e/acceptance.spec.ts`
- `scripts/acceptance.sh --dry-run` descriptions when coverage changes

## Out Of Scope

- changing backend memory, continuity, Wiki, or recall systems;
- artifact version history or multi-version comparison;
- multiple named terminal sessions;
- a full embedded code editor;
- arbitrary IDE docking and split layouts;
- automatic preview opening or active-tab switching;
- a new subtask scheduler or orchestration authority.

## Approved Decisions

- Theme: complete light and dark designs following the system.
- Surface: Floating Sheet with Adaptive Sheet content behavior.
- Content chrome: fused object bar.
- Width: resizable work area, default 40%, range 34%–62%.
- Empty state: titleless launcher.
- Launcher rows: quiet tonal fills with no borders or individual shadows.
- Visual direction: Quiet Native.
- State flow, responsive behavior, and visual system: approved in review on 2026-07-18.
