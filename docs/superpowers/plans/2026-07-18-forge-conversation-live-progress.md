# Forge Conversation Live Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace noisy, implementation-specific conversation progress with one delayed, stable, high-level status line that resolves into an honest duration-and-operation footer with a two-to-four-stage inline summary.

**Architecture:** Persist frontend-observed turn start and authoritative terminal timestamps on the user-message block, normalize runtime evidence into a finite safe stage vocabulary, and keep all presentation derivation in pure chat modules. The existing `ConversationTurn` remains the sole renderer: it shows one live line while a turn runs, streams the answer without dropping that line, then renders one compact terminal disclosure. Existing theme tokens are consumed unchanged.

**Tech Stack:** React 18, TypeScript, Zustand, Base UI collapsible primitives, CSS layers and media queries, Node test runner, Playwright, GitNexus.

## Implementation Status (2026-07-19)

- Tasks 1–5 are implemented on `cabbos/conversation-result-first`; Task 6 is the remaining documentation and final-verification gate.
- Task 1 includes a durability amendment: transcript replay injects each record's authoritative `recorded_at_ms`, then restores `turn_started_at_ms`, `turn_terminal_at_ms`, and `turn_outcome` when hydrating or tailing a session. Legacy records without sufficient timing evidence keep a `null` duration instead of receiving a fabricated elapsed time.
- Task 5 covers the singular safe live row, anti-flash and dwell timing, answer-stream continuity, completed/failed/stopped semantics, reduced motion, nested raw/runtime evidence, and the product-level result-first acceptance lifecycle. Rich preview regressions remain covered behind the explicit second-level evidence controls.

---

## Scope and Risk Record

This plan implements the approved design in `docs/superpowers/specs/2026-07-18-forge-conversation-live-progress-design.md`.

GitNexus impact results captured before planning:

- `deriveConversationTurnView`: LOW, zero indexed upstream callers. The index does not capture all TSX composition, so this result is not treated as sufficient by itself.
- `createSessionActions`: CRITICAL, one direct caller, 22 affected processes, 53 transitive symbols across Store, Messages, Layout, Session, Settings, History, Workpanel, and Context.
- `createOutputEventDispatcher`: CRITICAL, one direct caller, 22 affected processes, 53 transitive symbols across the same authority domains.
- `ConversationTurn`, `TurnProgress`, and `ConversationProcessDisclosure`: GitNexus returned UNKNOWN because the current index did not resolve those TSX function symbols. Manual call-chain inspection found `ConversationLane -> ConversationTurn -> TurnProgress / ConversationProcessDisclosure -> ConversationProcessItem`.

Risk response:

- Do not change the Store action or dispatcher public interfaces.
- Put timestamp mutation in one pure helper.
- Touch only the existing user-message, `agent_turn_updated`, and `session_stopped` branches.
- Cover the helper and both dispatcher terminal paths with deterministic tests.
- Run Store tests, chat projection tests, focused message E2E, product acceptance E2E, production build, and precommit checks.
- Before every implementation commit, run `detect_changes({ scope: "staged" })`; before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })` and review the conversation and Store flows it reports.

## File Structure

### New files

- `apps/desktop/src/lib/conversationTurnTiming.ts` — pure metadata keys, turn start stamping, terminal outcome stamping, and timing parsing shared by Store and chat projection.
- `apps/desktop/src/lib/conversationTurnTiming.test.ts` — deterministic lifecycle and legacy-data tests.

### Modified files

- `apps/desktop/src/store/session-actions.ts` — stamp a new user turn once when `addUserMessage` creates the user block.
- `apps/desktop/src/store/event-dispatch.ts` — close the latest open turn from authoritative `agent_turn_updated` terminal states and `session_stopped` fallback.
- `apps/desktop/src/store/event-dispatch.test.ts` — prove completed, failed, cancelled, and stopped outcome stamping without disturbing other Store state.
- `apps/desktop/src/components/chat/conversationProgress.ts` — finite safe stage vocabulary plus initial-delay and minimum-dwell state machine.
- `apps/desktop/src/components/chat/conversationProgress.test.ts` — safe mapping and payload-leak tests.
- `apps/desktop/src/components/chat/useStableProgressLabel.ts` — schedule the initial delay and later stage dwell from the pure state machine.
- `apps/desktop/src/components/chat/useStableProgressLabel.test.ts` — delay, coalescing, answering suppression, and urgent-waiting tests.
- `apps/desktop/src/components/chat/conversationTurnView.ts` — terminal summary, streaming-answer progress, meaningful operation grouping, and compact safe stage derivation.
- `apps/desktop/src/components/chat/conversationTurnView.test.ts` — result, duration, operation count, failure, stop, and two-to-four-stage coverage.
- `apps/desktop/src/components/chat/TurnProgress.tsx` — expose live versus waiting state without rendering internal details.
- `apps/desktop/src/components/chat/ConversationTurn.tsx` — preserve progress during answer streaming and render the terminal footer for direct, failed, and stopped turns.
- `apps/desktop/src/components/chat/ConversationProcessDisclosure.tsx` — render terminal wording, one inline key-stage disclosure, and a separately collapsed second-level evidence region.
- `apps/desktop/src/components/chat/ConversationProcessItem.tsx` — render a safe stage row; raw blocks remain available only after an explicit second-level evidence action and never appear in the stage label.
- `apps/desktop/src/components/chat/messageGrouping.ts` — re-export the revised view types.
- `apps/desktop/src/styles/conversation-turn.css` — 22px unframed line, weak dot and trace motion, terminal crossfade, safe expansion rhythm, and reduced-motion behavior.
- `apps/desktop/scripts/check-conversation-style.mjs` — lock the unframed progress and reduced-motion contract.
- `apps/desktop/e2e/messages.spec.ts` — detailed live/answer/terminal/expand/failure/stop/motion coverage.
- `apps/desktop/e2e/acceptance.spec.ts` — one product-level result-first lifecycle smoke.
- `README.md` — update the root product-surface description.
- `apps/desktop/README.md` — update desktop behavior and evidence wording.
- `CHANGELOG.md` — record the user-visible refinement.

### Deleted files

- `apps/desktop/src/components/chat/conversationProcessTarget.ts` — no longer needed because inline key stages must not open files or Work Panel objects.
- `apps/desktop/src/components/chat/conversationProcessTarget.test.ts` — removed with the obsolete target derivation.

The raw `BlockState[]` remains persisted. The first expansion shows only two to four safe stages; raw blocks and usage/delivery metadata remain available only behind an explicitly collapsed second-level `运行证据` action. This preserves existing debugging and rich-preview ownership without putting evidence in the normal conversation path.

---

### Task 1: Record authoritative turn timing and outcome

**Files:**

- Create: `apps/desktop/src/lib/conversationTurnTiming.ts`
- Create: `apps/desktop/src/lib/conversationTurnTiming.test.ts`
- Modify: `apps/desktop/src/store/session-actions.ts:1-218`
- Modify: `apps/desktop/src/store/event-dispatch.ts:56-625`
- Modify: `apps/desktop/src/store/event-dispatch.test.ts:1-910`

- [ ] **Step 1: Re-run Store impact analysis and report the CRITICAL blast radius before editing**

Run through GitNexus:

```text
impact({
  repo: "forge",
  target: "createSessionActions",
  file_path: "apps/desktop/src/store/session-actions.ts",
  kind: "Function",
  direction: "upstream",
  includeTests: true,
  summaryOnly: true
})

impact({
  repo: "forge",
  target: "createOutputEventDispatcher",
  file_path: "apps/desktop/src/store/event-dispatch.ts",
  kind: "Function",
  direction: "upstream",
  includeTests: true,
  summaryOnly: true
})
```

Expected: both reports remain CRITICAL because these functions create the shared Store actions and dispatcher. Report the direct caller count, affected process count, and selected regression tests before editing.

- [ ] **Step 2: Write the failing pure timing tests**

Create `apps/desktop/src/lib/conversationTurnTiming.test.ts` with:

```ts
import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "./protocol.ts";
import {
  markLatestConversationTurnTerminal,
  readConversationTurnTiming,
  startConversationTurnMetadata,
  turnOutcomeForAgentStatus,
} from "./conversationTurnTiming.ts";

test("stamps and closes only the latest open user turn", () => {
  const first = userBlock("first", 1_000, { turn_terminal_at_ms: 2_000, turn_outcome: "completed" });
  const second = userBlock("second", 5_000);
  const blocks = markLatestConversationTurnTerminal([first, second], "completed", 17_250);

  assert.deepEqual(readConversationTurnTiming(blocks[0]), {
    startedAtMs: 1_000,
    terminalAtMs: 2_000,
    outcome: "completed",
    durationMs: 1_000,
  });
  assert.deepEqual(readConversationTurnTiming(blocks[1]), {
    startedAtMs: 5_000,
    terminalAtMs: 17_250,
    outcome: "completed",
    durationMs: 12_250,
  });
});

test("terminal outcome is idempotent and guards clock skew", () => {
  const stopped = markLatestConversationTurnTerminal(
    [userBlock("user", 5_000)],
    "stopped",
    4_000,
  );
  const repeated = markLatestConversationTurnTerminal(stopped, "failed", 9_000);

  assert.equal(repeated, stopped);
  assert.deepEqual(readConversationTurnTiming(stopped[0]), {
    startedAtMs: 5_000,
    terminalAtMs: 5_000,
    outcome: "stopped",
    durationMs: 0,
  });
});

test("legacy turns remain honest when timing is unavailable", () => {
  const legacy: BlockState = {
    block_id: "legacy",
    event_type: "user_message",
    content: "旧对话",
    isComplete: true,
    metadata: {},
  };

  assert.deepEqual(readConversationTurnTiming(legacy), {
    startedAtMs: null,
    terminalAtMs: null,
    outcome: null,
    durationMs: null,
  });
  assert.deepEqual(startConversationTurnMetadata(123), { turn_started_at_ms: 123 });
});

test("maps only authoritative terminal agent states", () => {
  assert.equal(turnOutcomeForAgentStatus("completed"), "completed");
  assert.equal(turnOutcomeForAgentStatus("failed"), "failed");
  assert.equal(turnOutcomeForAgentStatus("cancelled"), "stopped");
  assert.equal(turnOutcomeForAgentStatus("verifying"), null);
});

function userBlock(
  blockId: string,
  startedAtMs: number,
  extra: Record<string, unknown> = {},
): BlockState {
  return {
    block_id: blockId,
    event_type: "user_message",
    content: blockId,
    isComplete: true,
    metadata: { ...startConversationTurnMetadata(startedAtMs), ...extra },
  };
}
```

- [ ] **Step 3: Run the timing test and verify it fails**

Run:

```bash
node --test apps/desktop/src/lib/conversationTurnTiming.test.ts
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `conversationTurnTiming.ts`.

- [ ] **Step 4: Implement the pure timing helper**

Create `apps/desktop/src/lib/conversationTurnTiming.ts` with:

```ts
import type { AgentTurnStatus, BlockState } from "./protocol.ts";

export type ConversationTurnOutcome = "completed" | "stopped" | "failed";

const STARTED_AT_KEY = "turn_started_at_ms";
const TERMINAL_AT_KEY = "turn_terminal_at_ms";
const OUTCOME_KEY = "turn_outcome";

export function startConversationTurnMetadata(now: number): Record<string, unknown> {
  return { [STARTED_AT_KEY]: finiteTimestamp(now) ?? 0 };
}

export function turnOutcomeForAgentStatus(status: AgentTurnStatus): ConversationTurnOutcome | null {
  if (status === "completed") return "completed";
  if (status === "failed") return "failed";
  if (status === "cancelled") return "stopped";
  return null;
}

export function markLatestConversationTurnTerminal(
  blocks: BlockState[],
  outcome: ConversationTurnOutcome,
  now: number,
): BlockState[] {
  const userIndex = findLastUserMessageIndex(blocks);
  if (userIndex < 0) return blocks;
  const user = blocks[userIndex];
  if (conversationTurnOutcome(user.metadata[OUTCOME_KEY])) return blocks;

  const startedAtMs = finiteTimestamp(user.metadata[STARTED_AT_KEY]);
  const observedAtMs = finiteTimestamp(now);
  const terminalAtMs = startedAtMs === null
    ? observedAtMs
    : Math.max(startedAtMs, observedAtMs ?? startedAtMs);
  const next = [...blocks];
  next[userIndex] = {
    ...user,
    metadata: {
      ...user.metadata,
      [TERMINAL_AT_KEY]: terminalAtMs,
      [OUTCOME_KEY]: outcome,
    },
  };
  return next;
}

export function readConversationTurnTiming(block: BlockState | null) {
  const startedAtMs = finiteTimestamp(block?.metadata[STARTED_AT_KEY]);
  const terminalAtMs = finiteTimestamp(block?.metadata[TERMINAL_AT_KEY]);
  const outcome = conversationTurnOutcome(block?.metadata[OUTCOME_KEY]);
  return {
    startedAtMs,
    terminalAtMs,
    outcome,
    durationMs: startedAtMs === null || terminalAtMs === null
      ? null
      : Math.max(0, terminalAtMs - startedAtMs),
  };
}

function findLastUserMessageIndex(blocks: BlockState[]) {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    if (blocks[index]?.event_type === "user_message") return index;
  }
  return -1;
}

function finiteTimestamp(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function conversationTurnOutcome(value: unknown): ConversationTurnOutcome | null {
  return value === "completed" || value === "stopped" || value === "failed" ? value : null;
}
```

- [ ] **Step 5: Run the timing test and verify it passes**

Run:

```bash
node --test apps/desktop/src/lib/conversationTurnTiming.test.ts
```

Expected: 4 tests PASS.

- [ ] **Step 6: Add failing dispatcher lifecycle tests**

Append to `apps/desktop/src/store/event-dispatch.test.ts`:

```ts
describe("createOutputEventDispatcher conversation turn outcome", () => {
  it("closes the latest timed turn from authoritative agent status", () => {
    const { state, dispatch } = createHarness([timedUserBlock(1_000)]);
    const originalNow = Date.now;
    Date.now = () => 13_000;
    try {
      dispatch({
        event_type: "agent_turn_updated",
        session_id: "session-1",
        state: testAgentTurnProjection("completed"),
      });
    } finally {
      Date.now = originalNow;
    }

    const user = state.sessions.get("session-1")!.blocks[0];
    assert.strictEqual(user.metadata.turn_outcome, "completed");
    assert.strictEqual(user.metadata.turn_terminal_at_ms, 13_000);
  });

  it("uses session_stopped as a stopped fallback without rewriting completed turns", () => {
    const { state, dispatch } = createHarness([timedUserBlock(2_000)]);
    const originalNow = Date.now;
    Date.now = () => 10_000;
    try {
      dispatch({ event_type: "session_stopped", session_id: "session-1", reason: "user" });
      dispatch({
        event_type: "agent_turn_updated",
        session_id: "session-1",
        state: testAgentTurnProjection("failed"),
      });
    } finally {
      Date.now = originalNow;
    }

    const user = state.sessions.get("session-1")!.blocks[0];
    assert.strictEqual(user.metadata.turn_outcome, "stopped");
    assert.strictEqual(user.metadata.turn_terminal_at_ms, 10_000);
  });
});

function timedUserBlock(startedAtMs: number): SessionState["blocks"][number] {
  return {
    block_id: "user-turn",
    event_type: "user_message",
    content: "处理这个任务",
    isComplete: true,
    metadata: { turn_started_at_ms: startedAtMs },
  };
}

function testAgentTurnProjection(status: "completed" | "failed") {
  return {
    session_id: "session-1",
    status,
    step_label: status,
    workspace_path: "/workspace",
    compact_count: 0,
    verification_status: status === "failed" ? "failed" as const : "passed" as const,
    model_rounds: 1,
    tool_call_count: 1,
    failed_tool_count: status === "failed" ? 1 : 0,
    stop_reason: null,
    compact_saved_tokens: 0,
  };
}
```

- [ ] **Step 7: Run the dispatcher tests and verify the new tests fail**

Run:

```bash
node --test apps/desktop/src/store/event-dispatch.test.ts
```

Expected: the two new assertions FAIL because no turn outcome metadata is written yet.

- [ ] **Step 8: Integrate start and terminal stamping without changing Store interfaces**

In `apps/desktop/src/store/session-actions.ts`, add:

```ts
import { startConversationTurnMetadata } from "../lib/conversationTurnTiming";
```

Change only the metadata of the new user block:

```ts
const startedAtMs = Date.now();
filtered.push({
  block_id: blockId,
  event_type: "user_message",
  content: text,
  isComplete: true,
  metadata: startConversationTurnMetadata(startedAtMs),
});
```

Use the same `startedAtMs` for the session's `updatedAt` assignment.

In `apps/desktop/src/store/event-dispatch.ts`, add:

```ts
import {
  markLatestConversationTurnTerminal,
  turnOutcomeForAgentStatus,
} from "../lib/conversationTurnTiming";
```

Replace the `agent_turn_updated` branch with the same projection update plus narrow terminal stamping:

```ts
if (event_type === "agent_turn_updated") {
  const agentTurnBySession = new Map(get().agentTurnBySession);
  agentTurnBySession.set(session_id, event.state);
  const outcome = turnOutcomeForAgentStatus(event.state.status);
  const session = get().sessions.get(session_id);
  if (!outcome || !session) {
    set({ agentTurnBySession });
    return;
  }

  const sessions = new Map(get().sessions);
  const blocks = markLatestConversationTurnTerminal(session.blocks, outcome, Date.now());
  sessions.set(session_id, touchSession(session, { blocks, streaming: false }));
  set({ agentTurnBySession, sessions });
  persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
  persistBlocksNow(session_id, blocks);
  return;
}
```

In the existing `session_stopped` branch, stamp before saving:

```ts
blocks = closeInterruptedConfirmBlocks(blocks, "session_stopped");
blocks = markLatestConversationTurnTerminal(blocks, "stopped", Date.now());
```

- [ ] **Step 9: Run Store and timing tests**

Run:

```bash
node --test \
  apps/desktop/src/lib/conversationTurnTiming.test.ts \
  apps/desktop/src/store/event-dispatch.test.ts \
  apps/desktop/src/store/blocks.test.ts
```

Expected: all tests PASS and existing replay/dedup tests remain green.

- [ ] **Step 10: Stage only Task 1 files, inspect GitNexus changes, and commit**

```bash
git add \
  apps/desktop/src/lib/conversationTurnTiming.ts \
  apps/desktop/src/lib/conversationTurnTiming.test.ts \
  apps/desktop/src/store/session-actions.ts \
  apps/desktop/src/store/event-dispatch.ts \
  apps/desktop/src/store/event-dispatch.test.ts
```

Run `detect_changes({ repo: "forge", scope: "staged", worktree: "/Users/cabbos/project/forge" })`. Review that the high-risk Store flow is limited to turn metadata lifecycle, then commit:

```bash
git commit -m "feat(desktop): record conversation turn outcomes"
```

---

### Task 2: Normalize and stabilize the single live stage

**Files:**

- Modify: `apps/desktop/src/components/chat/conversationProgress.ts:1-143`
- Modify: `apps/desktop/src/components/chat/conversationProgress.test.ts:1-108`
- Modify: `apps/desktop/src/components/chat/useStableProgressLabel.ts:1-30`
- Modify: `apps/desktop/src/components/chat/useStableProgressLabel.test.ts:1-78`

- [ ] **Step 1: Run or record impact analysis for the progress symbols**

Run `impact` for `deriveLiveProgressCandidate`, `createStableProgressState`, and `updateStableProgress` with their exact file path. If the index still cannot resolve `deriveLiveProgressCandidate`, record the fallback report: index result, `ConversationTurnView` import, `useStableProgressLabel` caller, selected unit tests, `messages.spec.ts`, and residual TSX-index risk.

- [ ] **Step 2: Replace filename-oriented expectations with failing stage-only tests**

In `apps/desktop/src/components/chat/conversationProgress.test.ts`, make the core expectations:

```ts
test("maps live activity to the finite user-facing stage vocabulary", () => {
  const derive = (turnView as ProgressModule).deriveLiveProgressCandidate!;

  assert.deepEqual(derive([
    incompleteBlock("read", "tool_call", {
      tool_name: "read_file",
      tool_input: { path: "/repo/private/AppShell.tsx", token: "never-show-this" },
    }),
  ]), { id: "discovering", label: "正在查找相关内容", motion: "live" });

  assert.deepEqual(derive([
    incompleteBlock("edit", "tool_call", {
      tool_name: "edit",
      tool_input: { file_path: "/repo/AppShell.tsx", replacement: "secret body" },
    }),
  ]), { id: "modifying", label: "正在进行修改", motion: "live" });

  assert.deepEqual(derive([
    incompleteBlock("test", "shell", { command: "npm test -- --token never-show-this" }),
  ]), { id: "verifying", label: "正在验证结果", motion: "live" });
});

test("never exposes raw payload text in any candidate", () => {
  const derive = (turnView as ProgressModule).deriveLiveProgressCandidate!;
  const candidates = [
    derive([incompleteBlock("read", "tool_call", { tool_name: "read_file", tool_input: { path: "/repo/Secret.tsx" } })]),
    derive([incompleteBlock("shell", "shell", { command: "curl https://example.test?token=secret" })]),
    derive([incompleteBlock("text", "text", {})]),
  ];

  const visible = JSON.stringify(candidates);
  assert.equal(visible.includes("Secret.tsx"), false);
  assert.equal(visible.includes("curl"), false);
  assert.equal(visible.includes("token"), false);
});
```

Update `ProgressModule` to return `{ id: string; label: string; motion: "live" | "paused" } | null`.

- [ ] **Step 3: Add failing state-machine tests for initial delay and answering suppression**

Replace the state-machine test types and helper with:

```ts
interface StableProgressState {
  visible: LiveProgressCandidate | null;
  visibleSince: number;
  pending: LiveProgressCandidate | null;
  dueAt: number | null;
  hasPresented: boolean;
}

type StableProgressModule = {
  createStableProgressState: (
    candidate: LiveProgressCandidate | null,
    now: number,
  ) => StableProgressState;
  updateStableProgress: (
    state: StableProgressState,
    candidate: LiveProgressCandidate | null,
    now: number,
    urgent?: boolean,
  ) => StableProgressState;
  flushStableProgress: (state: StableProgressState, now: number) => StableProgressState;
};

const module = progress as StableProgressModule;

function candidate(
  id: LiveProgressCandidate["id"],
  label: string,
): LiveProgressCandidate {
  return { id, label, motion: "live" };
}
```

Then add:

```ts
test("delays the first visible stage by 240ms and keeps the latest candidate", () => {
  const initial = module.createStableProgressState(candidate("analyzing", "正在分析"), 0);
  const changed = module.updateStableProgress(initial, candidate("modifying", "正在进行修改"), 120);
  const early = module.flushStableProgress(changed, 239);
  const visible = module.flushStableProgress(early, 240);

  assert.equal(early.visible, null);
  assert.equal(visible.visible?.id, "modifying");
  assert.equal(visible.hasPresented, true);
});

test("suppresses a fast answer before progress was ever visible", () => {
  const initial = module.createStableProgressState(candidate("analyzing", "正在分析"), 0);
  const answering = module.updateStableProgress(
    initial,
    candidate("answering", "正在生成答复"),
    100,
  );

  assert.equal(answering.visible, null);
  assert.equal(answering.pending, null);
  assert.equal(answering.dueAt, null);
  assert.equal(answering.hasPresented, false);
});

test("waiting bypasses delay and pauses motion", () => {
  const initial = module.createStableProgressState(candidate("analyzing", "正在分析"), 0);
  const waiting = module.updateStableProgress(
    initial,
    { id: "waiting", label: "等待你的确认", motion: "paused", urgent: true },
    100,
    true,
  );

  assert.equal(waiting.visible?.id, "waiting");
  assert.equal(waiting.visible?.motion, "paused");
});
```

- [ ] **Step 4: Run focused tests and verify the new expectations fail**

```bash
node --test \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts
```

Expected: FAIL on old filename labels, missing `motion`, immediate initial visibility, and fast-answer suppression.

- [ ] **Step 5: Implement the finite stage mapping**

In `conversationProgress.ts`, define and use:

```ts
export const PROGRESS_INITIAL_DELAY_MS = 240;
export const PROGRESS_LABEL_MINIMUM_MS = 600;

export type LiveProgressStage =
  | "analyzing"
  | "discovering"
  | "modifying"
  | "verifying"
  | "answering"
  | "waiting";

export interface LiveProgressCandidate {
  id: LiveProgressStage;
  label: string;
  motion: "live" | "paused";
  urgent?: boolean;
}

const LIVE_STAGE: Record<LiveProgressStage, LiveProgressCandidate> = {
  analyzing: { id: "analyzing", label: "正在分析", motion: "live" },
  discovering: { id: "discovering", label: "正在查找相关内容", motion: "live" },
  modifying: { id: "modifying", label: "正在进行修改", motion: "live" },
  verifying: { id: "verifying", label: "正在验证结果", motion: "live" },
  answering: { id: "answering", label: "正在生成答复", motion: "live" },
  waiting: { id: "waiting", label: "等待你的确认", motion: "paused", urgent: true },
};

export function analyzingProgressCandidate() {
  return LIVE_STAGE.analyzing;
}

export function answeringProgressCandidate() {
  return LIVE_STAGE.answering;
}

export function waitingProgressCandidate() {
  return LIVE_STAGE.waiting;
}

export function progressCandidateForBlock(block: BlockState): LiveProgressCandidate {
  if (block.event_type === "text") return LIVE_STAGE.answering;
  if (block.event_type === "tool_call") {
    const name = typeof block.metadata.tool_name === "string" ? block.metadata.tool_name : "";
    if (["read_file", "read", "search_content", "grep", "search_files", "glob"].includes(name)) {
      return LIVE_STAGE.discovering;
    }
    if (["write_file", "write", "edit"].includes(name)) return LIVE_STAGE.modifying;
  }
  if (block.event_type === "diff_view") return LIVE_STAGE.modifying;
  if (block.event_type === "shell" && isVerificationCommand(block.metadata.command)) {
    return LIVE_STAGE.verifying;
  }
  return LIVE_STAGE.analyzing;
}

export function deriveLiveProgressCandidate(blocks: BlockState[]): LiveProgressCandidate | null {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if ((block.event_type === "tool_call" || block.event_type === "shell") && !block.isComplete) {
      return progressCandidateForBlock(block);
    }
    if (block.event_type === "text" && (block.content.trim() || !block.isComplete)) {
      return answeringProgressCandidate();
    }
    if ((block.event_type === "thinking" || block.event_type === "pending") && !block.isComplete) {
      return analyzingProgressCandidate();
    }
  }
  return null;
}

function isVerificationCommand(value: unknown) {
  return typeof value === "string"
    && /(?:^|\s|:)(build|test|check|lint|typecheck|tsc)(?:\s|$|:)/i.test(value);
}
```

Map thinking/pending to `analyzing`, allow-listed read/search tools to `discovering`, allow-listed edit/write tools and `diff_view` to `modifying`, verification shell commands to `verifying`, text to `answering`, and unresolved confirmation to `waiting`. `isVerificationCommand` accepts only the existing build/test/check/lint/typecheck patterns. Unknown activity falls back to `analyzing`; never derive label text from payloads.

`conversationProgress.ts` becomes the single owner of `LiveProgressCandidate`. Update `useStableProgressLabel.ts`, `useStableProgressLabel.test.ts`, and later `TurnProgress.tsx` to import the type from `conversationProgress.ts`; remove the old type-only dependency from progress back to `conversationTurnView.ts`.

- [ ] **Step 6: Implement initial-delay plus minimum-dwell state**

Change `StableProgressState` to:

```ts
export interface StableProgressState {
  visible: LiveProgressCandidate | null;
  visibleSince: number;
  pending: LiveProgressCandidate | null;
  dueAt: number | null;
  hasPresented: boolean;
}
```

Implement the transitions as:

```ts
export function createStableProgressState(
  candidate: LiveProgressCandidate | null,
  now: number,
): StableProgressState {
  if (!candidate || candidate.id === "answering") return emptyProgressState(now);
  if (candidate.urgent) return presentedProgressState(candidate, now);
  return {
    visible: null,
    visibleSince: now,
    pending: candidate,
    dueAt: now + PROGRESS_INITIAL_DELAY_MS,
    hasPresented: false,
  };
}

export function updateStableProgress(
  state: StableProgressState,
  candidate: LiveProgressCandidate | null,
  now: number,
  urgent = false,
): StableProgressState {
  if (!candidate) return emptyProgressState(now);
  if (urgent || candidate.urgent) return presentedProgressState(candidate, now);

  if (!state.hasPresented) {
    if (candidate.id === "answering") return emptyProgressState(now);
    return {
      ...state,
      pending: candidate,
      dueAt: state.dueAt ?? now + PROGRESS_INITIAL_DELAY_MS,
    };
  }

  if (!state.visible) return createStableProgressState(candidate, now);
  if (state.visible.id === candidate.id) {
    return { ...state, visible: candidate, pending: null, dueAt: null };
  }

  const dueAt = state.visibleSince + PROGRESS_LABEL_MINIMUM_MS;
  if (now >= dueAt) return presentedProgressState(candidate, now);
  return { ...state, pending: candidate, dueAt };
}

export function flushStableProgress(
  state: StableProgressState,
  now: number,
): StableProgressState {
  if (!state.pending || state.dueAt === null || now < state.dueAt) return state;
  return presentedProgressState(state.pending, now);
}

function emptyProgressState(now: number): StableProgressState {
  return {
    visible: null,
    visibleSince: now,
    pending: null,
    dueAt: null,
    hasPresented: false,
  };
}

function presentedProgressState(
  candidate: LiveProgressCandidate,
  now: number,
): StableProgressState {
  return {
    visible: candidate,
    visibleSince: now,
    pending: null,
    dueAt: null,
    hasPresented: true,
  };
}
```

Update `useStableProgressLabel.ts` to pass `candidate?.urgent === true` into `updateStableProgress` and keep its timer keyed by `dueAt` and pending identity.

- [ ] **Step 7: Run progress tests**

```bash
node --test \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts
```

Expected: all safe-label, delay, dwell, coalescing, suppression, and urgent-waiting tests PASS.

- [ ] **Step 8: Stage, inspect, and commit Task 2**

```bash
git add \
  apps/desktop/src/components/chat/conversationProgress.ts \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts
```

Run staged `detect_changes`, confirm only the live-progress derivation and hook are affected, then:

```bash
git commit -m "feat(desktop): stabilize high-level turn stages"
```

---

### Task 3: Derive terminal summaries and meaningful operation groups

**Files:**

- Modify: `apps/desktop/src/components/chat/conversationTurnView.ts:1-176`
- Modify: `apps/desktop/src/components/chat/conversationTurnView.test.ts:1-177`
- Modify: `apps/desktop/src/components/chat/messageGrouping.ts:1-20`

- [ ] **Step 1: Run impact analysis for `deriveConversationTurnView`**

Run the exact-file GitNexus impact call. Expected indexed risk: LOW, with manual residual risk because `ConversationTurn` imports it and the TSX call chain is incompletely indexed.

- [ ] **Step 2: Write failing projection tests for streaming and terminal states**

Extend the view test return type with `terminalSummary` and use these assertions:

```ts
test("keeps one answering stage while final text streams", () => {
  const view = deriveConversationTurnView!(conversationTurn([
    timedUser("user", "整理页面", 1_000),
    block("thinking", "thinking", "private"),
    incompleteBlock("answer", "text", "正在输出结果"),
  ]));

  assert.equal(view.finalAnswer?.block_id, "answer");
  assert.deepEqual(view.liveProgress, {
    id: "answering",
    label: "正在生成答复",
    motion: "live",
  });
  assert.equal(view.terminalSummary, null);
});

test("derives duration, outcome, and grouped operation count", () => {
  const view = deriveConversationTurnView!(conversationTurn([
    timedUser("user", "整理页面", 1_000, 13_250, "completed"),
    block("thinking", "thinking", "private"),
    block("read-a", "tool_call", "", { tool_name: "read_file" }),
    block("read-b", "tool_call", "", { tool_name: "search_content" }),
    block("edit-a", "tool_call", "", { tool_name: "edit" }),
    block("diff-a", "diff_view", "", { file_path: "/repo/App.tsx" }),
    block("check-a", "shell", "ok", { command: "npm test", exit_code: 0 }),
    block("answer", "text", "已经完成。"),
  ]));

  assert.deepEqual(view.terminalSummary, {
    outcome: "completed",
    durationMs: 12_250,
    operationCount: 3,
  });
  assert.deepEqual(view.processDigest.items.map((item) => item.label), [
    "分析需求",
    "完成修改",
    "验证结果",
  ]);
  assert.equal(JSON.stringify(view.processDigest).includes("App.tsx"), false);
});

test("derives stopped and failed terminal summaries without completed semantics", () => {
  const stopped = deriveConversationTurnView!(conversationTurn([
    timedUser("stop", "停止", 1_000, 9_000, "stopped"),
  ]));
  const failed = deriveConversationTurnView!(conversationTurn([
    timedUser("fail", "检查", 2_000, 14_000, "failed"),
    block("shell", "shell", "failed", { command: "npm test", exit_code: 1 }),
    block("error", "error", "检查没有完成"),
  ]));

  assert.equal(stopped.terminalSummary?.outcome, "stopped");
  assert.equal(failed.terminalSummary?.outcome, "failed");
  assert.equal(failed.processDigest.items.at(-1)?.outcome, "failed");
});
```

Add this fixture beside the existing block helpers:

```ts
function timedUser(
  blockId: string,
  content: string,
  startedAtMs: number,
  terminalAtMs?: number,
  outcome?: "completed" | "stopped" | "failed",
): BlockState {
  return {
    block_id: blockId,
    event_type: "user_message",
    content,
    isComplete: true,
    metadata: {
      turn_started_at_ms: startedAtMs,
      ...(terminalAtMs === undefined ? {} : { turn_terminal_at_ms: terminalAtMs }),
      ...(outcome === undefined ? {} : { turn_outcome: outcome }),
    },
  };
}
```

- [ ] **Step 3: Run the projection tests and verify failure**

```bash
node --test apps/desktop/src/components/chat/conversationTurnView.test.ts
```

Expected: FAIL because streaming text currently clears progress, terminal summary is absent, and digest labels still expose object names.

- [ ] **Step 4: Implement presentation-safe view types**

Use these public types in `conversationTurnView.ts`:

```ts
export type ProcessDigestKind = "analysis" | "modification" | "verification" | "exception";

export interface ProcessDigestItem {
  id: string;
  kind: ProcessDigestKind;
  label: string;
  outcome: "running" | "done" | "stopped" | "failed";
  evidence: BlockState[];
}

export interface ProcessDigest {
  items: ProcessDigestItem[];
  operationCount: number;
  usage: BlockState[];
  delivery: BlockState | null;
}

export interface TurnTerminalSummary {
  outcome: "completed" | "stopped" | "failed";
  durationMs: number | null;
  operationCount: number;
}
```

Import `LiveProgressCandidate` from `conversationProgress.ts` and re-export that type from `conversationTurnView.ts` so the existing `messageGrouping.ts` compatibility export remains valid.

Derive `terminalSummary` from `readConversationTurnTiming(userMessage)` and the grouped digest. When no authoritative outcome has arrived yet, a complete final text may produce a provisional `completed` summary with `durationMs: null` only when no incomplete tool, shell, thinking, or confirmation block follows that text. A later tool round therefore restores live progress instead of leaving a false completed footer. Never fabricate a duration.

- [ ] **Step 5: Implement compact operation grouping**

Normalize raw blocks to safe stage groups:

```ts
thinking/pending/read/search -> analysis / "分析需求"
write/edit/diff_view          -> modification / "完成修改"
verification shell           -> verification / "验证结果"
failed shell/tool/error       -> matching safe stage with outcome "failed"
```

Merge consecutive groups of the same kind and outcome, concatenating their `evidence` arrays. Compute `operationCount` before compacting the visible list: count discovery/read/search groups, modification groups, and verification groups; do not count pure thinking/pending, answer streaming, confirmations, usage, or delivery metadata. Keep at most four visible groups; preserve the first analysis, latest modification, latest verification, and any failure. Preserve `provider_usage` and `delivery_summary` in `usage` and `delivery`, but never include raw evidence or payload-derived text in public stage labels.

When a terminal summary exists, convert any remaining `running` group to `stopped` for a stopped turn or `failed` for a failed turn. A completed turn may show only groups that actually completed; do not label an incomplete group as done.

Use an internal normalized shape and category compaction:

```ts
interface DigestGroup extends ProcessDigestItem {
  countsAsOperation: boolean;
}

function deriveProcessDigest(
  blocks: BlockState[],
  terminalOutcome: TurnTerminalSummary["outcome"] | null,
): ProcessDigest {
  const groups: DigestGroup[] = [];
  const usage: BlockState[] = [];
  let delivery: BlockState | null = null;

  for (const block of blocks) {
    if (block.event_type === "provider_usage") {
      usage.push(block);
      continue;
    }
    if (block.event_type === "delivery_summary") {
      delivery = block;
      continue;
    }

    const candidate = digestGroupForBlock(block);
    if (!candidate) continue;
    const previous = groups.at(-1);
    if (previous && previous.kind === candidate.kind && previous.outcome === candidate.outcome) {
      previous.evidence.push(...candidate.evidence);
      previous.countsAsOperation ||= candidate.countsAsOperation;
    } else {
      groups.push(candidate);
    }
  }

  const operationCount = groups.filter((group) => group.countsAsOperation).length;
  const summaryByKind = new Map<ProcessDigestKind, ProcessDigestItem>();
  for (const group of groups) {
    const outcome = terminalizedOutcome(group.outcome, terminalOutcome);
    if (!outcome) continue;
    const existing = summaryByKind.get(group.kind);
    if (!existing) {
      summaryByKind.set(group.kind, { ...group, outcome, evidence: [...group.evidence] });
      continue;
    }
    existing.evidence.push(...group.evidence);
    existing.outcome = strongerOutcome(existing.outcome, outcome);
  }

  return {
    items: [...summaryByKind.values()].slice(0, 4),
    operationCount,
    usage,
    delivery,
  };
}

function digestGroupForBlock(block: BlockState): DigestGroup | null {
  const outcome = blockFailed(block) ? "failed" : block.isComplete ? "done" : "running";
  if (block.event_type === "thinking" || block.event_type === "pending") {
    return digestGroup(block, "analysis", "分析需求", outcome, false);
  }
  if (block.event_type === "diff_view") {
    return digestGroup(block, "modification", "完成修改", outcome, true);
  }
  if (block.event_type === "shell") {
    return isVerificationCommand(block.metadata.command)
      ? digestGroup(block, "verification", "验证结果", outcome, true)
      : digestGroup(block, "analysis", "分析需求", outcome, true);
  }
  if (block.event_type === "tool_call" || block.event_type === "tool_call_result") {
    const name = typeof block.metadata.tool_name === "string" ? block.metadata.tool_name : "";
    const modification = ["write_file", "write", "edit"].includes(name);
    return modification
      ? digestGroup(block, "modification", "完成修改", outcome, true)
      : digestGroup(block, "analysis", "分析需求", outcome, true);
  }
  if (block.event_type === "error") {
    return digestGroup(block, "exception", "处理异常", "failed", false);
  }
  if (block.event_type.startsWith("context_compact")) {
    return digestGroup(block, "analysis", "分析需求", outcome, false);
  }
  return null;
}

function digestGroup(
  block: BlockState,
  kind: ProcessDigestKind,
  label: string,
  outcome: ProcessDigestItem["outcome"],
  countsAsOperation: boolean,
): DigestGroup {
  return {
    id: `${kind}-${block.block_id}`,
    kind,
    label,
    outcome,
    evidence: [block],
    countsAsOperation,
  };
}

function terminalizedOutcome(
  outcome: ProcessDigestItem["outcome"],
  terminal: TurnTerminalSummary["outcome"] | null,
): ProcessDigestItem["outcome"] | null {
  if (outcome !== "running") return outcome;
  if (terminal === "stopped") return "stopped";
  if (terminal === "failed") return "failed";
  if (terminal === "completed") return null;
  return outcome;
}

function strongerOutcome(
  left: ProcessDigestItem["outcome"],
  right: ProcessDigestItem["outcome"],
): ProcessDigestItem["outcome"] {
  const rank = { done: 0, running: 1, stopped: 2, failed: 3 } as const;
  return rank[right] > rank[left] ? right : left;
}
```

Keep the existing `blockFailed` and `isVerificationCommand` safety helpers, but remove payload-derived label helpers.

- [ ] **Step 6: Keep progress during streaming and waiting**

In `deriveConversationTurnView`, derive timing before digest terminalization:

```ts
const timing = readConversationTurnTiming(userMessage);
const processDigest = deriveProcessDigest(blocks, timing.outcome);
const terminalSummary = deriveTerminalSummary(userMessage, finalAnswer, processDigest, blocks);
const liveProgress = terminalSummary
  ? null
  : interruptions.length > 0
    ? waitingProgressCandidate()
    : deriveLiveProgressCandidate(blocks);
```

Implement terminal fallback without inventing duration:

```ts
function deriveTerminalSummary(
  userMessage: BlockState | null,
  finalAnswer: BlockState | null,
  digest: ProcessDigest,
  blocks: BlockState[],
): TurnTerminalSummary | null {
  const timing = readConversationTurnTiming(userMessage);
  if (timing.outcome) {
    return {
      outcome: timing.outcome,
      durationMs: timing.durationMs,
      operationCount: digest.operationCount,
    };
  }

  if (!finalAnswer?.isComplete || hasProcessActivityAfter(blocks, finalAnswer)) return null;
  return {
    outcome: "completed",
    durationMs: null,
    operationCount: digest.operationCount,
  };
}

function hasProcessActivityAfter(blocks: BlockState[], finalAnswer: BlockState) {
  const answerIndex = blocks.lastIndexOf(finalAnswer);
  return blocks.slice(answerIndex + 1).some((block) => (
    block.event_type === "thinking"
    || block.event_type === "pending"
    || block.event_type === "tool_call"
    || block.event_type === "shell"
    || block.event_type === "confirm_ask"
  ));
}
```

`deriveLiveProgressCandidate` scans newest to oldest, so a later incomplete tool or shell takes precedence over earlier text. Otherwise complete or streaming nonterminal text remains `正在生成答复` until a provisional or authoritative terminal summary exists.

- [ ] **Step 7: Run all chat projection tests**

```bash
node --test \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts \
  apps/desktop/src/components/chat/conversationTurnView.test.ts
```

Expected: all tests PASS; no public stage label contains filenames or commands, second-level evidence arrays remain intact, and no turn exposes more than four first-level summary stages.

- [ ] **Step 8: Stage, inspect, and commit Task 3**

```bash
git add \
  apps/desktop/src/components/chat/conversationTurnView.ts \
  apps/desktop/src/components/chat/conversationTurnView.test.ts \
  apps/desktop/src/components/chat/messageGrouping.ts
```

Run staged `detect_changes`, inspect the conversation projection boundary, then:

```bash
git commit -m "feat(desktop): summarize terminal turn progress"
```

---

### Task 4: Resolve live progress into the compact answer footer

**Files:**

- Modify: `apps/desktop/src/components/chat/TurnProgress.tsx:1-25`
- Modify: `apps/desktop/src/components/chat/ConversationTurn.tsx:1-67`
- Modify: `apps/desktop/src/components/chat/ConversationProcessDisclosure.tsx:1-155`
- Modify: `apps/desktop/src/components/chat/ConversationProcessItem.tsx:1-83`
- Delete: `apps/desktop/src/components/chat/conversationProcessTarget.ts`
- Delete: `apps/desktop/src/components/chat/conversationProcessTarget.test.ts`
- Modify: `apps/desktop/src/styles/conversation-turn.css:1-357`
- Modify: `apps/desktop/scripts/check-conversation-style.mjs:1-180`

- [ ] **Step 1: Record TSX impact fallback and direct call chain**

Attempt exact-file `impact` calls for `ConversationTurn`, `TurnProgress`, `ConversationProcessDisclosure`, and `ConversationProcessItem`. If unresolved, record:

```text
ConversationLane -> ConversationTurn
ConversationTurn -> TurnProgress
ConversationTurn -> ConversationProcessDisclosure
ConversationProcessDisclosure -> ConversationProcessItem
```

Also record selected evidence: chat unit tests, style gate, `messages.spec.ts`, `acceptance.spec.ts`, build, and precommit.

- [ ] **Step 2: Add failing static style-contract assertions**

In `check-conversation-style.mjs`, read `src/styles/conversation-turn.css` and add exact assertions:

```js
// In the files map:
conversationTurnComponent: read("src/components/chat/ConversationTurn.tsx"),
conversationTurnCss: read("src/styles/conversation-turn.css"),

// Update the two existing ConversationTurn assertions to use
// files.conversationTurnComponent, then add:
const progressBlock = selectorBlock(files.conversationTurnCss, ".forge-turn-progress");
assertIncludes(progressBlock, "min-height: 22px;", "live progress compact height");
assertNotIncludes(progressBlock, "background:", "live progress has no card background");
assertNotIncludes(progressBlock, "border:", "live progress has no card border");
assertIncludes(files.conversationTurnCss, "@media (prefers-reduced-motion: reduce)", "live progress reduced motion");
assertIncludes(files.conversationTurnComponent, 'data-progress-motion={visible.motion}', "progress motion state marker");
```

Use the two distinct keys so the component source and CSS source cannot be confused.

- [ ] **Step 3: Run the style gate and verify failure**

```bash
npm --prefix apps/desktop run check:conversation-style
```

Expected: FAIL on 22px height and the missing progress motion marker.

- [ ] **Step 4: Update the live row markup**

Render `TurnProgress` from the stabilized visible candidate only:

```tsx
export function TurnProgress({ candidate }: { candidate: LiveProgressCandidate | null }) {
  const visible = useStableProgressLabel(candidate);
  if (!visible) return null;

  return (
    <div
      data-testid="conversation-progress"
      data-progress-id={visible.id}
      data-progress-motion={visible.motion}
      className="forge-turn-progress"
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      <span aria-hidden="true" className="forge-turn-progress-dot" />
      <span key={visible.id} className="forge-turn-progress-label">{visible.label}</span>
      <span aria-hidden="true" className="forge-turn-progress-track">
        <span className="forge-turn-progress-trace" />
      </span>
    </div>
  );
}
```

Paused waiting state must stop both animations through `[data-progress-motion="paused"]` CSS.

- [ ] **Step 5: Make `ConversationTurn` render streaming progress and terminal footer independently**

Use these render conditions:

```tsx
const primaryResult = view.finalAnswer ?? view.terminalError;
const showTerminalFooter = Boolean(view.terminalSummary);

<TurnProgress candidate={view.liveProgress} />

{primaryResult && (
  <PrimaryBlock
    block={primaryResult}
    role={view.finalAnswer ? "assistant" : "artifact"}
    sessionId={sessionId}
  />
)}

{showTerminalFooter && (
  <ConversationProcessDisclosure
    digest={view.processDigest}
    terminal={view.terminalSummary!}
    sessionId={sessionId}
  />
)}
```

Do not require `primaryResult` for stopped turns. Include terminal summary in the early-return and assistant-rail conditions.

- [ ] **Step 6: Simplify the terminal disclosure**

`ConversationProcessDisclosure` accepts `digest`, `terminal`, and the existing optional `sessionId` used by second-level evidence renderers. Its first expansion renders only `digest.items`. Remove the next-action button. When `digest.usage` or `digest.delivery` exists, place the existing metadata renderers behind a second closed-by-default `ForgeCollapsible` trigger labeled `运行证据` and `data-testid="conversation-evidence-trigger"`; no usage or delivery row appears until that second disclosure opens.

Use exact footer formatting:

```ts
function terminalLabel(terminal: TurnTerminalSummary) {
  const status = terminal.outcome === "completed"
    ? "已完成"
    : terminal.outcome === "stopped"
      ? "已停止"
      : "未完成";
  const duration = terminal.durationMs === null ? null : formatTurnDuration(terminal.durationMs);
  const operations = terminal.operationCount > 0 ? `${terminal.operationCount} 项操作` : null;
  return [status, duration, operations].filter(Boolean).join(" · ");
}

function formatTurnDuration(durationMs: number) {
  if (durationMs < 1_000) return "<1 秒";
  const totalSeconds = Math.floor(durationMs / 1_000);
  if (totalSeconds < 60) return `${totalSeconds} 秒`;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return seconds === 0 ? `${minutes} 分钟` : `${minutes} 分 ${seconds} 秒`;
}
```

The terminal trigger uses `aria-expanded`, `aria-controls`, an outcome-specific text label, and one chevron. Its first expansion renders only `digest.items` plus the single collapsed `运行证据` control when secondary evidence exists.

If `digest.items`, `digest.usage`, and `digest.delivery` are all empty, render the same terminal label as a non-interactive `div` with `data-testid="conversation-process-status"` and `className="forge-process-status"`; do not render a button that expands an empty region. This is the only no-detail exception to the disclosure-button rule.

- [ ] **Step 7: Simplify stage items and remove obsolete target files**

`ConversationProcessItem.tsx` keeps raw blocks behind a second-level evidence disclosure but removes the Work Panel target action. Use:

```tsx
import { useState } from "react";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import type { ProcessDigestItem } from "@/components/chat/conversationTurnView";
import {
  ForgeCollapsible,
  ForgeCollapsibleContent,
  ForgeCollapsibleTrigger,
} from "@/components/primitives/collapsible";

export function ConversationProcessItem({
  item,
  sessionId,
}: {
  item: ProcessDigestItem;
  sessionId?: string;
}) {
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  return (
    <li
      data-testid="conversation-process-item"
      data-process-kind={item.kind}
      className="forge-process-digest-item"
    >
      <div className="forge-process-digest-row">
        <span
          aria-hidden="true"
          className="forge-process-digest-node"
          data-outcome={item.outcome}
        />
        <span className="forge-process-digest-label">{item.label}</span>
        <span className="forge-process-digest-outcome">
          {item.outcome === "failed"
            ? "失败"
            : item.outcome === "stopped"
              ? "已停止"
              : item.outcome === "running"
                ? "进行中"
                : "完成"}
        </span>
      </div>
      {item.evidence.length > 0 && (
        <ForgeCollapsible open={evidenceOpen} onOpenChange={setEvidenceOpen}>
          <ForgeCollapsibleTrigger
            type="button"
            aria-label={`${evidenceOpen ? "收起" : "查看"} ${item.label} 运行证据`}
            className="forge-process-detail-trigger"
          >
            <span aria-hidden="true" data-open={evidenceOpen ? "true" : "false"}>›</span>
            {evidenceOpen ? "收起证据" : "查看证据"}
          </ForgeCollapsibleTrigger>
          {evidenceOpen && (
            <ForgeCollapsibleContent>
              <div data-testid="conversation-process-details" className="forge-process-detail-content">
                {item.evidence.map((block, index) => (
                  <MemoizedBlockRenderer
                    key={`${block.block_id}-${block.event_type}-${index}`}
                    block={block}
                    sessionId={sessionId}
                  />
                ))}
              </div>
            </ForgeCollapsibleContent>
          )}
        </ForgeCollapsible>
      )}
    </li>
  );
}
```

Delete `conversationProcessTarget.ts` and its test after confirming `rg -n "deriveConversationProcessTarget" apps/desktop/src` returns only those two files.

- [ ] **Step 8: Implement the restrained CSS contract**

Update `conversation-turn.css` so:

```css
.forge-turn-progress {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-height: 22px;
  margin-top: 0.5rem;
  padding-inline: 0.125rem;
  color: var(--forge-text-muted);
  font-size: 12px;
  line-height: 18px;
}

.forge-turn-progress-track {
  position: relative;
  width: 32px;
  height: 1px;
  flex: 0 0 32px;
  overflow: hidden;
  background: var(--forge-border-subtle);
}

.forge-turn-progress[data-progress-motion="paused"] .forge-turn-progress-dot,
.forge-turn-progress[data-progress-motion="paused"] .forge-turn-progress-trace {
  animation: none;
}

.forge-process-disclosure,
.forge-process-status {
  animation: forge-turn-terminal-in 160ms ease-out both;
}

@keyframes forge-turn-terminal-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .forge-turn-progress-dot,
  .forge-turn-progress-label,
  .forge-turn-progress-trace,
  .forge-process-disclosure,
  .forge-process-status {
    animation: none;
  }

  .forge-turn-progress-dot {
    opacity: 1;
    box-shadow: none;
  }

  .forge-turn-progress-trace {
    left: 0;
    width: 100%;
    opacity: 0.32;
    transform: none;
  }
}
```

Keep the existing 1.8s dot, 1.6s trace, 120ms label, and add the approved 160ms terminal transition. Remove CSS for the target and next-action controls. Keep nested evidence CSS, but make its trigger visually quiet and hidden until the stage row is hovered or keyboard focus enters it; keep it visible under touch/coarse-pointer media. Keep usage/delivery metadata inside the closed `运行证据` region. Do not modify theme tokens.

- [ ] **Step 9: Run static, unit, and TypeScript build checks**

```bash
npm --prefix apps/desktop run check:conversation-style
node --test \
  apps/desktop/src/lib/conversationTurnTiming.test.ts \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts \
  apps/desktop/src/components/chat/conversationTurnView.test.ts
npm --prefix apps/desktop run build
```

Expected: style gate, unit tests, TypeScript, and Vite build PASS.

- [ ] **Step 10: Stage, inspect, and commit Task 4**

Stage exactly the six modified component/style-contract files plus the two deleted target files. Run staged `detect_changes`, verify that Work Panel flows are no longer called from the process item but Work Panel itself is unchanged, then:

```bash
git commit -m "feat(desktop): resolve progress into answer footer"
```

---

### Task 5: Prove the complete lifecycle in Playwright

**Files:**

- Modify: `apps/desktop/e2e/messages.spec.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [ ] **Step 1: Add a failing detailed message lifecycle test**

Add a test that:

```ts
test("conversation live progress stays singular, safe, and resolves inline", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await page.addInitScript((id) => {
    // @ts-expect-error mock
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.waitForFunction(() => (window as any).__tauriListeners?.["session-output"]?.length > 0);

  await page.locator("textarea").fill("整理这个页面");
  await page.locator("textarea").press("Enter");
  await simulateStream(page, sessionId, [{
    event_type: "tool_call_start",
    session_id: sessionId,
    block_id: "safe-read",
    tool_name: "read_file",
    tool_input: { path: "/repo/private/AppShell.tsx", token: "never-show-this" },
  }], 1);

  const progress = page.getByTestId("conversation-progress");
  await expect(progress).toHaveCount(1);
  await expect(progress).toContainText("正在查找相关内容");
  await expect(progress).not.toContainText("AppShell.tsx");
  await expect(progress).not.toContainText("never-show-this");

  await simulateStream(page, sessionId, [
    {
      event_type: "tool_call_result",
      session_id: sessionId,
      block_id: "safe-read",
      result: "ok",
      is_error: false,
      duration_ms: 20,
    },
    { event_type: "text_start", session_id: sessionId, block_id: "final" },
    { event_type: "text_chunk", session_id: sessionId, block_id: "final", content: "已经整理完成。" },
  ], 1);
  await expect(progress).toContainText("正在生成答复");
  await expect(page.getByTestId("assistant-message")).toContainText("已经整理完成");

  await simulateStream(page, sessionId, [
    { event_type: "text_end", session_id: sessionId, block_id: "final" },
    {
      event_type: "agent_turn_updated",
      session_id: sessionId,
      state: completedTurnProjection(sessionId, 1),
    },
  ], 1);

  await expect(progress).toHaveCount(0);
  const trigger = page.getByTestId("conversation-process-trigger");
  await expect(trigger).toContainText(/^已完成 · (?:<1 秒|\d+ 秒) · 1 项操作/);
  await trigger.click();
  await expect(page.getByTestId("conversation-process-item")).toHaveCount(1);
  await expect(page.getByText("分析需求", { exact: true })).toBeVisible();
  await expect(page.getByTestId("conversation-process-details")).toHaveCount(0);
});
```

Add the type import and fixtures:

```ts
import type { AgentTurnProjection } from "../src/lib/protocol";

function completedTurnProjection(
  sessionId: string,
  toolCallCount: number,
): AgentTurnProjection {
  return {
    session_id: sessionId,
    status: "completed",
    step_label: "已完成",
    workspace_path: "/workspace",
    compact_count: 0,
    verification_status: "passed",
    model_rounds: 1,
    tool_call_count: toolCallCount,
    failed_tool_count: 0,
    estimated_context_tokens: null,
    stop_reason: null,
    compact_saved_tokens: 0,
  };
}

async function startConversationTurn(
  page: import("@playwright/test").Page,
  sessionId: string,
  prompt: string,
) {
  await page.addInitScript((id) => {
    // @ts-expect-error mock
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error mock listener registry
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });
  await page.locator("textarea").fill(prompt);
  await page.locator("textarea").press("Enter");
}
```

- [ ] **Step 2: Add failure, stop, and reduced-motion tests**

Add these focused tests:

```ts
test("conversation terminal footer distinguishes failure and stop", async ({ page }) => {
  const failedSession = crypto.randomUUID();
  await startConversationTurn(page, failedSession, "运行检查");
  await simulateStream(page, failedSession, [
    { event_type: "error", session_id: failedSession, block_id: "failed", message: "检查没有完成", code: "check_failed" },
    {
      event_type: "agent_turn_updated",
      session_id: failedSession,
      state: {
        ...completedTurnProjection(failedSession, 1),
        status: "failed",
        step_label: "未完成",
        verification_status: "failed",
        failed_tool_count: 1,
      },
    },
  ], 1);
  await expect(page.getByTestId("conversation-process-trigger")).toContainText(/^未完成/);

  const stoppedSession = crypto.randomUUID();
  await startConversationTurn(page, stoppedSession, "停止这轮");
  await simulateStream(page, stoppedSession, [
    { event_type: "session_stopped", session_id: stoppedSession, reason: "user" },
  ], 1);
  await expect(page.getByTestId("conversation-process-trigger")).toContainText(/^已停止/);
});

test("reduced motion and waiting keep state without ambient loops", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const sessionId = crypto.randomUUID();
  await startConversationTurn(page, sessionId, "修改设置");
  await simulateStream(page, sessionId, [
    { event_type: "thinking_start", session_id: sessionId, block_id: "thinking" },
  ], 1);

  const progress = page.getByTestId("conversation-progress");
  await expect(progress).toBeVisible();
  const animationNames = await progress.evaluate((node) => ({
    dot: getComputedStyle(node.querySelector(".forge-turn-progress-dot")!).animationName,
    trace: getComputedStyle(node.querySelector(".forge-turn-progress-trace")!).animationName,
  }));
  expect(animationNames).toEqual({ dot: "none", trace: "none" });

  await simulateStream(page, sessionId, [{
    event_type: "confirm_ask",
    session_id: sessionId,
    block_id: "confirm",
    question: "允许修改？",
    kind: "write_file",
  }], 1);
  await expect(progress).toHaveAttribute("data-progress-motion", "paused");
  await expect(progress).toContainText("等待你的确认");
});
```

Do not assert theme colors.

- [ ] **Step 3: Add one product acceptance smoke**

In `acceptance.spec.ts`, add:

```ts
test("结果优先对话会把实时阶段收束为可展开的完成摘要", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await page.evaluate((id) => {
    // @ts-expect-error acceptance mock
    window.__mockSessionId = id;
  }, sessionId);
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error acceptance mock
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });
  await page.locator("textarea").fill("整理工作区");
  await page.locator("textarea").press("Enter");
  await simulateStream(page, sessionId, [{
    event_type: "tool_call_start",
    session_id: sessionId,
    block_id: "acceptance-read",
    tool_name: "read_file",
    tool_input: { path: "/repo/private/AppShell.tsx" },
  }], 1);

  await expect(page.getByTestId("conversation-progress")).toContainText("正在查找相关内容");
  await simulateStream(page, sessionId, [
    {
      event_type: "tool_call_result",
      session_id: sessionId,
      block_id: "acceptance-read",
      result: "ok",
      is_error: false,
      duration_ms: 20,
    },
    { event_type: "text_start", session_id: sessionId, block_id: "acceptance-result" },
    { event_type: "text_chunk", session_id: sessionId, block_id: "acceptance-result", content: "工作区已经整理完成。" },
    { event_type: "text_end", session_id: sessionId, block_id: "acceptance-result" },
    {
      event_type: "agent_turn_updated",
      session_id: sessionId,
      state: {
        session_id: sessionId,
        status: "completed",
        step_label: "已完成",
        workspace_path: "/workspace",
        compact_count: 0,
        verification_status: "passed",
        model_rounds: 1,
        tool_call_count: 1,
        failed_tool_count: 0,
        estimated_context_tokens: null,
        stop_reason: null,
        compact_saved_tokens: 0,
      },
    },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  const trigger = turn.getByTestId("conversation-process-trigger");
  await expect(trigger).toContainText(/^已完成 · (?:<1 秒|\d+ 秒) · 1 项操作/);
  await trigger.click();
  await expect(turn.getByTestId("conversation-process-item")).toHaveCount(1);
  await expect(turn).not.toContainText("AppShell.tsx");
});
```

- [ ] **Step 4: Run the new tests and verify failure before the UI task is applied**

When executing TDD task-by-task, run the new tests before Task 4 implementation and expect failures on safe labels, answering persistence, footer duration, and first-level evidence collapse. If Task 4 is already applied in the same execution batch, confirm the test diff itself fails against the Task 3 commit using a temporary staged snapshot rather than reverting user work.

- [ ] **Step 5: Update existing result-first regressions without dropping second-level evidence coverage**

Replace `revealLatestProcessDetails` with:

```ts
async function revealLatestProcessDetails(page: import("@playwright/test").Page) {
  const disclosure = await openLatestProcess(page);
  const metadataTrigger = disclosure.getByTestId("conversation-evidence-trigger");
  if (await metadataTrigger.count()) await metadataTrigger.click();
  const detailTriggers = disclosure.getByRole("button", { name: /^查看 .* 运行证据$/ });
  for (let remaining = await detailTriggers.count(); remaining > 0; remaining -= 1) {
    await detailTriggers.first().click();
  }
  return disclosure;
}
```

Update existing expectations deliberately:

```text
"已理解任务"                    -> "分析需求"
fullConversation operation count     -> remains 2 (modification + verification; pure thinking is not an operation)
"已整理上下文"                 -> "分析需求"
provider usage metadata tests         -> click conversation-evidence-trigger first
raw tool, shell, diff preview tests    -> keep using revealLatestProcessDetails
conversation-next-action expectations  -> assert count 0; next-action copy remains in persisted delivery evidence
```

Do not delete the rich-preview assertions at lines currently reached through `revealLatestProcessDetails`; they prove raw evidence still exists behind the second level while the first expansion remains clean.

For each provider-usage case, insert the second-level open before the existing model-usage action:

```ts
await disclosure.getByTestId("conversation-evidence-trigger").click();
await disclosure.getByRole("button", { name: "查看模型用量" }).click();
```

- [ ] **Step 6: Run focused Playwright suites**

```bash
npm --prefix apps/desktop run test:e2e -- \
  e2e/messages.spec.ts \
  e2e/acceptance.spec.ts \
  --grep "live progress|结果优先对话|timeline messages|hidden work structure"
```

Expected: all focused lifecycle and existing result-first regression tests PASS.

- [ ] **Step 7: Run the full relevant suites**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts e2e/acceptance.spec.ts
```

Expected: both complete specs PASS with no theme-coupled assertion changes caused by this feature.

- [ ] **Step 8: Stage, inspect, and commit Task 5**

```bash
git add apps/desktop/e2e/messages.spec.ts apps/desktop/e2e/acceptance.spec.ts
```

Run staged `detect_changes`, then:

```bash
git commit -m "test(desktop): cover live conversation progress"
```

---

### Task 6: Synchronize product documentation and final verification

**Files:**

- Modify: `README.md:84`
- Modify: `apps/desktop/README.md:62`
- Modify: `CHANGELOG.md:10`

- [ ] **Step 1: Update the root product-surface description**

Replace the result-first row with exactly:

```markdown
| 结果优先对话 | `apps/desktop/src/components/chat/`：每轮主路径只保留用户消息、一个延迟出现且稳定切换的高层阶段和最终结果；进度不会暴露文件名、命令或内部思考。答案完成后，同一状态收束为带整轮耗时和有效操作数的页脚，点击只在原位展开 2–4 个关键阶段；等待、停止与失败保持真实语义。 |
```

- [ ] **Step 2: Update the desktop README behavior**

Replace the existing desktop conversation bullet with:

```markdown
- 在桌面 UI 中以“用户消息 → 一个高层实时阶段 → 最终结果”呈现每轮任务：进度延迟出现、稳定切换，并只使用“正在分析 / 查找相关内容 / 进行修改 / 验证结果 / 生成答复”等用户语言，不显示文件名、命令或内部思考。完成后收束为“已完成 · 耗时 · 有效操作数”，点击只在答案下方展开 2–4 个关键阶段；等待确认、主动停止和失败使用各自准确状态。
```

- [ ] **Step 3: Update the changelog**

Add an Unreleased item:

```markdown
- Refined desktop conversation progress into one continuous status line: a 240ms anti-flash delay, 600ms stage stability, safe high-level labels, restrained dot-and-trace motion, answer-stream continuity, honest completed/stopped/failed footers with turn duration and meaningful operation count, and an inline two-to-four-stage summary with no raw execution payloads.
```

- [ ] **Step 4: Verify the acceptance matrix remains aligned**

Run:

```bash
scripts/acceptance.sh --dry-run
```

Expected: exit 0 and the existing `completion contract mocked desktop smoke` gate still advertises `e2e/acceptance.spec.ts`. Do not modify `scripts/acceptance.sh` because the advertised command already covers the new acceptance test.

- [ ] **Step 5: Run the complete verification set**

```bash
node --test \
  apps/desktop/src/lib/conversationTurnTiming.test.ts \
  apps/desktop/src/store/event-dispatch.test.ts \
  apps/desktop/src/store/blocks.test.ts \
  apps/desktop/src/components/chat/conversationProgress.test.ts \
  apps/desktop/src/components/chat/useStableProgressLabel.test.ts \
  apps/desktop/src/components/chat/conversationTurnView.test.ts
npm --prefix apps/desktop run check:conversation-style
npm --prefix apps/desktop run build
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts e2e/acceptance.spec.ts
npm --prefix apps/desktop run check:precommit
scripts/acceptance.sh --dry-run
```

Expected: every command exits 0. Record test counts from Node and Playwright rather than reporting only that commands were run.

- [ ] **Step 6: Inspect the rendered desktop behavior**

Run the desktop frontend, open a new conversation, and verify at normal and reduced motion:

```text
one live row only
no filename or command in the row
dot + trace are subtle at normal motion
answer streams while "正在生成答复" remains visible
footer includes outcome, duration, and meaningful operation count
inline expansion contains no more than four safe stages
stopped and failed turns never display completed semantics
reduced motion keeps static state information and no loop
```

Capture screenshots only for verification; do not introduce theme changes in response to unrelated visual differences from the user's dirty V5 worktree.

- [ ] **Step 7: Stage only documentation, inspect final scope, and commit**

```bash
git add README.md apps/desktop/README.md CHANGELOG.md
```

Run staged `detect_changes`, then commit:

```bash
git commit -m "docs(desktop): document live conversation progress"
```

- [ ] **Step 8: Run final branch-scope GitNexus review**

Run:

```text
detect_changes({
  repo: "forge",
  scope: "compare",
  base_ref: "main",
  worktree: "/Users/cabbos/project/forge"
})
```

Review every changed conversation and Store flow. Confirm no unrelated theme files were staged or committed. Report residual risk from the CRITICAL Store factories and the exact Store, unit, build, and E2E evidence that mitigates it.
