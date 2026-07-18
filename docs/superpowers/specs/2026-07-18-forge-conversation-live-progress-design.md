# Forge Conversation Live Progress Design

**Status:** Approved in product discussion

**Date:** 2026-07-18

**Scope:** Forge desktop conversation turns

**Baseline:** `2026-07-18-forge-conversation-result-first-design.md`

## 1. Summary

Forge will represent the work-in-progress portion of a conversation turn as one quiet, continuous status line. The line communicates only the current user-understandable stage. It must not expose internal reasoning, tool names, commands, file paths, or a stream of implementation events.

When the answer finishes, the active status resolves into a compact footer beneath the answer:

```text
已完成 · 12 秒 · 4 项操作
```

The footer can expand in place to show two to four summarized stages. It never opens a new panel and never turns the conversation back into an execution log.

The selected direction is **Continuous Status Line**. This is a preserve-mode refinement of the existing result-first conversation design, not a theme redesign.

## 2. Product Intent

The experience should make three things clear without adding cognitive load:

1. Forge is actively working.
2. The work has moved into a new understandable stage.
3. The work completed, stopped, or failed with an honest summary.

The interaction should feel like a native desktop workbench: dense enough for frequent use, visually quiet, and motivated by state changes rather than decorative motion.

Recommended design dials:

- Visual variance: 4/10
- Motion intensity: 4/10
- Information density: 6/10

## 3. Goals

- Keep exactly one active progress surface per conversation turn.
- Replace noisy process messages with one stable stage label.
- Give the live state restrained motion so the interface does not feel frozen.
- Preserve continuity between live progress and the terminal answer footer.
- Summarize elapsed time and meaningful operation count after completion.
- Allow users to inspect two to four key stages without leaving the conversation.
- Represent completion, cancellation, waiting, and failure truthfully.
- Respect reduced-motion preferences.
- Preserve the existing theme, typography system, and result-first hierarchy.

## 4. Non-goals

- Showing chain-of-thought or hidden model reasoning.
- Showing raw tool calls, shell commands, file paths, or terminal output.
- Creating a task timeline, activity feed, or second conversation stream.
- Adding a new work-panel destination for process details.
- Redesigning the Forge color palette or theme tokens.
- Replacing existing evidence or diagnostics surfaces used for debugging.
- Reporting a fake numeric percentage when the runtime cannot know real completion progress.

## 5. Chosen Interaction Model

### 5.1 Turn lifecycle

Each turn follows one visible lifecycle:

```text
submitted
  -> delayed live progress
  -> stable stage updates
  -> generating answer
  -> terminal answer footer
  -> optional inline stage expansion
```

Detailed behavior:

1. After submission, Forge waits approximately 240 ms before showing live progress. Fast responses therefore do not flash a transient loader.
2. Once visible, the turn renders one status line beneath the user message.
3. The status line displays one high-level stage at a time.
4. Each visible stage remains stable for at least approximately 600 ms. Bursty runtime events update the pending stage but are not replayed as a queue.
5. When answer streaming starts, the stage becomes `正在生成答复` and the quiet live motion continues while text arrives.
6. When streaming reaches a terminal state, the active status line leaves and a terminal footer enters beneath the answer using matching typography and a short crossfade.
7. Clicking the terminal footer expands summarized stages directly below it. Clicking again collapses them.

The live row and footer do not need a literal geometric morph. Continuity comes from shared type scale, baseline, color role, compact height, wording, and coordinated timing.

### 5.2 Visible stage vocabulary

Runtime events are normalized to a finite vocabulary:

| Visible label | User meaning | Typical underlying activity |
| --- | --- | --- |
| `正在分析` | Understanding the request and deciding an approach | reasoning, planning, initial inspection |
| `正在查找相关内容` | Locating information needed to proceed | search, read, context gathering |
| `正在进行修改` | Applying changes to the requested target | edits, patches, configuration changes |
| `正在验证结果` | Checking whether the outcome works | builds, tests, checks, visual verification |
| `正在生成答复` | Turning the completed work into the user-facing result | answer streaming |
| `等待你的确认` | Progress cannot continue until the user decides or approves | approval or required user input |

Labels must stay in user language. They must never be assembled from raw filenames, commands, providers, model messages, or tool identifiers.

If an event cannot be mapped safely, Forge retains the current safe stage instead of exposing the event payload or inventing a more specific label.

### 5.3 Terminal footer

Terminal states use concise, honest copy:

| State | Default copy |
| --- | --- |
| Completed | `已完成 · 12 秒 · 4 项操作` |
| Stopped by user | `已停止 · 8 秒` |
| Failed | `未完成 · 12 秒 · 3 项操作` |

Rules:

- Duration starts when the user submits the turn and ends when the turn reaches its terminal state.
- Durations below one second display as `<1 秒`.
- Durations from one to 59 seconds display as whole seconds.
- Longer durations use compact minute-and-second wording.
- Stopped turns omit operation count when no meaningful operation completed.
- Failed turns must never use the completed icon, color, or label.
- Historical terminal footers are static and never resume looping motion.

### 5.4 Meaningful operation count

The operation count is a count of user-meaningful activity groups, not raw runtime events.

Examples:

- A burst of related searches and reads becomes one analysis or discovery operation.
- A contiguous set of edits serving the same requested change becomes one modification operation.
- A related build, test, and check batch becomes one verification operation.
- Answer streaming is not counted as an operation.
- Retries within one activity group do not inflate the count.

Grouping should be deterministic and derived from normalized activity categories and turn ordering. The count is a summary aid, not an instrumentation metric.

### 5.5 Inline expansion

Clicking the terminal footer expands a short list of key stages in place:

```text
✓ 分析需求
✓ 完成修改
✓ 验证结果
```

Expansion rules:

- Prefer two to four stages.
- Do not invent an extra stage for a trivial turn that has only one meaningful stage.
- Merge consecutive activity groups of the same kind.
- Use completed, stopped, or failed semantics accurately per stage.
- Keep raw evidence, command output, filenames, and tool metadata out of this list.
- The expansion pushes following content naturally; it does not use a popover or side panel.
- The expansion state is local to the turn and can be toggled without network work.

## 6. Visual Specification

### 6.1 Active status line

- Approximate occupied height: 22 px.
- No card, pill, container background, or enclosing border.
- One 6 px status dot.
- One short stage label using the existing subdued text role.
- One approximately 32 px by 1 px trace after the label.
- Alignment follows the assistant answer column rather than the user message bubble edge.
- Reserve a stable minimum height while active to avoid vertical jitter during label changes.

### 6.2 Terminal footer

- Uses the same type size, visual baseline, and subdued color family as the active status line.
- Uses a compact disclosure affordance without adding a filled button treatment.
- Hover and focus states may strengthen text or icon contrast slightly, but must not introduce a new card surface.
- Expanded stages use spacing and a light vertical rhythm instead of nested cards.

### 6.3 Theme preservation

This feature consumes existing semantic tokens for text, border, success, warning, error, and focus. It does not introduce a new palette and does not alter the user's current theme work.

## 7. Motion Specification

Motion is state feedback only.

### 7.1 Live motion

- The 6 px dot breathes on an approximately 1.8 s loop.
- Breathing changes opacity and only a very small amount of scale.
- The 32 px trace carries a low-contrast moving highlight on an approximately 1.6 s loop.
- The trace must remain secondary to the stage label.

### 7.2 Stage changes

- Stage labels crossfade in approximately 120 ms.
- Labels do not slide horizontally, bounce, blur, or type character by character.
- The stable-stage rule prevents motion from restarting rapidly during event bursts.

### 7.3 Terminal transition

- Active progress and the terminal footer use an approximately 160 ms coordinated crossfade.
- The final answer remains the dominant entrance; the footer should not compete with it.
- Waiting, stopped, failed, and completed states do not retain ambient looping motion.

### 7.4 Reduced motion

Under `prefers-reduced-motion: reduce`:

- Stop the breathing loop.
- Stop the moving trace.
- Use a static status dot and trace.
- Remove nonessential label and footer transitions.
- Preserve all state wording and interaction affordances.

## 8. Boundary and Failure Behavior

### 8.1 Fast completion

If the turn reaches answer streaming before the approximately 240 ms presentation delay, skip the live line and render the answer normally. The terminal footer still appears when the turn completes.

### 8.2 Bursty events

Only the latest safe pending stage is retained while the current visible stage satisfies its minimum dwell. Forge does not replay missed intermediate stages.

### 8.3 Waiting for the user

When approval or required input blocks progress:

- Stop ambient live motion.
- Display `等待你的确认`.
- Keep the actual approval or input control in its existing authoritative surface.
- Do not imply that work is continuing in the background.

### 8.4 Cancellation

When the user stops a turn:

- Stop live motion immediately.
- Preserve any already-streamed answer content.
- Render the stopped footer.
- Expand only stages that actually occurred.

### 8.5 Failure

When a turn fails:

- Stop live motion immediately.
- Render `未完成`, never `已完成`.
- Keep the user-facing failure explanation in the result area.
- Mark the failed stage accurately in the inline summary.
- Do not expose raw failure payloads unless an existing diagnostics surface explicitly owns them.

### 8.6 Multiple concurrent or background events

Each conversation turn owns at most one live status line. Background events that are not part of the visible turn must not hijack its stage label.

## 9. Accessibility

- The live stage is a polite status region, not an assertive alert.
- Stage changes should be announced without announcing every raw runtime event.
- The terminal footer is a real button with keyboard focus, `aria-expanded`, and an associated content region.
- Completed, stopped, and failed states are distinguished by text and semantics, not color alone.
- Focus rings use the existing semantic focus token.
- Motion-reduction behavior is testable and does not remove information.

## 10. Data and Component Design

The feature extends the existing result-first conversation path rather than creating a parallel renderer.

### 10.1 Normalized view data

The UI needs normalized, presentation-safe data equivalent to:

```ts
type TurnStage =
  | "analyzing"
  | "discovering"
  | "modifying"
  | "verifying"
  | "answering"
  | "waiting";

type TurnOutcome = "completed" | "stopped" | "failed";

type TurnProgressView = {
  stage: TurnStage | null;
  startedAt: number;
  terminalAt: number | null;
  outcome: TurnOutcome | null;
  meaningfulOperationCount: number;
  summaryStages: Array<{
    kind: TurnStage;
    outcome: TurnOutcome | "completed";
  }>;
};
```

Exact names may follow current repository conventions. The important boundary is that raw event payloads are normalized before rendering.

### 10.2 Ownership

- `conversationProgress.ts` owns safe stage derivation, timing inputs, meaningful activity grouping, and terminal summary derivation.
- `TurnProgress.tsx` owns the delayed appearance, stable visible label, live status semantics, and active motion markup.
- `ConversationTurn.tsx` owns the transition between active progress, streamed answer, and terminal footer.
- `conversationTurnView.ts` owns presentation-ready terminal wording and compact summary stages.
- `conversation-turn.css` owns active, terminal, expansion, and reduced-motion styling.

No component should infer user-facing text directly from a raw filename, command, or tool name.

## 11. Testing Strategy

### 11.1 Unit coverage

Cover at least:

- Raw activities map only to the approved finite vocabulary.
- Unknown payloads cannot leak into labels.
- One current candidate is derived per turn.
- Initial visibility delay suppresses fast-response flashing.
- Minimum dwell prevents rapid stage churn.
- Bursty stages collapse to the latest safe pending stage.
- Meaningful operation grouping is deterministic.
- Answer streaming is excluded from operation count.
- Duration formatting covers sub-second, seconds, and minute ranges.
- Completed, stopped, and failed summaries use correct copy.
- Summary stages merge consecutive duplicates and remain compact.

### 11.2 Component coverage

Cover at least:

- One live status region renders for an active turn.
- Stage labels transition without adding message rows.
- Waiting state stops active semantics.
- Terminal footer exposes correct button and expansion attributes.
- Inline stages expand and collapse in place.
- Reduced-motion styling removes looping animation.

### 11.3 Desktop product E2E

Add or extend acceptance coverage proving:

1. A running turn never shows more than one live progress row.
2. The progress row contains no filename, command, tool name, or internal reasoning text.
3. The answer can stream while the stage reads `正在生成答复`.
4. Completion renders duration and meaningful operation count.
5. Clicking the footer expands two to four key stages in the conversation.
6. Collapsing restores the compact result-first view.
7. Stopped and failed turns do not render completed semantics.
8. Reduced-motion mode removes the breathing and trace loops.

## 12. Documentation and Acceptance Sync

Because this is a user-visible desktop runtime surface, implementation must update:

- root `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `apps/desktop/e2e/acceptance.spec.ts` or the closest authoritative desktop product spec
- `scripts/acceptance.sh --dry-run` if its advertised coverage changes

Documentation should describe the user-visible behavior, not the internal event mapper.

## 13. Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| Stage wording leaks implementation detail | Finite vocabulary and safe fallback retain the last known stage |
| Fast events make the label flicker | Initial delay plus minimum dwell and latest-pending collapse |
| Motion becomes decorative or distracting | Small geometry, low contrast, restrained timing, full reduced-motion path |
| Operation count feels inflated or dishonest | Count normalized activity groups rather than tool events |
| Completion footer adds conversation clutter | One compact line, inline disclosure, no nested cards |
| Failure looks successful | Separate terminal outcomes, wording, semantics, and tests |
| Theme work is overwritten | Consume existing semantic tokens; do not modify the palette |

## 14. Acceptance Criteria

The design is implemented when all of the following are true:

- [ ] Every active conversation turn renders at most one live status line.
- [ ] The line uses only the approved safe stage vocabulary.
- [ ] No raw internal activity appears in the main conversation.
- [ ] Fast responses do not flash a loader.
- [ ] Visible stages do not churn faster than the stable-stage threshold.
- [ ] The dot and trace provide restrained live motion.
- [ ] Reduced-motion mode stops all ambient loops.
- [ ] Answer streaming uses the generating-answer stage.
- [ ] Completed answers show duration and meaningful operation count.
- [ ] Stopped and failed turns use accurate terminal wording.
- [ ] Terminal details expand in place and remain limited to key stages.
- [ ] The feature introduces no new cards, pills, work-panel routes, or theme palette changes.
- [ ] Unit, component, desktop E2E, build, and precommit verification pass.
- [ ] User-facing documentation and acceptance coverage remain synchronized.
