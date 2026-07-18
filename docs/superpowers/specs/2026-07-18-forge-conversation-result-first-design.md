# Forge Conversation Result-First Design

Date: 2026-07-18
Status: approved in design discussion; pending written-spec review
Scope: Desktop Conversation information hierarchy, live progress, process disclosure, and interruption presentation

## Goal

Reduce the cognitive load of Forge conversations by making the user's message and the assistant's final answer the primary reading path. Replace the current sequence of Thinking, tool, Shell, Diff, usage, and delivery cards with one live micro-progress row during execution and one optional process disclosure after completion.

This specification changes presentation and interaction only. Raw runtime events, authority evidence, persistence, backend protocols, theme colors, and auditability remain intact.

## Problem

The current Conversation renders many runtime blocks at the same visual level:

- Thinking;
- pending state;
- individual or grouped tool activity;
- Shell output;
- Diff evidence;
- provider usage;
- delivery summary;
- assistant text;
- confirmations and errors.

Adjacent tool calls are grouped, but the conversation still asks the user to distinguish primary communication from process evidence. The existing Thinking row has a small dot indicator, yet it does not create a clear sense of progression, state transition, or completion.

The result is a dense timeline with a weak mental model: users must filter the process themselves before they can read the answer.

## Product Principle

The default conversation is **result-first, evidence-on-demand**.

For each turn, the normal reading order is:

1. user message;
2. one live progress row while Forge is working;
3. the assistant's final answer;
4. one quiet process disclosure inside the answer footer.

Process evidence remains available, but it does not compete with the answer unless the user must act or the turn cannot complete.

## Selected Direction

The selected direction is **Result First With Live Micro-Progress**.

Rejected alternatives:

- **Milestone timeline:** keeps separate Think, execution, and verification rows. It improves grouping but still leaves several process messages in every turn.
- **Compressed full trace:** preserves the existing timeline and reduces spacing. It changes density without fixing the user's mental model.
- **Pinned composer progress:** keeps progress visible near the input but forces it to move into the conversation after completion and creates spatial discontinuity.

## Turn Presentation Model

The presentation layer derives one `ConversationTurnView` from the stored raw blocks:

```text
User message
Live progress, while running
Action-required interruption, only when present
Final answer or terminal error
Process disclosure in the final answer footer
```

The view contains five conceptual fields:

- `userMessage`: the initiating user content;
- `liveProgress`: the current safe action summary while the turn is active;
- `interruptions`: unresolved confirmations, permission blocks, or actionable errors;
- `finalAnswer`: the assistant response, or a terminal error when no response can be produced;
- `processDigest`: the ordered, collapsed evidence summary derived from all process blocks.

The raw `BlockState[]` remains the source of truth. This view is a projection, not a replacement event format.

## Default Visibility

### Always Visible

- the user message;
- the current micro-progress row while the turn is running;
- an unresolved item that requires user action;
- the final answer;
- a small process disclosure in the answer footer.

### Hidden By Default

- Thinking content;
- completed tool calls and results;
- successful Shell output;
- Diff details;
- provider usage;
- resolved confirmations;
- resolved nonterminal errors;
- delivery summary cards.

Hidden content is summarized in `processDigest` and remains inspectable.

### Promoted Only When Necessary

- confirmation requests awaiting a decision;
- permission blocks awaiting a decision;
- actionable errors preventing progress;
- a terminal error when the turn cannot produce a final answer.

## Live Micro-Progress

### Placement

The micro-progress row appears directly below the current turn's user message. It belongs to that turn and is never pinned to the composer.

### Content

The row shows exactly one current action. Examples:

- `正在理解需求`
- `正在查看 AppShell.tsx`
- `正在调整工作面板`
- `正在验证构建`
- `正在整理结果`

The label is derived from actual runtime event types and safe metadata. It must not expose:

- private chain-of-thought;
- hidden prompts or context bodies;
- secrets or credential-like values;
- full commands;
- long absolute paths;
- internal orchestration terminology.

When a safe specific label cannot be produced, the row falls back to one of these stable phases:

- `正在理解任务`
- `正在查看相关内容`
- `正在执行操作`
- `正在验证结果`
- `正在整理回答`

### Update Rules

- Only the latest meaningful action is visible.
- Rapid events are coalesced.
- A visible label remains for at least `600ms` unless user action or an error must be surfaced immediately.
- Equivalent consecutive labels do not restart the transition.
- File and object labels are shortened to a safe readable name.
- No percentage, progress fraction, completion estimate, or ETA is shown unless the backend provides a genuine bounded total. This design does not introduce such a total.

### Motion

The running row uses:

- one breathing status point;
- one short moving light trace of approximately `32px`;
- a `120ms` opacity transition when the action label changes.

The motion is continuous enough to communicate activity but must not bounce, spin, scale, or move the row itself.

When final answer text begins:

1. the current label changes to `正在整理结果` if needed;
2. the status point briefly resolves to a completion mark;
3. the progress row fades out without moving surrounding content;
4. the answer continues in its stable location.

With `prefers-reduced-motion: reduce`, the row uses a static status point and immediate text replacement. No looping animation remains.

## Action-Required Interruptions

Unresolved confirmations, permission blocks, and actionable errors appear directly below live progress as one compact interruption surface.

Rules:

- An interruption is visible only while it requires attention.
- It retains the existing backend authority, evidence, and explicit actions.
- It does not expose hidden permissions or infer approval locally.
- Resolving it removes the interruption from the primary timeline.
- The resolved event becomes an item in the process digest.
- Focus is not stolen when an interruption appears; keyboard navigation reaches it in document order.
- After an explicit action, focus moves to the next logical action or returns to the conversation without jumping to hidden content.

If an error prevents the turn from producing a final answer, the error becomes the turn's primary result. It includes recovery guidance and retains the process disclosure. It must not be hidden behind the disclosure.

## Final Answer And Footer

The final answer remains the primary assistant content.

After the answer, one quiet footer control displays:

```text
✓ 已完成 · 4 项操作 · 查看过程
```

Rules:

- The operation count is derived from the digest's meaningful grouped actions, not the raw number of stream events.
- The footer is part of the answer surface, not a separate message block.
- It is collapsed by default, including after history restoration.
- It is reachable and operable by keyboard.
- Its accessible name communicates expanded or collapsed state.
- Expanding it must not move focus unexpectedly or force-scroll the conversation.

If the turn contains no process evidence, the footer is omitted.

## Process Digest

Expanding `查看过程` reveals one lightweight ordered timeline rather than restoring the previous collection of cards.

The digest groups evidence into these categories:

1. **理解** — the generic completed phase `已理解任务`; it is never synthesized from hidden Thinking text;
2. **操作** — files, tools, or user-visible objects inspected or changed;
3. **验证** — checks executed and their result;
4. **异常** — resolved confirmations, permission decisions, retries, or errors.

### Detail Levels

The first expanded level shows compact rows with label, outcome, and optional duration.

Long content remains behind a second disclosure:

- full Shell commands and output;
- complete tool inputs and results;
- Diff bodies;
- verbose failure evidence;
- provider usage metadata.

File paths, loopback preview URLs, and current-worktree Diffs open as concrete Work Panel objects. Evidence without one of those target identities remains in the Conversation disclosure. The digest is a summary and navigation surface, not an embedded IDE.

### Provider Usage

Provider and token usage no longer renders as a standalone timeline card. It appears only as secondary metadata inside the expanded process digest or in existing diagnostic/context surfaces.

### Delivery Summary

The standalone `本轮交付` card is removed from the primary timeline. Its useful actions are redistributed:

- preview and file actions open concrete Work Panel objects;
- checkpoint or verification state appears in the process digest;
- at most one next action appears in the final answer footer, and only when the delivery event provides an explicit actionable prompt.

The footer must not duplicate prose already present in the final answer.

## Data Derivation

The projection consumes existing block types without changing the stream protocol.

Conceptual mapping:

- `user_message` → `userMessage`;
- incomplete `thinking` or `pending` → live understanding phase;
- running `tool_call`, `tool_call_result`, or `shell` → live specific action and digest operation;
- `confirm_ask` → unresolved interruption until backend-resolved;
- `diff_view` → digest operation and Work Panel target;
- `provider_usage` → digest secondary metadata;
- `delivery_summary` → footer actions and digest facts;
- complete `text` → `finalAnswer`;
- terminal `error` without answer → primary terminal error.

Event ordering is preserved inside the digest. Grouping may reduce repeated events but cannot reorder authority decisions or outcomes.

## History And Restoration

Restored sessions derive the same result-first view from persisted raw blocks.

- Completed turns reopen with only the user message, final answer, and collapsed process footer.
- An incomplete restored turn reconstructs the latest safe progress or recovery state.
- Resolved confirmations and historical errors remain in the digest.
- The open or closed state of the process disclosure is ephemeral UI state and defaults to closed after restoration.
- No stored evidence is deleted to achieve the compact presentation.

## Scrolling And Streaming

- Updating the live label does not create a new message block.
- The progress row keeps stable height while labels change.
- Progress updates do not reset the user's scrolled-up state.
- Final answer streaming follows the existing auto-scroll contract.
- Expanding the digest does not force-scroll unless the user activates a navigation target.
- Coalesced progress updates prevent repeated layout and announcement churn.

## Accessibility

- Live progress uses `role="status"` or an equivalent polite live region.
- Only meaningful phase changes are announced; high-frequency tool events are not individually announced.
- The decorative breathing point and light trace are hidden from assistive technology.
- The process footer exposes `aria-expanded` and its controlled region.
- Compact interruptions preserve accessible labels, action names, and backend-derived status.
- Color and motion are not the only indicators of running, complete, blocked, or failed state.
- Reduced-motion mode removes all looping and transitional movement.

## Component Boundaries

The implementation should keep responsibilities focused:

- `messageGrouping.ts` or a new adjacent pure module derives `ConversationTurnView` and `processDigest` from raw blocks.
- `ConversationLane` renders one turn composition instead of mapping every block into an equal-weight timeline row.
- A focused live-progress component owns safe labels, cadence, live-region behavior, and motion.
- A focused process-disclosure component owns digest grouping, keyboard disclosure, and second-level evidence expansion.
- Existing confirmation and error components retain authority-specific actions and are hosted in the interruption slot.
- Existing low-level renderers remain reusable inside expanded evidence details where appropriate.

The backend event protocol and store remain unchanged unless implementation discovery proves an existing event cannot distinguish required unresolved and resolved states.

## Verification

### Pure Projection Coverage

Verify that a mixed turn containing Thinking, tools, Shell, Diff, usage, delivery, and text derives:

- one user message;
- no more than one live progress state;
- only unresolved interruptions in the primary view;
- one final answer;
- one ordered digest with stable grouped-action count.

Verify safe-label fallbacks, duplicate-event coalescing, operation counting, terminal error promotion, and restored completed turns.

### Interaction Coverage

1. A running turn shows one progress row below the user message.
2. Specific safe actions replace the current label without creating new message blocks.
3. Rapid tool events are coalesced and do not visibly flicker.
4. A confirmation appears while unresolved and leaves the primary view when resolved.
5. A recoverable error follows the same rule.
6. Final answer streaming replaces the live progress without a vertical jump.
7. Completed process evidence appears only in the final answer footer.
8. The footer expands and collapses by pointer and keyboard.
9. Long commands, output, and Diffs require a second disclosure or open in the Work Panel.
10. Provider usage and delivery summary do not create standalone timeline cards.
11. History restoration reconstructs the compact completed state.
12. Scrolled-up users are not pulled to the bottom by progress-label changes.
13. Reduced-motion mode contains no looping progress animation.

### Visual Acceptance

Capture and inspect these states at the same viewport:

- initial Thinking;
- active file/tool operation;
- active verification;
- unresolved confirmation;
- final answer with collapsed process footer;
- expanded process digest;
- terminal error;
- reduced-motion rendering;
- narrow Conversation width with the Work Panel open.

Reject captures with duplicated progress, stacked process cards, default-expanded evidence, layout jumps, cropped interruptions, or standalone provider-usage/delivery cards.

## Documentation Impact

Because this changes a user-visible desktop surface, implementation must align:

- `README.md`;
- `apps/desktop/README.md`;
- `CHANGELOG.md`;
- `apps/desktop/e2e/messages.spec.ts`;
- `apps/desktop/e2e/process.spec.ts`;
- `apps/desktop/e2e/acceptance.spec.ts` when the product flow changes;
- `scripts/acceptance.sh --dry-run` descriptions when advertised coverage changes.

## Out Of Scope

- deleting or reducing raw runtime evidence;
- changing provider Thinking or tool protocols;
- exposing private chain-of-thought;
- inventing percentage progress or ETA;
- moving all active confirmations into the Work Panel;
- redesigning the composer, sidebar, Work Panel, or theme colors;
- replacing backend permission authority;
- building a general activity dashboard inside Conversation.

## Approved Decisions

- Default model: result-first, evidence-on-demand.
- Running state: one specific current-action row below the user message.
- Motion: breathing point, short light trace, and opacity-only label transition.
- Completion: process access moves into the final answer footer.
- Interruptions: visible only while unresolved, then folded into the digest.
- Process detail: one ordered lightweight timeline with second-level disclosure for verbose evidence.
- Provider usage: no standalone conversation card.
- Delivery summary: no standalone conversation card; useful actions merge into answer footer and Work Panel navigation.
- Storage: raw events and evidence remain unchanged.
