# Forge Conversation Result-First Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task by task. Use `test-driven-development` for each behavior change and `verification-before-completion` before claiming completion.

**Goal:** Replace the desktop Conversation's equal-weight event wall with one result-first turn containing the user message, one safe live progress row, unresolved interruptions, the final answer, and a collapsed process digest.

**Architecture:** Keep `BlockState[]` and all backend/persistence events unchanged. Add a pure presentation projection beside `messageGrouping.ts`, then make `ConversationLane` render that projection through focused progress and process-disclosure components. Reuse existing block renderers only for primary user/answer/interruption content and second-level evidence. Put new structural styling in a dedicated stylesheet that consumes existing theme tokens without changing the user's current palette.

**Tech Stack:** React 18, TypeScript, Base UI disclosure primitives, existing Forge motion utilities, CSS custom properties, Node test runner, Playwright.

---

## Guardrails

- Treat raw transcript blocks as immutable evidence; do not delete, rewrite, or stop persisting any event type.
- Do not change provider protocols, confirmation authority, permission handling, store hydration, or usage-ledger accounting.
- Do not expose Thinking content, complete tool inputs, full commands, credentials, or long absolute paths in the live progress label.
- Do not add percentages, fake step counts, ETAs, or completion estimates.
- Do not edit theme colors. New CSS may use existing semantic variables only.
- Preserve unresolved confirmation behavior and backend response calls exactly.
- Preserve the current scroll contract; progress-label changes must not change raw block count.
- Before modifying any existing function, run GitNexus upstream impact analysis. Run `detect_changes` before every commit.
- The worktree contains user-owned design changes. Stage only files from the task being committed.

## Task 1: Add the pure turn projection

**Files:**

- Create: `apps/desktop/src/components/chat/conversationTurnView.ts`
- Create: `apps/desktop/src/components/chat/conversationTurnView.test.ts`
- Modify: `apps/desktop/src/components/chat/messageGrouping.ts`

**Step 1: Write failing projection tests**

Cover a mixed turn with `user_message`, Thinking, tool start/result, Shell, Diff, resolved confirmation, provider usage, delivery summary, and final text. Assert the derived view has:

```ts
assert.equal(view.userMessage?.event_type, "user_message");
assert.equal(view.finalAnswer?.event_type, "text");
assert.equal(view.interruptions.length, 0);
assert.equal(view.processDigest.operationCount, 4);
assert.deepEqual(view.processDigest.items.map((item) => item.kind), [
  "understanding",
  "operation",
  "verification",
  "exception",
]);
```

Add focused cases for:

- unresolved versus `metadata.confirmed === true` confirmation;
- interrupted restored confirmations;
- terminal error promotion when no assistant answer exists;
- recoverable/historical error folding when an answer follows;
- latest complete text selected as the final answer;
- streaming text marked as answer-started and suppressing live progress;
- provider usage and delivery summary retained as digest metadata, not primary items;
- duplicate tool start/result events grouped into one meaningful operation;
- original block order retained in digest evidence references.

**Step 2: Run the test to prove it fails**

Run:

```bash
node --test apps/desktop/src/components/chat/conversationTurnView.test.ts
```

Expected: FAIL because `conversationTurnView.ts` does not exist.

**Step 3: Implement the smallest pure model**

Define stable presentation types:

```ts
export interface ConversationTurnView {
  key: string;
  userMessage: BlockState | null;
  finalAnswer: BlockState | null;
  terminalError: BlockState | null;
  interruptions: BlockState[];
  liveProgress: LiveProgressCandidate | null;
  processDigest: ProcessDigest;
}

export interface ProcessDigest {
  items: ProcessDigestItem[];
  operationCount: number;
  delivery: BlockState | null;
  usage: BlockState[];
}
```

Export `deriveConversationTurnView(turn: ConversationTurn)` and pure predicates for unresolved confirmations and primary terminal errors. Keep raw `MessageItem` evidence references on digest items so detailed rendering can reuse current components without reparsing content.

Update `messageGrouping.ts` only as needed to export existing item predicates or to make their semantics explicit. Do not change storage or event ordering.

**Step 4: Run the focused tests**

Run:

```bash
node --test apps/desktop/src/components/chat/conversationTurnView.test.ts
```

Expected: PASS.

**Step 5: Commit the pure projection**

Run GitNexus `detect_changes({scope: "staged"})`, review the report, then commit only the files above:

```bash
git commit -m "feat(desktop): derive result-first conversation turns"
```

## Task 2: Derive safe live-progress labels

**Files:**

- Create: `apps/desktop/src/components/chat/conversationProgress.ts`
- Create: `apps/desktop/src/components/chat/conversationProgress.test.ts`

**Step 1: Write failing safe-label tests**

Exercise real metadata shapes from `blocks.ts`:

```ts
assert.equal(progressLabel(block("thinking", {})), "正在理解任务");
assert.equal(progressLabel(tool("read_file", { path: "/repo/src/AppShell.tsx" })), "正在查看 AppShell.tsx");
assert.equal(progressLabel(shell("npm run build")), "正在验证构建");
```

Also verify:

- read/search/edit/write tools produce specific but short labels;
- build/test/check/lint commands produce verification labels without revealing the command;
- unknown tools fall back to `正在执行操作`;
- text streaming produces `正在整理回答` only before visible answer content starts;
- credential-like values, prompts, JSON bodies, URLs with secrets, and absolute path ancestors never appear;
- labels are capped to a short readable length;
- equivalent consecutive candidates have the same identity key for coalescing.

**Step 2: Run the test to prove it fails**

```bash
node --test apps/desktop/src/components/chat/conversationProgress.test.ts
```

Expected: FAIL because the module does not exist.

**Step 3: Implement safe derivation**

Implement `deriveLiveProgressCandidate(blocks)` and helpers that inspect only allow-listed metadata keys. Use `basename`-style string handling without importing Node-only path APIs into the browser bundle. Never render raw `tool_input` or `command` values; use them solely to classify an allow-listed operation and extract a sanitized basename.

**Step 4: Run focused tests**

```bash
node --test apps/desktop/src/components/chat/conversationProgress.test.ts
```

Expected: PASS.

**Step 5: Commit safe progress derivation**

Run staged GitNexus change detection, then:

```bash
git commit -m "feat(desktop): derive safe conversation progress"
```

## Task 3: Render one live progress row

**Files:**

- Create: `apps/desktop/src/components/chat/ConversationProgress.tsx`
- Create: `apps/desktop/src/components/chat/useStableProgressLabel.ts`
- Create: `apps/desktop/src/components/chat/useStableProgressLabel.test.ts`
- Create: `apps/desktop/src/styles/conversation-turn.css`
- Modify: `apps/desktop/src/components/chat/ConversationLane.tsx`

**Step 1: Write failing cadence tests**

Test a pure scheduler/reducer exported by `useStableProgressLabel.ts`:

- the first meaningful label appears immediately;
- a replacement waits until the current label has been visible for 600ms;
- rapid replacements coalesce to the latest candidate;
- the same identity does not restart the dwell window;
- an interruption or error bypasses the dwell window;
- answer start removes the running row without appending another message.

Use injected timestamps rather than sleeping in tests.

**Step 2: Prove failure, then implement cadence**

```bash
node --test apps/desktop/src/components/chat/useStableProgressLabel.test.ts
```

Expected before implementation: FAIL. Expected after minimal implementation: PASS.

**Step 3: Build the accessible row**

`ConversationProgress` must render one stable-height row directly below the user message:

```tsx
<div role="status" aria-live="polite" data-testid="conversation-progress">
  <span aria-hidden="true" className="forge-turn-progress-dot" />
  <span className="forge-turn-progress-label">{visibleLabel}</span>
  <span aria-hidden="true" className="forge-turn-progress-trace" />
</div>
```

The dedicated stylesheet provides:

- one breathing dot;
- a light trace whose travel is approximately 32px;
- a 120ms opacity-only label transition;
- stable row height;
- a complete static fallback under `prefers-reduced-motion: reduce`.

Use existing color/token variables and avoid hard-coded theme colors.

**Step 4: Wire the row into the lane**

Convert each `ConversationTurn` to `ConversationTurnView` inside or immediately before `ConversationLane`. Render the user message, then the single progress row. Do not yet remove final answer or interruption rendering; this task establishes the stable live slot.

**Step 5: Build and run focused browser coverage**

```bash
npm run build --prefix apps/desktop
npm run test:e2e --prefix apps/desktop -- e2e/process.spec.ts
```

Expected: build succeeds; existing process behavior remains operable while focused selectors are updated in a later task.

**Step 6: Commit the live row**

After staged GitNexus change detection:

```bash
git commit -m "feat(desktop): show one live turn progress row"
```

## Task 4: Replace the event wall with result-first composition

**Files:**

- Create: `apps/desktop/src/components/chat/ConversationTurn.tsx`
- Modify: `apps/desktop/src/components/chat/ConversationLane.tsx`
- Modify: `apps/desktop/src/components/chat/BlockRenderer.tsx` only if a focused evidence-rendering export is required
- Modify: `apps/desktop/src/styles/conversation-turn.css`
- Create: `apps/desktop/e2e/conversation-result-first.spec.ts`

**Step 1: Write failing Playwright coverage**

Add a fixture turn containing all noisy block types. Assert:

- exactly one user message and one final answer remain in the primary path;
- exactly one progress row exists while running;
- Thinking, tool, Shell, provider usage, and delivery summary cards are absent from the primary path;
- unresolved confirmation remains visible;
- a resolved confirmation disappears from the primary path;
- a terminal error becomes the primary result when no text answer exists;
- historical restored completed turns are collapsed by default.

**Step 2: Run the new spec to prove failure**

```bash
npm run test:e2e --prefix apps/desktop -- e2e/conversation-result-first.spec.ts
```

Expected: FAIL against the equal-weight block wall.

**Step 3: Implement `ConversationTurn` composition**

Render in this exact order:

```text
userMessage
liveProgress
unresolved interruptions
finalAnswer or terminalError
processDisclosure
```

Use `MemoizedBlockRenderer` for user, answer, unresolved confirmation, and terminal error. Do not render provider usage or delivery summary through `BlockRenderer` in the primary path. Preserve internal-context suppression.

**Step 4: Run the focused spec and build**

```bash
npm run test:e2e --prefix apps/desktop -- e2e/conversation-result-first.spec.ts
npm run build --prefix apps/desktop
```

Expected: PASS.

**Step 5: Commit result-first composition**

After staged GitNexus change detection:

```bash
git commit -m "feat(desktop): render result-first conversation turns"
```

## Task 5: Add the process digest and answer footer

**Files:**

- Create: `apps/desktop/src/components/chat/ConversationProcessDisclosure.tsx`
- Create: `apps/desktop/src/components/chat/ConversationProcessItem.tsx`
- Modify: `apps/desktop/src/components/chat/ConversationTurn.tsx`
- Modify: `apps/desktop/src/styles/conversation-turn.css`
- Modify: `apps/desktop/e2e/conversation-result-first.spec.ts`

**Step 1: Extend failing interaction tests**

Assert a completed mixed turn shows:

```text
✓ 已完成 · 4 项操作 · 查看过程
```

Then verify:

- disclosure defaults closed after live completion and history restoration;
- pointer and keyboard activation toggle `aria-expanded`;
- the first expanded level is one ordered lightweight timeline;
- raw Shell/tool/Diff/provider details remain behind a second disclosure;
- expanding does not focus hidden content or force-scroll;
- no standalone provider-usage or delivery-summary card appears;
- an explicit delivery next action appears at most once in the footer.

**Step 2: Implement the footer and ordered digest**

Use the existing Forge/Base UI collapsible primitive. Render digest summary rows directly; reuse `ToolActivityDetails`, Diff detail rendering, and usage rendering only inside second-level evidence. Keep the footer visually part of the answer surface, not a new message card.

For delivery actions, reuse the existing pending-input behavior but expose no more than one action. Do not duplicate answer prose.

**Step 3: Add concrete Work Panel navigation where identity exists**

When a digest item has a safe file path, loopback preview URL, or current-worktree Diff identity, use existing Work Panel state/actions to open that concrete object. Leave non-concrete evidence in the disclosure. Run GitNexus impact analysis on the selected Work Panel state function before editing or importing it into this flow.

**Step 4: Verify focused interaction**

```bash
npm run test:e2e --prefix apps/desktop -- e2e/conversation-result-first.spec.ts
npm run build --prefix apps/desktop
```

Expected: PASS.

**Step 5: Commit the digest**

After staged GitNexus change detection:

```bash
git commit -m "feat(desktop): fold process evidence into answer footer"
```

## Task 6: Reconcile existing tests and accessibility contracts

**Files:**

- Modify: `apps/desktop/e2e/messages.spec.ts`
- Modify: `apps/desktop/e2e/process.spec.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `apps/desktop/e2e/guardrails.spec.ts`
- Modify: `apps/desktop/scripts/check-conversation-style.mjs` if its structural rules describe the old card wall

**Step 1: Update assertions, not evidence fixtures**

Keep fixtures emitting the same raw events. Replace assertions that expect standalone Thinking/tool/usage/delivery cards with result-first assertions and disclosure expansion where evidence must be inspected.

**Step 2: Add accessibility and motion checks**

Cover:

- one polite live region per running turn;
- `aria-expanded` and controlled digest region;
- keyboard toggle;
- unresolved confirmation document order;
- reduced-motion static point and no looping trace animation;
- no repeated live-region announcements for coalesced equivalent labels;
- scroll position preserved when only the progress label changes.

**Step 3: Run the desktop regression slice**

```bash
npm run test:e2e --prefix apps/desktop -- e2e/conversation-result-first.spec.ts e2e/messages.spec.ts e2e/process.spec.ts e2e/acceptance.spec.ts
npm run check:conversation-style --prefix apps/desktop
npm run build --prefix apps/desktop
```

Expected: PASS.

**Step 4: Commit coverage**

After staged GitNexus change detection:

```bash
git commit -m "test(desktop): cover result-first conversation flow"
```

## Task 7: Align product documentation and acceptance advertising

**Files:**

- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh` only if its dry-run labels enumerate the old Conversation behavior
- Modify: `docs/superpowers/specs/2026-07-18-forge-conversation-result-first-design.md`

**Step 1: Describe the user-visible behavior**

Document result-first turns, the single live current-action row, evidence-on-demand, unresolved interruption visibility, and unchanged raw evidence/auditability. Do not describe private Thinking or claim bounded progress.

Because the three documentation files already contain user-owned edits, merge the statements into the current content and inspect each diff before staging.

**Step 2: Verify acceptance advertising**

```bash
scripts/acceptance.sh --dry-run
```

Expected: the advertised desktop coverage remains accurate and the command exits successfully.

**Step 3: Commit documentation**

After staged GitNexus change detection:

```bash
git commit -m "docs(desktop): document result-first conversation"
```

## Task 8: Full verification and visual review

**Files:**

- No production changes unless verification identifies a scoped defect.

**Step 1: Run required repository verification**

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh --dry-run
```

Expected: all commands pass.

**Step 2: Run the focused desktop suite**

```bash
node --test \
  apps/desktop/src/components/chat/conversationTurnView.test.ts \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts
npm run test:e2e --prefix apps/desktop -- \
  e2e/conversation-result-first.spec.ts \
  e2e/messages.spec.ts \
  e2e/process.spec.ts \
  e2e/acceptance.spec.ts
```

Expected: all pass.

**Step 3: Inspect visual states**

Capture the initial Thinking, active file operation, verification, unresolved confirmation, collapsed result, expanded digest, terminal error, reduced-motion, and narrow-with-Work-Panel states. Reject any capture with stacked process cards, two progress rows, default-open evidence, layout jumps, clipped interruptions, or palette changes.

**Step 4: Run final change analysis**

Run:

```text
detect_changes({scope: "compare", base_ref: "main"})
```

Confirm that changed symbols are confined to Conversation presentation, tests, and advertised documentation. If Work Panel navigation imports expand the blast radius, verify those flows explicitly.

**Step 5: Final handoff**

Report concrete test output, visual-review evidence, and any pre-existing dirty-worktree files that were deliberately excluded. Do not claim completion while a required check is failing or the old equal-weight card wall remains reachable in the default path.

## Self-Review

- The plan covers every approved visibility, progress, interruption, footer, digest, restoration, scroll, accessibility, and reduced-motion requirement.
- All new behavior starts with a failing automated test.
- The plan preserves raw events and authority behavior.
- It avoids theme changes and isolates new structural CSS.
- It accounts for the dirty user-owned worktree and staged-scope commits.
- It includes exact files, commands, expected outcomes, and commit boundaries.
- It leaves no placeholder implementation steps.
