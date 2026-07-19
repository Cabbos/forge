# Forge Design Language

Updated: 2026-07-18

## Positioning

Forge V1 should feel like a local agent workbench with desktop-product craft: calm like Claude Desktop, precise like Linear, action-ready like Raycast, and dense enough for developer work like Warp and Zed.

The design language is **Console Craft** (v5, successor to v4 "Warm Precision"). It is not a new product concept and should not create new navigation, panels, terms, or user-visible abilities. It is a visual and interaction contract for existing Forge surfaces: cool graphite surfaces, a single cyan signal, and monospaced evidence chrome.

The app shell carries `data-design-version="v5-console-craft"`.

## Design Alignment Contract

Forge should align to the Figma workbench prototypes at the system level, not at the pixel-perfect layer. The prototypes define the product language, hierarchy, density, state vocabulary, and material ladder. The implementation should still adapt to real stream content, platform constraints, component behavior, and accessibility.

Pixel matching is not the goal. A screen is aligned when a developer can move between Figma and the running app without relearning the interface: the same surfaces carry the same roles, risky moments have the same weight, and routine evidence stays quiet until it matters.

### Must Match

- Token roles: base, depth, surface, raised, composer, border, text, muted text, accent, success, warning, and danger.
- Information architecture: sidebar, titlebar, message lane, composer, process feedback, confirmation, delivery summary, archive, settings, and command surfaces.
- State semantics: pending, running, confirmed, resolved, success, failed, interrupted, compacted, and needs permission.
- Density and rhythm: compact desktop controls, bounded message lane, quiet rows, stable gutters, and no marketing-page spacing.
- Risk hierarchy: confirmation, destructive shell/file writes, failed checks, and recovery prompts must be more structured than routine output.

### May Adapt

- Exact x/y positions, heights, and row wrapping may change for real content and responsive behavior.
- shadcn/Tauri component primitives may replace bespoke Figma layers when they preserve the same role and density.
- Long Markdown, shell output, diffs, file paths, and tool traces may introduce scroll, wrapping, or expansion behavior not shown in static frames.
- Copy may change when real product data is clearer than prototype placeholder language.
- Motion may be simpler than the prototype if reduced motion, performance, or focus management benefits.

### Product Can Correct Design

The running product is allowed to correct the prototype when real behavior exposes a better rule. Examples include long session histories, multiline composer input, stalled confirmations, failed shell recovery, missing API keys, and large Markdown tables. When this happens, update the shared token/component rule instead of making a one-off visual exception.

## Brand Image Contract

Forge should read as a trustworthy local agent workbench, not as an AI chat skin, a raw terminal wrapper, or a literal forge/fire metaphor.

The brand image is built from five signals:

- Cool graphite desk: blue-slate charcoal surfaces (`#0A0E14` base), crisp paper text, no warm brown as the default.
- Cyan signal light: `#22D3EE` (dark) / `#0891B2` (light) marks focus, ready actions, current selection, live state, and risk-adjacent attention.
- Evidence first: logs, diffs, shell output, confirmations, and delivery summaries are part of the brand, not secondary debug clutter. Monospace (`Geist Mono Variable` via `--forge-font-mono`) is the voice of evidence, labels, and compact controls.
- Quiet desktop density: compact rows, small controls, hairline borders, and restrained shadows help Forge feel like a daily tool.
- Local control: project boundaries, permissions, MCP, skills, and automation should feel auditable and reversible.

Avoid purple AI glow, playful SaaS dashboards, orange fire decoration, gold luxury styling, terminal cosplay, marketing hero composition, and card walls. If a visual choice does not make agent work clearer, safer, or calmer, it does not belong in V1.

## Reference Inputs

| Reference | What To Borrow | What To Avoid |
| --- | --- | --- |
| Claude Desktop | warm trust, readable conversation, quiet confirmation hierarchy | cream marketing canvas, editorial hero language |
| Linear | hairline precision, restrained dark craft, clear affordance | cold lavender identity, marketing screenshot composition |
| Raycast | keyboard confidence, compact command surfaces, crisp hover states | launcher-only mental model |
| Warp | cool near-charcoal developer density, understated terminal craft | making Forge feel like a raw terminal |
| Zed | thread density, editor-native layout discipline | overly sparse IDE chrome |
| Impeccable | product-register restraint, token discipline, cognitive-load checks | visual spectacle for its own sake |

## Core Scene

A developer opens Forge after lunch. The room is quiet: a graphite desk, a thin cyan signal where attention belongs, evidence stacked in tidy rows. They read the assistant's last answer, glance at a running check, approve one guarded write, and get back to code. Nothing glows without a reason.

## Visual Principles

### Cool Graphite, Not Warm Brown

The dark theme is a blue-slate ladder, not olive-charcoal: base `#0A0E14`, depth `#070B10`, surface `#10161F`, raised `#141B26`, elevated hover `#182130`. Text is cool paper (`#E6E9EF`), muted slate (`#7C8694`), never brown-tinted.

### Cool Light, Not Cream

The light theme is a clean console day mode, not warm paper: base `#F4F6F8`, depth `#EDF1F4`, surface `#FCFDFE`, raised materials `#FFFFFF`. Borders are cool hairlines (`#DCE3EA`, `#C3CED9` for the composer). Accent steps down to `#0891B2` for contrast.

### One Accent With Restraint

Cyan is the only brand accent. It marks the send button when ready, the selected menu/command row (active fill plus a 2px inset left rail), active capability tabs and toggles, focus rings, and live process LEDs. It never floods backgrounds and never appears on routine text.

### Surfaces Form A Ladder

Every surface sits on a named rung of the token ladder (`--forge-bg-*`, `--forge-material-*`). No ad-hoc hex in components, no warm v4 leftovers (`#B88A56`, `rgba(184, 138, 86, …)`, `#FEFCF8` and friends are forbidden by guardrail tests). Diff and Markdown surfaces may use neutral slate tints (`rgba(148, 163, 184, …)`).

### Desktop Density With Breathing Room

Controls are compact (send 1.875rem, tools 1.875rem high, 6px radii on buttons), section labels are uppercase mono micro-labels, and the conversation lane stays bounded at 760px. Breathing room comes from rhythm tokens, not from inflated padding.

### Evidence Is Calm Until It Matters

Routine tool rows are transparent and borderless; running and error states surface through LED dots, mono timestamps, and restrained accent/danger tints. Pulse animations breathe over 1.5s ease-in-out, never faster.

### Markdown Must Stay Bounded

Tables render as token-driven report cards (`--forge-material-raised`, 8px radius, no vertical grid), blockquotes get a 3px left rail, code stays mono and unwrapped inside table cells, and long content scrolls inside the lane instead of stretching it.

## Component Direction

### Composer

A single raised card (light: `#FFFFFF`, `#C3CED9` hairline). Send is a compact 6px-radius square: transparent when idle, cyan fill with `--forge-accent-foreground-strong` text when ready, red-tinted (`#FEF2F2`/`#B91C1C`) for stop. Tool chips are 11.5px mono with 6px radii.

### Message Lane

Assistant turns ride a quiet rail with a square 6px mono avatar. User messages are right-aligned notes (light: `#EFF3F6`) with a 3px `--forge-border-strong` left rail; long notes center and widen. Status disclosures stay borderless until hovered.

### Process Feedback

Tool activity collapses into lightweight disclosure rows; running state reads amber (`#B45309` light), error reads red (`#DC2626` on `rgba(220, 38, 38, 0.06)`). Shell/evidence surfaces keep mono type and the cool ladder.

### Menus And Buttons

Menus and command palettes are material popovers; the selected row is an accent-tinted fill with a 2px inset left rail, not a bordered pill. Buttons are 6px-radius mono; approve actions are solid accent, cancel is a quiet raised fill.

### Project Archive

The work panel keeps terminal-grade surfaces on the dark ladder, mono 11.5px tabs with an accent inset underline for the active tab, and hairline `#DCE3EA` separators in light mode.

## Guardrails

- Style gate (`npm run check:conversation-style`) pins the light-theme token block, composer, message, markdown, process, confirm, and delivery recipes to v5 values.
- Guardrail e2e forbids v4 warm literals in brand assets and diff/markdown styles, dark glass overlays, and blurred backdrops.
- Focus visible states use `box-shadow: 0 0 0 2px var(--forge-focus-ring)` (no outlines), light `rgba(8, 145, 178, 0.35)` / dark `rgba(34, 211, 238, 0.38)`.
- `--color-*` utility aliases are re-declared inside the light theme block so portal/body surfaces resolve light values instead of inheriting frozen `:root` dark ones.

## Verification

- `npm run build` (includes the conversation style gate and `tsc`).
- `npx playwright test e2e/messages.spec.ts e2e/chrome.spec.ts e2e/process.spec.ts e2e/composer.spec.ts e2e/guardrails.spec.ts e2e/acceptance.spec.ts` — theme-coupled assertions carry the v5 palette.
