# Forge Frontend Maintainability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` or `executing-plans` to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Keep Forge's V1 desktop UI polish moving toward Claude Desktop-level quality while making the frontend easier to extend, test, and safely refactor.

**Architecture:** Treat UI polish as a set of narrow, reversible slices. Each slice preserves the current product surface, adds characterization coverage first, extracts one responsibility, and verifies adjacent visual behavior before moving on. Product concepts, event schemas, IPC contracts, and backend protocols remain outside this plan.

**Tech Stack:** React, TypeScript, Tauri 2, Zustand, Vite, Tailwind-style utility classes, CSS tokens in `src/styles/globals.css`, Playwright e2e coverage.

---

## Scope

This plan is for long-running frontend quality maintenance. It is not a V2 product roadmap.

Allowed work:

- Split large frontend files by responsibility.
- Consolidate visual tokens, component classes, renderer helpers, and test fixtures.
- Improve local UI implementation quality for composer, message lane, process feedback, Markdown, shell/tool evidence, titlebar, sidebar, and Project Archive.
- Update product and engineering docs in `docs/product/`.
- Add or reorganize Playwright tests.

Forbidden without main owner confirmation:

- New user-visible entries, modules, panels, product terms, workflows, or abilities.
- Rust `StreamEvent` changes.
- TypeScript protocol schema changes in `src/lib/protocol.ts`.
- New IPC commands or backend event contracts.
- New model response schemas.
- New testing framework or component library.

## Current Baseline

The frontend has improved visually, but several files now carry too much responsibility:

| Area | Current File | Current Size | Risk |
| --- | --- | ---: | --- |
| Global style system | `src/styles/globals.css` | 3376 lines | Tokens, layout, Markdown, composer, archive, and process surfaces are coupled. |
| Composer | `src/components/session/InputBar.tsx` | 512 lines | Input state, chips, model menu, suggestions, resume state, and send logic live together. |
| Project Archive | `src/components/layout/HubPanel.tsx` | 543 lines | Panel shell, archive disclosure, context file rendering, MCP source mapping, and runtime loading are mixed. |
| Message grouping | `src/components/chat/MessageList.tsx` | 169 lines | Recently improved by extracting `messageGrouping.ts`; keep this as the preferred pattern. |
| Markdown file refs | `src/components/messages/TextBlock.tsx` | 242 lines | Recently improved by extracting `markdownFileRefs.tsx`; continue extracting renderer helpers this way. |
| E2E coverage | `e2e/frontend.spec.ts` | 8005 lines | Many domains are bundled into one spec, which makes maintenance and targeted review harder. |

The priority is not to make files small for its own sake. The priority is to make each future visual slice obvious, testable, and low-risk.

## Operating Principles

1. Preserve behavior before polishing.
   Every refactor starts with characterization coverage for the current behavior.

2. One responsibility per slice.
   A slice should touch one domain: composer, process feedback, Markdown, archive, CSS tokens, or test infrastructure.

3. No product expansion.
   If the fix requires new data, new events, new navigation, or new visible terminology, stop and ask the main owner.

4. Visual changes and structural refactors do not mix by default.
   Extract first, verify parity, then polish in a follow-up slice.

5. Existing successful extractions are the model.
   `src/components/chat/messageGrouping.ts` and `src/components/messages/markdownFileRefs.tsx` are the preferred style: pure helpers, explicit types, compact tests, and no protocol changes.

6. Dirty worktree discipline.
   Only stage files from the current slice. Never revert unrelated changes.

## Target Shape

### CSS Modules By Responsibility

Keep `src/styles/globals.css` as the import coordinator or compatibility entry, then migrate domain rules into focused files:

| Target File | Responsibility |
| --- | --- |
| `src/styles/tokens.css` | Forge color, border, shadow, motion, density, and z-index tokens. |
| `src/styles/layout.css` | App shell, titlebar, sidebar, conversation lane, scrollbars, reduced motion. |
| `src/styles/composer.css` | Composer root, textarea, chip tray, toolbar, model menu, suggestion menu. |
| `src/styles/messages.css` | User/assistant messages, turn rhythm, copy action, delivery summary, confirmation surfaces. |
| `src/styles/process.css` | Thinking rows, tool activity groups, shell cards, pending rows, detail surfaces. |
| `src/styles/markdown.css` | Paragraphs, headings, lists, tables, inline code, code blocks, diagrams, links, file refs. |
| `src/styles/archive.css` | Project Archive panel, disclosure groups, context materials, empty states. |

Acceptance:

- Import order is explicit and stable.
- A visual token can be found in one place.
- Domain CSS files can be reviewed independently.
- `src/styles/globals.css` stops being the only place to understand the UI.

### Composer Decomposition

Keep `InputBar` as the shell and split responsibility into small units:

| Target File | Responsibility |
| --- | --- |
| `src/components/session/InputBar.tsx` | Orchestrates composer state and renders the composed input surface. |
| `src/components/session/composerTypes.ts` | `Chip`, menu mode, and small shared composer types. |
| `src/components/session/composerCommands.ts` | Existing slash command list and command metadata. |
| `src/components/session/ComposerChipTray.tsx` | File and command chips, removal, overflow behavior. |
| `src/components/session/ComposerSuggestionMenu.tsx` | Existing `/` and `@` suggestion menu rendering and keyboard selected state. |
| `src/components/session/ComposerModelMenu.tsx` | Existing provider/model menu rendering. |
| `src/components/session/ComposerToolbar.tsx` | Existing `@`, `/`, stop/send/resume controls and low-noise status. |
| `src/components/session/useComposerInput.ts` | Textarea value, auto-height, IME composition, pending input insertion. |
| `src/components/session/useComposerSuggestions.ts` | Trigger detection, file search, menu dismissal, active option. |

Acceptance:

- `InputBar.tsx` drops below 280 lines.
- Existing `/`, `@`, model menu, file chips, send, stop, and resume behavior stays intact.
- Composer tests remain green before and after extraction.
- No new command, menu entry, or visible product term appears.

### Message And Process Renderer Decomposition

Keep renderers local to `src/components/messages/`, but continue moving pure logic out of components:

| Target File | Responsibility |
| --- | --- |
| `src/components/messages/markdownFileRefs.tsx` | Existing file reference parsing and rendering. |
| `src/components/messages/markdownTransforms.ts` | Pure Markdown preprocessing helpers that do not render React. |
| `src/components/messages/processLabels.ts` | Tool/shell status labels, durations, compact summaries. |
| `src/components/messages/processEvidence.ts` | Promotion rules for quiet rows versus structured evidence. |
| `src/components/messages/messageSurfaces.ts` | Shared class-name helpers for message, detail, audit, and failure surfaces. |

Acceptance:

- Render components remain readable at a glance.
- Promotion rules for failure, confirmation, long output, and audit evidence are testable as pure logic.
- Routine thinking/tool/shell rows stay quiet; failures and decisions stay inspectable.
- No `StreamEvent` or protocol shape changes.

### Project Archive Decomposition

Keep the current Project Archive surface and visual role. Do not add archive abilities.

| Target File | Responsibility |
| --- | --- |
| `src/components/layout/HubPanel.tsx` | Opens/closes the inspector and wires active session/project state. |
| `src/components/layout/archive/ArchiveDisclosure.tsx` | Existing disclosure section shell. |
| `src/components/layout/archive/ArchiveContextMaterials.tsx` | Existing context file/material list rendering. |
| `src/components/layout/archive/archiveContextMaterials.ts` | Pure mapping from MCP resources/prompts/status to context material rows. |
| `src/components/layout/archive/ArchiveLayerHeader.tsx` | Existing section header treatment. |

Acceptance:

- `HubPanel.tsx` drops below 320 lines.
- Existing keyboard close/open behavior remains.
- Existing Project Archive visual density remains aligned with titlebar/sidebar/composer.
- Information architecture concerns are documented, not implemented.

### Test Structure

Split the single large Playwright file into domain specs without changing test intent:

| Target File | Responsibility |
| --- | --- |
| `e2e/fixtures/app.ts` | Shared app setup, mock sessions, route helpers, and viewport helpers. |
| `e2e/fixtures/sessionBuilders.ts` | Reusable stream block/session builders. |
| `e2e/composer.spec.ts` | Input, chips, suggestions, model menu, send/stop/resume, short windows. |
| `e2e/messages.spec.ts` | Message rhythm, copy action, Markdown, code, table, diagram, file refs. |
| `e2e/process.spec.ts` | Thinking, tool, shell, pending, confirm, diff, failure, expansion. |
| `e2e/archive.spec.ts` | Project Archive, right-side density, context materials, empty states. |
| `e2e/chrome.spec.ts` | Titlebar, sidebar, scroll-to-bottom, reduced motion, desktop frame rhythm. |
| `e2e/frontend.spec.ts` | Compatibility shell during migration; eventually only cross-domain smoke tests. |

Acceptance:

- No domain spec exceeds 2200 lines.
- Existing test names remain searchable during migration.
- Fixtures do not depend on the absolute Forge repo path.
- Targeted runs become faster to reason about than one giant spec.

## Execution Phases

### Phase 0: Guardrail Baseline

**Files:**

- Modify: `docs/product/forge-frontend-maintainability-plan.md`
- Inspect: `src/styles/globals.css`
- Inspect: `src/components/session/InputBar.tsx`
- Inspect: `src/components/layout/HubPanel.tsx`
- Inspect: `e2e/frontend.spec.ts`

- [ ] Step 1: Record current line counts.

Run:

```bash
wc -l src/styles/globals.css src/components/session/InputBar.tsx src/components/layout/HubPanel.tsx src/components/chat/MessageList.tsx src/components/messages/TextBlock.tsx e2e/frontend.spec.ts
```

Expected:

```text
src/styles/globals.css is the largest style risk.
src/components/session/InputBar.tsx and src/components/layout/HubPanel.tsx are the largest component risks.
e2e/frontend.spec.ts is the largest test maintenance risk.
```

- [ ] Step 2: Confirm no schema work is needed.

Run:

```bash
git diff -- src-tauri/src/protocol/events.rs src/lib/protocol.ts src/lib/tauri.ts
```

Expected:

```text
No diff for protocol or IPC files in maintenance-only slices.
```

- [ ] Step 3: Run whitespace validation.

Run:

```bash
git diff --check
```

Expected: no output.

### Phase 1: CSS System Split

**Files:**

- Create: `src/styles/tokens.css`
- Create: `src/styles/layout.css`
- Create: `src/styles/composer.css`
- Create: `src/styles/messages.css`
- Create: `src/styles/process.css`
- Create: `src/styles/markdown.css`
- Create: `src/styles/archive.css`
- Modify: `src/styles/globals.css`
- Test: `e2e/chrome.spec.ts`
- Test: `e2e/composer.spec.ts`
- Test: `e2e/messages.spec.ts`

- [ ] Step 1: Add characterization checks for current visual tokens.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "desktop chrome shares the Forge material system|composer keeps dense references inside the input surface|assistant markdown uses a compact editorial rhythm"
```

Expected: pass before extraction.

- [ ] Step 2: Move only token definitions into `src/styles/tokens.css`.

Implementation rule:

```text
Move variables and token definitions first. Do not move component selectors in the same slice.
Keep selector output identical after import.
```

- [ ] Step 3: Import tokens from `src/styles/globals.css`.

Implementation rule:

```css
@import "./tokens.css";
```

The import must appear before rules that consume Forge tokens.

- [ ] Step 4: Verify token-only extraction.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "desktop chrome shares the Forge material system|composer keeps dense references inside the input surface|assistant markdown uses a compact editorial rhythm"
npm run build
git diff --check
```

Expected: all pass.

- [ ] Step 5: Repeat the same pattern for layout, composer, messages, process, Markdown, and archive selectors.

Order:

```text
layout.css -> composer.css -> messages.css -> process.css -> markdown.css -> archive.css
```

Each move gets its own characterization run and commit.

### Phase 2: Composer Extraction

**Files:**

- Create: `src/components/session/composerTypes.ts`
- Create: `src/components/session/composerCommands.ts`
- Create: `src/components/session/ComposerChipTray.tsx`
- Create: `src/components/session/ComposerSuggestionMenu.tsx`
- Create: `src/components/session/ComposerModelMenu.tsx`
- Create: `src/components/session/ComposerToolbar.tsx`
- Create: `src/components/session/useComposerInput.ts`
- Create: `src/components/session/useComposerSuggestions.ts`
- Modify: `src/components/session/InputBar.tsx`
- Test: `e2e/composer.spec.ts`

- [ ] Step 1: Capture current composer behavior before extraction.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "composer keeps dense references inside the input surface|composer floating menus stay bounded|stopped sessions keep resume controls reachable|composer handles long pasted text without breaking the lane"
```

Expected: pass before extraction.

- [ ] Step 2: Extract static command data.

Implementation rule:

```text
Move the existing COMMANDS array into src/components/session/composerCommands.ts.
Do not rename existing command text or descriptions.
```

- [ ] Step 3: Extract shared composer types.

Implementation rule:

```text
Move Chip and menu-mode type aliases into src/components/session/composerTypes.ts.
Keep exported names simple and local to composer.
```

- [ ] Step 4: Extract visual subcomponents one at a time.

Order:

```text
ComposerChipTray -> ComposerSuggestionMenu -> ComposerModelMenu -> ComposerToolbar
```

For each extraction:

```bash
npx playwright test e2e/frontend.spec.ts -g "composer keeps dense references inside the input surface|composer floating menus stay bounded"
npm run build
git diff --check
```

Expected: all pass.

- [ ] Step 5: Extract hooks after visual parity is stable.

Order:

```text
useComposerInput -> useComposerSuggestions
```

Expected result:

```text
InputBar.tsx remains the orchestration shell and drops below 280 lines.
```

### Phase 3: Process Feedback Extraction

**Files:**

- Create: `src/components/messages/processLabels.ts`
- Create: `src/components/messages/processEvidence.ts`
- Create: `src/components/messages/messageSurfaces.ts`
- Modify: `src/components/messages/ToolActivityGroup.tsx`
- Modify: `src/components/messages/ShellCard.tsx`
- Modify: `src/components/messages/ToolCallCard.tsx`
- Modify: `src/components/messages/ThinkingBlock.tsx`
- Modify: `src/components/messages/PendingBlock.tsx`
- Test: `e2e/process.spec.ts`

- [ ] Step 1: Capture process behavior before extraction.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "consecutive tool activity becomes one process evidence group|successful tool activity collapses into one handled summary|failed shell output uses inspectable evidence styling|expanded logs share one detail surface"
```

Expected: pass before extraction.

- [ ] Step 2: Extract labels and duration formatting.

Implementation rule:

```text
Move pure string/label formatting only.
Do not change rendered text in the same slice.
```

- [ ] Step 3: Extract promotion rules.

Implementation rule:

```text
processEvidence.ts decides whether content is quiet row, expandable detail, failure evidence, confirmation, or audit summary.
It must not import React.
```

- [ ] Step 4: Extract shared surface class helpers.

Implementation rule:

```text
messageSurfaces.ts may export class-name helpers.
It must not know about backend event types beyond existing frontend block shape.
```

- [ ] Step 5: Verify adjacent regressions.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "conversation turns keep quiet separation without card framing|message stream uses one gap token without component margins|first loop delivery summary can open the archive"
npm run build
git diff --check
```

Expected: all pass.

### Phase 4: Markdown Renderer Hardening

**Files:**

- Create: `src/components/messages/markdownTransforms.ts`
- Modify: `src/components/messages/TextBlock.tsx`
- Modify: `src/components/messages/CodeBlock.tsx`
- Modify: `src/components/messages/DiagramBlock.tsx`
- Modify: `src/styles/markdown.css`
- Test: `e2e/messages.spec.ts`

- [ ] Step 1: Capture Markdown containment before extraction.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "inline file references stay quiet and wrap within the message lane|assistant markdown uses a compact editorial rhythm|streaming markdown renders structure before the final chunk|ASCII architecture diagrams stay inside the message lane"
```

Expected: pass before extraction.

- [ ] Step 2: Move pure preprocessing from `TextBlock.tsx` into `markdownTransforms.ts`.

Implementation rule:

```text
markdownTransforms.ts handles text normalization only.
React rendering stays in TextBlock.tsx and markdownFileRefs.tsx.
```

- [ ] Step 3: Keep diagrams on the existing `DiagramBlock` path.

Implementation rule:

```text
Do not introduce a new chart entry, graph panel, diagram mode, or model schema.
Only improve rendering stability and containment for existing ASCII diagram handling.
```

- [ ] Step 4: Verify long content.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "wide markdown tables use internal overflow|code blocks do not force the conversation lane wider|user messages can carry pasted code paths and logs without breaking the lane"
npm run build
git diff --check
```

Expected: all pass.

### Phase 5: Project Archive Extraction

**Files:**

- Create: `src/components/layout/archive/ArchiveDisclosure.tsx`
- Create: `src/components/layout/archive/ArchiveContextMaterials.tsx`
- Create: `src/components/layout/archive/archiveContextMaterials.ts`
- Create: `src/components/layout/archive/ArchiveLayerHeader.tsx`
- Modify: `src/components/layout/HubPanel.tsx`
- Modify: `src/styles/archive.css`
- Test: `e2e/archive.spec.ts`

- [ ] Step 1: Capture current archive behavior.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "project archive shares the Forge material system|first loop delivery summary can open the archive|archive context materials keep dense local project rhythm"
```

Expected: pass before extraction.

- [ ] Step 2: Extract visual shell components.

Order:

```text
ArchiveLayerHeader -> ArchiveDisclosure -> ArchiveContextMaterials
```

- [ ] Step 3: Extract pure material mapping.

Implementation rule:

```text
archiveContextMaterials.ts maps existing MCP resources, prompts, and status into row view models.
It does not add archive concepts or new context types.
```

- [ ] Step 4: Verify shell and archive adjacency.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "titlebar keeps project metadata readable|sidebar keeps active row inset calm|project archive shares the Forge material system"
npm run build
git diff --check
```

Expected: all pass.

### Phase 6: Playwright Spec Split

**Files:**

- Create: `e2e/fixtures/app.ts`
- Create: `e2e/fixtures/sessionBuilders.ts`
- Create: `e2e/composer.spec.ts`
- Create: `e2e/messages.spec.ts`
- Create: `e2e/process.spec.ts`
- Create: `e2e/archive.spec.ts`
- Create: `e2e/chrome.spec.ts`
- Modify: `e2e/frontend.spec.ts`

- [ ] Step 1: Extract fixture helpers without moving tests.

Run:

```bash
npx playwright test e2e/frontend.spec.ts -g "composer keeps dense references inside the input surface|consecutive tool activity becomes one process evidence group|assistant markdown uses a compact editorial rhythm|project archive shares the Forge material system"
```

Expected: pass before and after fixture extraction.

- [ ] Step 2: Move composer tests.

Implementation rule:

```text
Move tests by domain. Keep test titles unchanged during migration.
```

Run:

```bash
npx playwright test e2e/composer.spec.ts
npx playwright test e2e/frontend.spec.ts -g "composer"
```

Expected:

```text
Composer tests pass in their new file.
No duplicate composer coverage remains in frontend.spec.ts after the move.
```

- [ ] Step 3: Move messages, process, archive, and chrome tests in that order.

Run after each domain move:

```bash
npx playwright test e2e/composer.spec.ts
npx playwright test e2e/messages.spec.ts
npx playwright test e2e/process.spec.ts
npx playwright test e2e/archive.spec.ts
npx playwright test e2e/chrome.spec.ts
npx playwright test e2e/frontend.spec.ts
npm run build
git diff --check
```

Expected:

```text
The moved domain passes in isolation.
frontend.spec.ts stays as cross-domain smoke coverage only.
```

### Phase 7: Build And Bundle Hygiene

**Files:**

- Inspect: `vite.config.ts`
- Inspect: `package.json`
- Inspect: `src/components/messages/CodeBlock.tsx`
- Inspect: `src/components/messages/TextBlock.tsx`
- Inspect: `src/lib/*`

- [ ] Step 1: Record current build warning.

Run:

```bash
npm run build
```

Expected:

```text
Build passes. Vite may warn about large chunks.
```

- [ ] Step 2: Investigate high-impact bundle sources.

Run:

```bash
rg -n "shiki|highlight|monaco|codemirror|import\\(" src vite.config.ts package.json
```

Expected:

```text
The investigation identifies whether large chunks come from syntax highlighting, renderer dependencies, or app shell imports.
```

- [ ] Step 3: Apply lazy loading only when it does not affect first-render stability.

Implementation rule:

```text
Lazy-load expensive renderer helpers only behind existing render paths.
Do not change visible loading states unless the main owner confirms the copy and behavior.
```

- [ ] Step 4: Verify build and primary UI flows.

Run:

```bash
npm run build
npx playwright test e2e/messages.spec.ts e2e/process.spec.ts
git diff --check
```

Expected: all pass.

## Slice Checklist

Use this checklist for every maintenance slice:

- [ ] Identify the one responsibility being improved.
- [ ] Confirm no product concept, protocol, IPC, or backend schema change is required.
- [ ] Add or select characterization coverage before code changes.
- [ ] Make the smallest extraction or polish change.
- [ ] Run the target Playwright command.
- [ ] Run one adjacent regression command.
- [ ] Run `npm run build`.
- [ ] Run `git diff --check`.
- [ ] Update `docs/product/forge-experience-optimization-plan.md` only when a visual slice changes product quality guidance.
- [ ] Update this plan only when the maintenance strategy itself changes.

## Quality Gates

| Gate | Target |
| --- | --- |
| `InputBar.tsx` | Below 280 lines after composer extraction. |
| `HubPanel.tsx` | Below 320 lines after archive extraction. |
| `TextBlock.tsx` | Stays below 260 lines unless a renderer helper is extracted first. |
| `MessageList.tsx` | Stays below 220 lines and keeps grouping logic in `messageGrouping.ts`. |
| CSS domains | No single domain CSS file grows past 900 lines without a split proposal. |
| E2E specs | No domain spec grows past 2200 lines. |
| Protocol files | No diff in `src-tauri/src/protocol/events.rs` or `src/lib/protocol.ts` for polish-only slices. |
| Build | `npm run build` passes before handoff. |
| Whitespace | `git diff --check` passes before handoff. |

## Obsidian Sync Format

This plan is intentionally Markdown-native for Forge's Obsidian/project-memory workflow:

- Use stable headings and tables.
- Keep decisions under product docs, not transient scratch files.
- Do not commit screenshot artifacts.
- Record main-owner confirmation needs as explicit product questions in `docs/product/forge-experience-optimization-plan.md`.
- Keep execution results in commit messages or handoff notes, not in this strategy document.

## Review Rhythm

Use a recurring quality review every 3 to 5 slices:

1. Re-run line counts for the main risk files.
2. Check whether new helpers stayed pure and local.
3. Check whether Playwright specs are still domain-focused.
4. Check whether visual tokens are still centralized.
5. Check whether any slice quietly introduced product vocabulary.
6. Update this plan only when the execution model needs to change.

Suggested review command:

```bash
wc -l src/styles/*.css src/components/session/*.tsx src/components/layout/*.tsx src/components/messages/*.tsx e2e/*.spec.ts
git diff -- src-tauri/src/protocol/events.rs src/lib/protocol.ts src/lib/tauri.ts
git diff --check
```

## Completion Definition

The long-term maintenance track is considered healthy when:

- The largest frontend files are split by responsibility.
- Composer changes can be made without touching process or archive code.
- Message renderer changes can be made without touching composer code.
- CSS token changes have predictable blast radius.
- Playwright failures point to one UI domain instead of the entire app.
- The user-facing app still feels like one calm desktop product.
- No new product concept was introduced without explicit confirmation.

---

## Appendix: Desktop Product Layer Map

> **Last updated:** 2026-06-08. This section maps the current `apps/desktop/src` directory layout to Forge product surfaces. Use it when deciding where a new visual slice belongs, or when assessing whether a directory rename is justified.

### Entry chain

```
src/main.tsx
  → src/App.tsx
    → src/components/layout/AppShell.tsx
      → Sidebar + main-workbench + CapabilityDrawer + CommandPalette + HubPanelHost
```

### Product surfaces ↔ source directories

| Product surface | Current directory / files | What lives there |
|-----------------|--------------------------|------------------|
| **Workbench / Shell** | `src/components/layout/` | `AppShell`, `AppTitlebar`, `Sidebar` + actions/session-history/workspace-menu, `EmptyWorkbench`, `HubPanel` + `HubPanelHost` + `HubPanelContent`, `CapabilityDrawer`, `ProjectCockpit` + `ProjectStatus*` |
| **Conversation** | `src/components/chat/` | `ChatView`, `MessageList`, `ConversationLane`, `BlockRenderer` (renderer entry), `messageGrouping.ts`, scroll & motion hooks |
| **Composer** | `src/components/session/` | `InputBar` (orchestrator), `ComposerSurface`/`ComposerTextarea`/`ComposerToolbar`/`ComposerChipTray`/`ComposerMenuLayer`/`ComposerModelMenu`/`ComposerSuggestionMenu`/`ComposerResumeError`, ~20 `useComposer*` hooks, `composer*.ts` pure logic |
| **Artifacts / Evidence** | `src/components/messages/` | Per-type renderers: `TextBlock`, `ThinkingBlock`, `UserMessage`, `ShellCard` + detail/header/output sections, `DiffCard` + `DiffBody`, `ConfirmCard` + actions/views, `DeliverySummaryCard`, `ToolCallCard`, `CodeBlock`, `DiagramBlock`, `FilePreviewSheet`, `ErrorCard`, `MissingApiKeyCard`, `PendingBlock`, `ContextCompactCard`, various `*Presentation.ts` |
| **Search** | `src/components/CommandPalette.tsx` + `CommandPaletteContent.tsx` | Global command palette (Cmd+K), session switch, theme toggle, settings shortcut |
| **Settings** | `src/components/settings/` | `SettingsDialog` (dialog shell), `SettingsCenterShell` (6-section nav + content), `CapabilityManager` + tabs/rows, provider & local-data sections |
| **Context / Wiki** | `src/components/context/` | `WikiSections*` + `WikiRecord*`, `ActiveContextSection`, `ContinuityExperiencesSection`, `ProjectOverviewCard` |
| **Workflow** | `src/components/workflow/` | `CurrentTaskCard` |
| **Workbench** | `src/components/workbench/` | `StartReadinessCard`, `StartReadinessView` — workbench-level readiness indicator used by layout and chat surfaces |
| **Primitives** | `src/components/primitives/` | Forge-specific thin wrappers over Radix/shadcn primitives: `ForgeButton`, `ForgeDialog`, `ForgeCommandDialog`, `ForgeControlButton`, etc. |
| **UI (shadcn)** | `src/components/ui/` | Stock shadcn/ui components: button, dialog, command, input, textarea, tabs, tooltip, etc. |
| **Styles** | `src/styles/*.css` | Domain-scoped CSS files (see CSS Modules By Responsibility above). `globals.css` is the import coordinator. |

### Naming debt (do not rename without explicit slice approval)

- `components/session/` → should conceptually be `components/composer/` (most files are composer surface or hooks, not generic session logic).
- `components/chat/` → should conceptually be `components/conversation/` (contains MessageList, grouping, scroll — not just "chat" UI).
- `components/messages/` → should conceptually be `components/artifacts/` or `components/evidence/` (contains renderers for all backend event types, not just "messages").

These directories are stable at runtime. Any physical move must be a standalone slice with characterization coverage and must not collide with open branches.

### Import boundary rules

- `layout/` may import from `chat/`, `session/`, `settings/`, `workbench/`, and `primitives/`.
- `chat/` may import from `messages/`, `workbench/`, and `primitives/`.
- `session/` may import from `chat/` (for `BlockRenderer` grouping helpers only) and `primitives/`.
- `messages/` may import from `primitives/` and `lib/*` (protocol, motion, helpers). It must NOT import from `session/` or `settings/`.
- `settings/` may import from `primitives/` and `lib/*`. It must NOT import from `messages/`.
- `workbench/` may import from `primitives/` and `lib/*`. It must NOT import from `session/`, `chat/`, `messages/`, `settings/`, or `context/`.
- `context/` may import from `primitives/` and `lib/*`.
- `primitives/` must NOT import from any product surface directory.
