import assert from "node:assert/strict";
import test from "node:test";
import type { LiveProgressCandidate } from "./conversationTurnView.ts";
import * as progress from "./conversationProgress.ts";

interface StableProgressState {
  visible: LiveProgressCandidate | null;
  visibleSince: number;
  pending: LiveProgressCandidate | null;
  dueAt: number | null;
}

type StableProgressModule = {
  createStableProgressState?: (candidate: LiveProgressCandidate | null, now: number) => StableProgressState;
  updateStableProgress?: (
    state: StableProgressState,
    candidate: LiveProgressCandidate | null,
    now: number,
    urgent?: boolean,
  ) => StableProgressState;
  flushStableProgress?: (state: StableProgressState, now: number) => StableProgressState;
};

test("coalesces rapid labels and exposes only the latest after 600ms", () => {
  const module = progress as StableProgressModule;
  assert.equal(typeof module.createStableProgressState, "function");
  assert.equal(typeof module.updateStableProgress, "function");
  assert.equal(typeof module.flushStableProgress, "function");

  const reading = candidate("read:App.tsx", "正在查看 App.tsx");
  const editing = candidate("edit:App.tsx", "正在调整 App.tsx");
  const checking = candidate("verify:type", "正在检查类型");
  const initial = module.createStableProgressState!(reading, 0);
  const queuedEdit = module.updateStableProgress!(initial, editing, 180);
  const coalescedCheck = module.updateStableProgress!(queuedEdit, checking, 420);
  const early = module.flushStableProgress!(coalescedCheck, 599);
  const released = module.flushStableProgress!(early, 600);

  assert.equal(queuedEdit.visible?.id, reading.id);
  assert.equal(coalescedCheck.pending?.id, checking.id);
  assert.equal(early.visible?.id, reading.id);
  assert.equal(released.visible?.id, checking.id);
  assert.equal(released.visibleSince, 600);
  assert.equal(released.pending, null);
});

test("keeps equivalent labels stable and lets interruptions bypass the dwell", () => {
  const module = progress as Required<StableProgressModule>;
  const reading = candidate("read:App.tsx", "正在查看 App.tsx");
  const editing = candidate("edit:App.tsx", "正在调整 App.tsx");
  const blocked = candidate("blocked:confirm", "等待你的确认");
  const initial = module.createStableProgressState(reading, 100);
  const queued = module.updateStableProgress(initial, editing, 240);
  const equivalent = module.updateStableProgress(queued, reading, 320);
  const urgent = module.updateStableProgress(equivalent, blocked, 360, true);
  const cleared = module.updateStableProgress(urgent, null, 400);

  assert.equal(equivalent.visibleSince, 100);
  assert.equal(equivalent.pending, null);
  assert.equal(urgent.visible?.id, blocked.id);
  assert.equal(urgent.visibleSince, 360);
  assert.equal(cleared.visible, null);
});

function candidate(id: string, label: string): LiveProgressCandidate {
  return { id, label };
}
