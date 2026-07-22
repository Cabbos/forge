# Forge Dual-Theme Glass Workbench Design

Date: 2026-07-22

Status: User-approved design

Scope: Forge desktop application visual system, conversation, navigation, work panel, settings, controls, and streaming-render stability

## Goal

Redesign Forge as one coherent desktop workspace with two structurally identical themes:

- a graphite dark theme inspired by the supplied desktop-workbench references;
- a cool-white translucent theme with restrained glass materiality.

The redesign preserves Forge's product information architecture and runtime authority model. It changes visual hierarchy, material treatment, interaction feedback, conversation presentation, and streaming stability. It does not add a new navigation model or silently replace Forge's generation protocol.

## Design Read

This is a redesign overhaul for a developer-facing desktop workbench. The intended language combines a quiet result-first conversation, native macOS-like spatial hierarchy, and tactile controls. The product must remain dense enough for daily technical work without looking like an admin dashboard or a card wall.

Design dials:

- Design variance: 6
- Motion intensity: 4
- Visual density: 6

These values allow a calm asymmetric workspace, purposeful state transitions, and practical developer density without ornamental motion.

## Reference Translation

The supplied references establish the following reusable principles:

- conversation content is the primary visual object;
- assistant answers sit directly on the canvas instead of inside message cards;
- user messages remain compact, right-aligned bubbles;
- process details collapse into a quiet duration row and concise evidence summaries;
- the sidebar is a stable navigation plane with restrained selected-row treatment;
- settings use grouped rows and whitespace instead of one card per setting;
- controls have visible hover, pressed, focus, disabled, and loading states;
- the right-side output surface is contextual and subordinate to the conversation.

The references are not a pixel-copy target. Forge keeps its own file, review, terminal, permission, verification, and work-panel semantics.

## Theme Architecture

Both themes use the same layout, spacing, type roles, radii, component anatomy, and interaction rules. Theme switching changes semantic material tokens only. It must not remount the conversation tree or reset scroll, input, disclosure, or work-panel state.

### Dark Theme

- The main canvas uses a soft graphite near-black rather than pure black.
- The sidebar uses a low-saturation blue-gray material that is distinct from the main canvas without becoming a bright color block.
- Composer, dialogs, menus, settings groups, and other true overlays use translucent dark material with a subtle inner highlight and restrained edge definition.
- Blue is the single interaction accent for focus, enabled toggles, active states, and primary actions.
- Semantic success, warning, and danger colors remain available only where status meaning requires them.

### Light Theme

- The base is a very light cool white with enough tonal variation for transparent materials to remain legible.
- Sidebar, composer, dialogs, and settings groups use differentiated white-glass materials.
- Glass depth comes from a translucent fill, a cool outer edge, a brighter inner edge, and a background-tinted shadow. Blur alone is insufficient.
- Routine content sits directly on the canvas. Containers are reserved for real grouping, input, or elevation.
- The solid-surface fallback remains visually complete when transparency is reduced or unsupported.

### Material Ladder

The semantic ladder consists of:

1. base canvas;
2. navigation plane;
3. quiet grouped surface;
4. raised control surface;
5. floating popover;
6. modal overlay.

Components consume these roles rather than declaring local colors or blur values. Blur is limited to actual overlays and selected glass surfaces. It must not be applied to every card or scrolling content container.

### Shape and Shadow Rules

- Large shells and modal containers: 18px radius.
- Composer, popovers, and grouped settings surfaces: 16px radius.
- Standard controls and compact containers: 10px radius.
- Icon-only circular actions may use a full radius.
- Shadows communicate actual elevation only and are tinted to the current background family.
- Inner highlights and hairline borders provide material edges without outer glow.

## Typography and Density

- Continue using the existing Geist and Geist Mono families.
- Conversation prose uses the sans family with larger line height and paragraph spacing.
- Evidence, file paths, commands, durations, and compact technical metadata use the mono family.
- Assistant prose targets a readable measure of roughly 70 to 82 Chinese characters at common desktop widths.
- Settings and navigation remain compact, while conversation spacing prioritizes reading.
- Section labels are used only when they improve navigation. Decorative uppercase labels and repeated micro-headings are removed.

## Application Shell

The desktop shell has three spatial roles:

1. The left sidebar owns projects, tasks, history, and global navigation.
2. The central conversation is always the primary workspace.
3. The right Work Panel appears only for user-opened previews, files, reviews, terminals, and subtasks.

Opening the Work Panel reduces the conversation width only while a comfortable reading measure remains. At narrow widths, the Work Panel becomes a flush full-workspace surface instead of crushing the conversation into a narrow column. The titlebar remains thin and contains only the current task identity, workspace location, and necessary window or panel actions.

## Conversation Design

### Turn Anatomy

Each turn follows a result-first sequence:

1. The user message appears as a right-aligned, low-contrast bubble.
2. A compact progress row communicates the current stage while work is running.
3. The final assistant answer appears directly on the canvas without an avatar, author label, or card shell.
4. A terminal duration row such as `已处理 32s` separates the result from the expandable process evidence.
5. Copy, feedback, retry, and related actions sit beneath the answer with low default contrast and stronger hover visibility.

### Evidence Hierarchy

Routine reads, commands, edits, and checks collapse into concise summaries such as:

- `运行了多个命令`
- `查看了 3 个文件`
- `修改了 2 个文件`
- `完成了构建验证`

Users can expand these summaries to inspect full evidence. Permission requests, unresolved failures, risky actions, and recovery choices remain expanded and prominent. Evidence is never hidden when user action is required.

### Composer

- The composer is anchored at the bottom of the reading workspace without obscuring the final message.
- It uses the theme's raised glass material and follows the 16px radius rule.
- Model, permission, attachment, and context controls remain visible but subordinate to the text entry.
- The send action is the only persistent primary action.
- Loading or stop states preserve control width and position to prevent layout movement.

## Stable Streaming and Generate UI

### Current Diagnosis

The current conversation renderer has no independent structured Generate UI layer. Text responses pass through `ReactMarkdown`. Streaming updates are throttled and unclosed code fences are repaired, but the full active Markdown content is still reprocessed as it grows. Incomplete lists, code fences, tables, and diagrams can therefore change element structure during streaming. This is the primary implementation hypothesis for visible flashing and must be confirmed with a reproducible streaming case before modification.

### Stability Contract

- A new answer container may animate once when it first appears. Streaming text within it does not repeatedly animate.
- Completed paragraphs, list items, and code blocks become immutable render segments.
- Only the active trailing segment updates during streaming.
- Complex structures such as tables, diagrams, and fenced code use a stable provisional representation until structurally complete, then replace it once.
- Turn ids, block ids, React keys, scroll anchors, disclosure state, composer focus, and work-panel state remain stable throughout streaming.
- Theme changes do not remount streamed content.
- Auto-scroll follows the stream only while the user remains at the bottom. Manual upward scrolling is never overridden.

### Fix-First Strategy

The first implementation path keeps the existing Forge renderer and introduces stable render segments around it. This preserves file references, code actions, diagrams, evidence presentation, and current security boundaries.

If the existing path remains unstable after the stability contract is implemented and measured, evaluate Streamdown as the first open-source streaming Markdown replacement. Any replacement must support Forge's custom renderers and pass the same stability, accessibility, and security tests before adoption.

If Forge later adds model-generated structured interactive components, treat that as a separate product capability. Evaluate Vercel's open-source json-render with:

- a schema-validated component tree;
- a fixed allowlisted component catalog;
- explicit action bindings;
- stable incremental JSON patches;
- no evaluation of arbitrary model-generated JavaScript or component source.

Structured generative UI is not required to complete the present conversation redesign.

## Sidebar

- Dark mode uses the low-saturation blue-gray navigation material from the reference family.
- Light mode uses a translucent cool-white navigation material.
- Current project and task selection use a quiet full-row fill rather than a bright outline or accent flood.
- Navigation is grouped by work, projects, and system only where grouping improves scanability.
- Icons use one family, one optical size, and one stroke-weight rule.
- Colored dots appear only for real semantic state.
- The collapsed sidebar retains essential icons and accessible tooltips.

## Work Panel

- The panel remains a native split workspace, not a drawer containing another card.
- Its object bar is compact and aligned with the titlebar hierarchy.
- Preview, file, review, terminal, and subtask content fills the available area without redundant outer containers.
- Empty, unavailable, and failed states remain local to the active tab and preserve sibling tabs.
- New output may mark an existing object as unread but never steals focus or changes the active tab.

## Settings

- Use a stable left navigation and right content structure.
- Search remains at the top of the settings navigation.
- Related settings share one grouped material surface.
- Setting rows align label and description on the left with the action on the right.
- Dividers are sparse and support grouping rather than boxing every row.
- High-risk permissions receive a dedicated group and explicit impact copy.
- Search results navigate to and focus the exact setting row.

## Controls and Interaction

Every interactive control implements the full state cycle:

- default;
- hover;
- active;
- focus-visible;
- disabled;
- loading or pending;
- selected where applicable.

Interaction rules:

- Hover increases edge clarity and fill concentration without outer glow.
- Active controls scale to approximately 0.98 and move down by 1px where layout permits.
- Transitions run between 120ms and 220ms with a fast-start, soft-stop curve.
- Icon buttons provide at least a 32px target.
- Switches communicate state through position, shape, and color.
- Menus align spatially with their triggers and preserve a consistent minimum width.
- Loading labels do not change the control's measured width.
- Focus is always visible for keyboard users.

Motion above basic state feedback respects `prefers-reduced-motion`. Translucent materials provide a solid accessible fallback for reduced-transparency preferences and unsupported environments.

## Empty, Loading, Error, and Recovery States

- Empty states contain one clear next action and no decorative card wall.
- Loading uses skeletons that match the final content geometry.
- Errors stay adjacent to the failed message, setting, preview, or panel tab.
- Existing content remains visible during provider, preview, or network failures.
- Retry, switch-provider, open-externally, close, and recovery choices appear only where valid.
- Permission and dangerous-action prompts stay highly visible and explain both action and impact.

## Migration Plan Boundary

The implementation plan should sequence work as follows:

1. Establish dual-theme semantic tokens, material roles, radii, shadows, and control states.
2. Rebuild conversation presentation and stable streaming behavior.
3. Align composer, menus, message actions, permissions, and evidence summaries.
4. Align sidebar, titlebar, and Work Panel.
5. Align settings, command palette, dialogs, and shared primitives.
6. Remove superseded Console Craft recipes and redundant theme overrides.
7. Update design documentation, desktop documentation, changelog, screenshots, and acceptance coverage.

The implementation plan must isolate visual-token work from streaming-render behavior so regressions can be attributed and reviewed independently.

## Accessibility and Performance

- Body text and control labels meet WCAG AA contrast in both themes.
- Keyboard navigation covers sidebar, composer, evidence disclosures, Work Panel, settings, menus, and dialogs.
- Focus order follows the visual order.
- Reduced motion and reduced transparency preserve complete functionality.
- Blur is not applied to large scrolling descendants that would cause continuous repaint cost.
- Streaming updates avoid full-tree remounts and retain input responsiveness.
- Theme switching retains hierarchy and contrast parity.

## Verification

### Visual Verification

Capture stable screenshots for both themes covering:

- empty workbench;
- active conversation with a user message and long assistant answer;
- streaming progress and completed duration disclosure;
- expanded evidence with commands, file changes, and verification;
- focused composer and control states;
- Work Panel preview and review;
- settings navigation and grouped rows;
- command palette, permission prompt, and error state.

### Streaming Verification

- Stream a response for at least 60 seconds without full-answer flashes or repeated entry animation.
- Exercise paragraphs, nested lists, fenced code, tables, file references, and diagrams.
- Confirm stable scroll anchoring when following the stream and when manually scrolled upward.
- Confirm composer focus, disclosure state, and selected Work Panel tab survive streaming updates.
- Confirm reduced-motion mode removes entry transitions.

### Interaction Verification

- Verify default, hover, active, focus-visible, disabled, loading, and selected states for shared buttons and controls.
- Verify theme switching does not change layout geometry or lose state.
- Verify sidebar collapse, Work Panel resize, narrow-window fallback, settings search, menus, and dialogs by keyboard.
- Verify transparency fallbacks remain readable without backdrop blur.

### Required Project Gates

- `npm run build:desktop`
- relevant desktop component tests for conversation grouping, streaming segmentation, controls, settings, and Work Panel
- relevant Playwright coverage in `apps/desktop/e2e/acceptance.spec.ts`
- theme and conversation guardrails
- `scripts/acceptance.sh --dry-run`

## Documentation Impact

Because this redesign changes user-visible runtime surfaces, implementation must keep these files aligned:

- `apps/desktop/docs/product/forge-design-language.md`
- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `apps/desktop/e2e/acceptance.spec.ts`
- `scripts/acceptance.sh`

## Out of Scope

- Changing Forge's product information architecture.
- Adding new runtime authority or bypassing permission gates.
- Executing arbitrary model-generated UI code.
- Replacing the current renderer before a reproducible stability test exists.
- Introducing a second component design system.
- Reworking the website prototype as part of the desktop redesign.

## Acceptance Criteria

The design is complete when:

1. Dark and light themes share one component anatomy and interaction model.
2. Conversation matches the result-first reference hierarchy while retaining Forge evidence and permission semantics.
3. Routine evidence is concise by default and risky or failed states remain prominent.
4. Streaming for 60 seconds produces no full-answer flash, repeated entry animation, focus loss, or forced scroll jump.
5. Sidebar, Work Panel, settings, composer, dialogs, and command surfaces use one coherent material and radius system.
6. Every shared control has complete pointer, keyboard, loading, and disabled behavior.
7. Reduced motion and reduced transparency modes remain fully usable.
8. Documentation, screenshots, tests, and acceptance coverage describe and verify the new design language.
