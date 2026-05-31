# Forge Design Language

Updated: 2026-05-27

## Positioning

Forge V1 should feel like a local agent workbench with desktop-product craft: calm like Claude Desktop, precise like Linear, action-ready like Raycast, and dense enough for developer work like Warp and Zed.

The design language is **Warm Precision**. It is not a new product concept and should not create new navigation, panels, terms, or user-visible abilities. It is a visual and interaction contract for existing Forge surfaces.

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

- Warm dark desk: olive-charcoal surfaces, warm paper text, no blue-slate graphite as the default.
- Copper action light: `#B88A56` marks focus, ready actions, current selection, live state, and risk-adjacent attention.
- Evidence first: logs, diffs, shell output, confirmations, and delivery summaries are part of the brand, not secondary debug clutter.
- Quiet desktop density: compact rows, small controls, hairline borders, and restrained shadows help Forge feel like a daily tool.
- Local control: project boundaries, permissions, MCP, skills, and automation should feel auditable and reversible.

Avoid purple AI glow, blue SaaS dashboards, orange fire decoration, gold luxury styling, terminal cosplay, marketing hero composition, and card walls. If a visual choice does not make agent work clearer, safer, or calmer, it does not belong in V1.

## Reference Inputs

| Reference | What To Borrow | What To Avoid |
| --- | --- | --- |
| Claude Desktop | warm trust, readable conversation, quiet confirmation hierarchy | cream marketing canvas, editorial hero language |
| Linear | hairline precision, restrained dark craft, clear affordance | cold lavender identity, marketing screenshot composition |
| Raycast | keyboard confidence, compact command surfaces, crisp hover states | launcher-only mental model |
| Warp | warm near-charcoal developer density, understated terminal craft | making Forge feel like a raw terminal |
| Zed | thread density, editor-native layout discipline | overly sparse IDE chrome |
| Impeccable | product-register restraint, token discipline, cognitive-load checks | visual spectacle for its own sake |

## Core Scene

Forge is used by a developer or operator working in a local project for long stretches, often in a dim desktop environment, while delegating real file and shell work to an agent. The UI should lower cognitive noise, preserve evidence, and make risky moments feel deliberate.

The light theme scene is the same user in a brighter daytime desk environment. Light mode should feel like a warm local workbench, not a generic white SaaS dashboard. It exists for comfort, readability, and OS/theme fit while preserving the same evidence-first behavior.

## Visual Principles

### Warm Dark, Not Cold Graphite

The base surface should be a tinted near-black, leaning warm olive-charcoal rather than blue slate. Text should read like warm paper on a dark desk, not stark white on black.

- Base: `#1B1A17`
- Depth: `#12110F`
- Surface: `#26231E`
- Raised: `#2A2721`
- Primary text: `#EEEAE1`
- Secondary text: `#CFC7B8`
- Muted text: `#928B7E`

### Warm Light, Not White SaaS

The light theme should invert the material logic without becoming stark white. It uses warm paper and stone neutrals, with the same copper accent and restrained state colors.

- Base: `#F7F2E9`
- Depth: `#ECE2D4`
- Surface: `#FBF7EF`
- Raised: `#F3EADC`
- Control: `#E8DCCD`
- Border: `#D8C9B8`
- Primary text: `#2D2923`
- Secondary text: `#51483D`
- Muted text: `#7E7368`

Light mode should preserve density and evidence hierarchy. Do not add pure white cards, blue SaaS accents, or larger spacing to make the interface feel "lighter." The same workbench should simply be usable under brighter ambient light.

### One Accent With Restraint

Forge uses a quiet copper-gold accent for live state, focus, ready action, and warning-adjacent attention. It should stay below the level of brand decoration.

- Accent: `#B88A56`
- Accent hover: `#C79A69`
- Focus ring: `rgba(184, 138, 86, 0.34)`

Do not add purple, blue, or gradient accents as a default styling escape hatch.

### Surfaces Form A Ladder

Use the same material ladder across titlebar, sidebar, conversation, composer, popovers, process detail, and Project Archive.

| Surface | Role |
| --- | --- |
| App base | the quiet desk |
| Depth | sidebar and deep background |
| Surface | normal panels and Markdown blocks |
| Raised | evidence, popovers, archive sections |
| Composer | the grounded input table |
| Overlay | temporary inspection layers |

Cards remain reserved for decisions, failures, diffs, long evidence, and delivery summaries. Routine conversation and process feedback should not become a card wall.

### Desktop Density With Breathing Room

Forge should stay compact, but not cramped:

- 8px max radius for product surfaces.
- 28-32px controls for repeated desktop actions.
- Stable gutters and lane widths instead of full-width content.
- No nested cards.
- No decorative orbs, bokeh, or purple gradients.

### Evidence Is Calm Until It Matters

Thinking, shell execution, and routine tools stay quiet. Confirmations, failed checks, diffs, dangerous writes, and long output earn stronger structure.

### Markdown Must Stay Bounded

Tables, code, ASCII diagrams, inline file paths, and long commands must remain inside the message lane. The renderer should preserve readability without adding new output concepts.

## Component Direction

### Composer

The composer is the primary visual anchor. It should feel stable, warm, and slightly raised. Long text, file chips, model menu, resume state, and errors should not resize the workbench unpredictably.

### Message Lane

Assistant output reads as document-like prose on the canvas. User messages are compact local notes, not bright chat bubbles. Copy actions are available but quiet.

### Process Feedback

Process rows follow a Zed-like density. Expanded details share one raised evidence material. Failure states add tone only where inspection is needed.

### Menus And Buttons

Menus follow Raycast-like crispness: bounded width, clear selected row, low-noise hover, keyboard-safe focus. Buttons should use icons where the action is already familiar.

### Project Archive

Project Archive inherits the same material, border, density, and scroll behavior. It should feel like local project memory, but remain secondary to the active task.

## Guardrails

- Do not change `StreamEvent`, IPC, protocol schema, or model response schema for visual polish.
- Do not add new visible modules, panels, entries, product terms, or abilities.
- Do not introduce a new component library for V1 polish.
- Do not use visual novelty to hide information hierarchy issues.
- If a better experience requires new data fields or IA changes, record it as a confirmation item instead of implementing it.

## Verification

Each slice should keep these checks current:

- token values stay warm, readable, and restrained
- composer and core surfaces use the same material ladder
- process detail, popovers, archive, and message panels share border and elevation rules
- long Markdown content remains bounded
- reduced motion remains understandable
- screenshots of dense conversations do not read as a wall of cards
