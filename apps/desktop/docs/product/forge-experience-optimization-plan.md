# Forge Experience Optimization Plan

Updated: 2026-05-22

## North Star

Forge should feel like a mature desktop agent surface: calm, trustworthy, spatially ordered, and efficient for local project work. The reference quality is closer to Claude Desktop than to a raw terminal wrapper, while still preserving Forge's local agent speed.

This plan is intentionally about polish and implementation quality. It does not introduce new user-visible product concepts, entries, modules, panels, terms, or abilities.

## First Priority

The first priority is the conversation workbench:

1. Composer
2. Running process feedback
3. Tool feedback
4. Markdown and structured output stability

Right-side and Project Archive areas are included only for visual consistency: materials, borders, scrolling, density, empty states, and layout stability.

Long-term frontend maintenance and refactor slices are tracked in `docs/product/forge-frontend-maintainability-plan.md`.

## Experience Principles

## Competitor Reference Scorecard

This scorecard translates external references into Forge V1 execution rules. It is not a request to copy product concepts or add new modules.

| Reference | What Forge Learns | V1 Execution Rule | Current Focus |
| --- | --- | --- | --- |
| Claude Desktop | calm desktop conversation, trustworthy confirmations, low-noise composer | Use quiet defaults, promote only decisions/failures/long evidence into structured surfaces | composer, confirmations, message lane |
| Cursor | code-agent execution clarity, diff and terminal evidence | Keep edits, shell output, failed checks, and recoveries inspectable without making routine steps heavy | diff, shell, verification recovery |
| Windsurf Cascade | long-running agent flow, checkpoints, linter/problem feedback | Group continuous tool work and show checkpoints/delivery as confidence evidence, not product theater | process groups, delivery summary |
| Zed Agent Panel | dense editor-native thread feel | Prefer compact rows, stable lanes, and reduced chrome over card-heavy chat UI | titlebar, sidebar, process rows |
| Linear | precise affordance, command-first operations, restrained status UI | Buttons, menus, selected rows, and status pills use one consistent desktop interaction language | menus, focus, hover, status |
| Raycast | fast launcher-like actions and keyboard confidence | Existing `/`, `@`, model, copy, open, locate, and resume actions stay keyboard-friendly and visually bounded | command menu, file refs |
| Obsidian | local-first knowledge and durable Markdown records | Project Archive feels like local project memory while remaining visually secondary to the current task | archive density, docs sync |

Score each slice against these checks before merging:

- Claude: does the default state feel calm?
- Cursor/Windsurf: can the user audit important work evidence?
- Zed: does the UI keep desktop density without cramped text?
- Linear/Raycast: are actions clear, keyboard-safe, and visually consistent?
- Obsidian: does durable project context stay local, quiet, and secondary?

### Quiet By Default

Thinking, shell execution, pending tool calls, and routine progress should render as low-contrast status rows. They should show enough evidence to maintain trust without creating a wall of heavy cards.

### Progressive Disclosure

Only promote content into a stronger surface when the user needs to inspect or decide:

- failure
- confirmation
- dangerous write or shell action
- long shell output
- diff
- audit-relevant evidence
- overflow-prone Markdown

### Stable Desktop Materials

Core surfaces should share one material system for border, background, shadow, focus, hover, and overlay behavior. Composer, message panels, popovers, process detail, and Project Archive should look like parts of one desktop app.

### Lane Safety

Model output must stay inside the message lane. Tables, code blocks, ASCII diagrams, long paths, long commands, and long replies must not stretch the conversation, cover controls, or produce layout jumps.

### No Product Expansion

If a polish issue requires a new data field, event type, command, panel, workflow, or visible product term, stop and confirm with the main owner before implementation.

## Current Slice

The current implementation slice establishes the shared material baseline:

- tokenized material border, surface, raised, popover, overlay, and shadow values
- composer focus and default material states
- log detail surface consistency
- model menu popover consistency
- Project Archive panel consistency
- Playwright coverage for the above surfaces

This is a foundation slice. It should make later composer and process feedback work less scattered.

The follow-up composer slice keeps dense references inside the input surface:

- long file reference chips keep ellipsis behavior
- many selected references stay inside a capped chip tray
- the chip tray scrolls internally instead of growing the whole composer
- long prompts cap at a quieter desktop input height before scrolling internally
- narrow desktop windows let the toolbar wrap inside the composer instead of pushing controls out
- toolbar and send controls remain reachable under dense context
- Playwright coverage verifies dense long references, long prompts, and a short-width composer together

The process feedback slice keeps dense evidence summaries quiet:

- consecutive tool and shell activity remains one compact process group
- dense summary labels stay on one line inside the message lane
- summary items use ellipsis instead of wrapping into a heavy block
- expanded details, failures, running states, thinking, and pending rows keep their existing behavior
- Playwright coverage verifies narrow-width dense process evidence

The desktop chrome slice relaxes the main app titlebar without changing navigation:

- the primary titlebar gets enough height for title plus project boundary metadata
- project metadata stays visually secondary but no longer presses into the title
- left and right titlebar padding match a calmer desktop reading rhythm
- secondary titlebars such as archive and drawers keep their compact baseline
- Playwright coverage verifies the main titlebar spacing, project chip height, and action affordances

The sidebar refinement slice keeps the rail dense but less compressed:

- the brand block and workspace boundary get a little more vertical breathing room
- primary actions and history rows keep the existing compact 28px rhythm
- active conversation rows reserve enough inset beside the status accent
- utility and drawer surfaces keep their previous compact sizing
- Playwright coverage verifies sidebar rail metrics, active row inset, and drawer boundaries

The conversation rhythm slice gives the main reading lane more air without adding cards:

- conversation and composer vertical gutters move together from 16px to 18px
- message block spacing moves from 12px to 14px for calmer scan rhythm
- turn separation increases to 16px while keeping transparent, cardless turns
- scroll-to-bottom placement continues to follow the same rhythm token
- Playwright coverage verifies gutters, turn spacing, lane gap, and hidden work structure

The message affordance slice makes routine message actions feel more native:

- assistant copy actions sit inside the reserved message action slot instead of floating over the edge
- copy affordances keep a compact 26px desktop target with low-noise reveal behavior
- user messages use a quieter material with a subtle border and inset highlight instead of a flat bright bubble
- long pasted prompts, code references, Markdown tables, code blocks, and diagrams keep their existing lane containment
- Playwright coverage verifies message material, copy action position, and adjacent Markdown/diagram regressions

The resolved confirmation slice keeps audit evidence without repeating a full decision surface:

- pending confirmations keep the existing structured boundary grid and explicit continue/cancel actions
- approved or cancelled confirmations remove the secondary helper line and collapse into a tighter audit header
- resolved icons lose their contained badge so the state pill and summary carry the evidence
- resolved summaries keep workspace, operation, affected scope, and first file visible in one compact row
- Playwright coverage verifies resolved height, pending confirmation behavior, long command containment, and delivery recovery adjacency

The delivery summary slice keeps handoff evidence useful without becoming a wide status wall:

- delivery cards use their own compact surface width instead of inheriting the widest structured panel baseline
- summary items use a bounded auto-fit grid so evidence follows actual item count rather than spreading into empty tracks
- primary delivery actions reuse Forge's 28px desktop button rhythm, focus ring, hover material, and loaded state
- failed checks, pending records, first-loop delivery, and project-path redaction keep their existing behavior
- Playwright coverage verifies delivery width, grid density, action affordance, repair prompt loading, archive opening, and first-loop regressions

The composer floating menu slice makes command and file suggestions feel like bounded desktop popovers:

- `/` and `@` suggestion menus keep the same existing commands and file search behavior
- suggestion menus no longer stretch across the full conversation lane on wide desktops
- wide desktop menus cap at a focused picker width while narrow windows still use available space
- model menu behavior, keyboard selection, active row styling, and one-menu-at-a-time behavior stay unchanged
- Playwright coverage verifies suggestion width, floating gap, keyboard selection, model menu positioning, narrow composer bounds, and menu dismissal

The failed shell evidence slice makes failed checks more inspectable without making routine process rows heavy:

- successful and running shell rows keep the same quiet 22px process rhythm
- failed shell detail surfaces receive a subtle error tone only after expansion
- stderr sections become bounded evidence blocks with light error material, preserving copy and scroll behavior
- process grouping, failed delivery repair prompts, and first-loop recovery behavior stay unchanged
- Playwright coverage verifies failed shell evidence styling plus adjacent process, delivery, and first-loop regressions

The compact Markdown table slice keeps small reference tables from feeling like heavy panels:

- short two-column tables fit closer to their content instead of inheriting a wide default surface
- wide tables still cap at the message lane and fall back to internal horizontal scroll
- table border, header, cell padding, and scrollbar behavior stay on the existing Markdown renderer path
- no new Markdown renderer, schema, or user-visible output concept is introduced
- Playwright coverage verifies compact table width and wide table overflow together

The inline file reference slice makes long paths feel like quiet local project tokens:

- file references inside inline code keep their existing preview/open behavior
- long path labels explicitly wrap inside the message lane instead of relying on inherited wrapping
- nested file links drop the standard underline so path tokens read like local project references
- visible labels compact to file name plus line metadata, while full paths stay available in title and accessibility labels
- regular external links keep their existing underline treatment
- Playwright coverage verifies long inline file references stay bounded and visually quiet

## Implementation Order

### Phase 1: Material Baseline

- Centralize material tokens in `src/styles/globals.css`.
- Apply them to composer, message panels, process detail, popovers, and Project Archive.
- Verify titlebar, sidebar, composer, archive, and popover are visually aligned.

### Phase 2: Composer Polish

- Keep the input area stable under focus, long text, file chips, model menu, errors, sending, and stopping.
- Prevent pasted code or long task text from stretching the layout.
- Keep menu layering clear without covering the active typing area.

### Phase 3: Process Feedback

- Keep routine thinking and tool progress as quiet status rows.
- Promote failures, confirmations, long logs, diffs, and risky actions into structured surfaces.
- Avoid a heavy card wall during continuous tool execution.

### Phase 4: Markdown Resilience

- Keep tables, code blocks, diagrams, paths, and commands inside the message lane.
- Improve ASCII architecture diagram rendering through the existing `DiagramBlock` path only.
- Reduce flicker, layout jumps, and floating control overlap.

### Phase 5: Right And Archive Consistency

- Align right-side and Project Archive materials, scroll behavior, borders, density, and empty states.
- Record information architecture concerns as follow-up notes.
- Do not add new archive concepts, entries, or abilities in this plan.

## Acceptance Criteria

- Composer does not jump, stretch, or lose visual hierarchy during focus, long input, chips, model menu, send, stop, and error states.
- Routine process feedback feels quiet and readable.
- Failure, confirmation, diff, and long output states are visibly inspectable.
- Markdown tables, code, ASCII diagrams, long paths, and long commands stay inside the conversation lane.
- Scroll-to-bottom controls do not cover primary content.
- Project Archive and right-side surfaces match the same material system.
- Reduced motion remains usable and does not rely on animation for comprehension.
- No Rust `StreamEvent`, IPC, backend schema, or user-visible product concept changes are introduced by polish-only slices.

## Test Coverage

Each polish slice should follow this rhythm:

1. Add or update a failing Playwright check.
2. Implement the smallest visual or component change.
3. Run the target Playwright spec.
4. Run adjacent regression coverage.
5. Run `npm run build`.
6. Run `git diff --check`.

Key Playwright areas:

- composer sizing, focus, long text, file chips, model menu, send, stop, and errors
- thinking, shell, pending, confirm, diff, default, expanded, and failed states
- Markdown table, code block, ASCII diagram, long path, and long command containment
- scroll retention, scroll-to-bottom button, short windows, and reduced motion
- titlebar, sidebar, right-side, and Project Archive material consistency

## Boundaries

Do not change:

- Rust `StreamEvent`
- TypeScript protocol schema
- IPC methods
- backend event contracts
- model response schema
- user-visible navigation or module structure
- user-visible product vocabulary

Allowed changes:

- CSS tokens
- component class composition
- visual density
- message and tool renderer styling
- Playwright coverage
- product planning documentation

## Notes For Obsidian

This document is written as a durable project note. It can be copied into Forge's Obsidian space as the source of truth for the current polish track. Temporary brainstorm files, screenshots, and one-off review notes should stay out of commits unless they become durable product documentation.
